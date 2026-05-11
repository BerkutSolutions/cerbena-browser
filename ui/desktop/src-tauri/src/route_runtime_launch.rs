use super::*;
use std::collections::BTreeSet as StdBTreeSet;
use std::sync::{Mutex, OnceLock};

fn launch_guard() -> &'static Mutex<StdBTreeSet<Uuid>> {
    static GUARD: OnceLock<Mutex<StdBTreeSet<Uuid>>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(StdBTreeSet::new()))
}

struct ProfileLaunchGuard {
    profile_id: Uuid,
}

impl Drop for ProfileLaunchGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = launch_guard().lock() {
            active.remove(&self.profile_id);
        }
    }
}

pub(crate) fn ensure_profile_route_runtime_impl(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Result<(), String> {
    {
        let mut active = launch_guard()
            .lock()
            .map_err(|_| "route runtime launch guard lock poisoned".to_string())?;
        if active.contains(&profile_id) {
            return Err(format!(
                "route runtime launch re-entry detected for profile {}",
                profile_id
            ));
        }
        active.insert(profile_id);
    }
    let _guard = ProfileLaunchGuard { profile_id };

    let state = app_handle.state::<AppState>();
    let profile_key = profile_id.to_string();
    let (route_mode, template_id, template) = {
        let store = state
            .network_store
            .lock()
            .map_err(|_| "network store lock poisoned".to_string())?;
        let (route_mode, selected_id) = resolve_effective_route_selection(&store, &profile_key);
        let selected = selected_id
            .as_ref()
            .and_then(|id| store.connection_templates.get(id))
            .cloned();
        (route_mode, selected_id, selected)
    };

    if route_mode == "direct" {
        stop_profile_route_runtime(app_handle, profile_id);
        return Ok(());
    }

    let Some(template) = template else {
        stop_profile_route_runtime(app_handle, profile_id);
        return Ok(());
    };
    let template_id = template_id.unwrap_or_else(|| template.id.clone());
    let nodes = normalized_nodes(&template);
    if nodes.is_empty() {
        stop_profile_route_runtime(app_handle, profile_id);
        return Ok(());
    }

    let requires_runtime = nodes.len() > 1
        || nodes
            .iter()
            .any(|node| !matches!(node.connection_type.as_str(), "proxy"));
    if !requires_runtime {
        stop_profile_route_runtime(app_handle, profile_id);
        return Ok(());
    }

    let uses_openvpn = nodes
        .iter()
        .any(|node| node.connection_type == "vpn" && node.protocol == "openvpn");
    let sandbox_strategy =
        resolve_profile_network_sandbox_mode(&state, profile_id, Some(&template))?;
    if !sandbox_strategy.available {
        return Err(format!(
            "network sandbox strategy `{}` is not available for this profile: {}",
            sandbox_strategy.mode.as_str(),
            sandbox_strategy.reason
        ));
    }
    let amnezia_native_required = nodes.len() == 1
        && nodes[0].connection_type == "vpn"
        && nodes[0].protocol == "amnezia"
        && amnezia_node_requires_native_backend(&nodes[0])?;
    let uses_amnezia_native = sandbox_strategy.mode
        == ResolvedNetworkSandboxMode::CompatibilityNative
        && amnezia_native_required;
    let uses_container_runtime = sandbox_strategy.mode == ResolvedNetworkSandboxMode::Container;
    let uses_amnezia_container = uses_container_runtime && amnezia_native_required;
    if uses_openvpn
        && !(nodes.len() == 1
            && nodes[0].connection_type == "vpn"
            && nodes[0].protocol == "openvpn")
    {
        return Err(
            "openvpn runtime currently supports only single-node VPN templates (without chain)"
                .to_string(),
        );
    }
    if !nodes.iter().all(node_supported_by_runtime) {
        let unsupported = nodes
            .iter()
            .filter(|node| !node_supported_by_runtime(node))
            .map(|node| format!("{}:{}", node.connection_type, node.protocol))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "route runtime does not support protocol chain yet: {unsupported}"
        ));
    }
    let required_tools = required_runtime_tools(
        &nodes,
        uses_openvpn,
        uses_amnezia_native,
        uses_amnezia_container,
        uses_container_runtime,
    );
    eprintln!(
        "[route-runtime][trace] profile={} step=required-tools tools={:?}",
        profile_id, required_tools
    );
    let has_runtime_tools = !required_tools.is_empty();
    if has_runtime_tools {
        emit_profile_launch_progress(
            app_handle,
            profile_id,
            "network-runtime",
            "profile.launchProgress.networkRuntime",
        );
    }
    append_route_runtime_log(
        state.inner(),
        format!(
            "[route-runtime] profile={} strategy={} requested={} native_required={} reason={}",
            profile_id,
            sandbox_strategy.mode.as_str(),
            sandbox_strategy.requested_mode,
            sandbox_strategy.requires_native_backend,
            sandbox_strategy.reason
        ),
    );
    eprintln!(
        "[route-runtime] profile={} strategy={} requested={} native_required={} reason={}",
        profile_id,
        sandbox_strategy.mode.as_str(),
        sandbox_strategy.requested_mode,
        sandbox_strategy.requires_native_backend,
        sandbox_strategy.reason
    );

    let signature_payload = json!({
        "route_mode": route_mode,
        "template_id": template_id,
        "nodes": nodes,
    });
    eprintln!(
        "[route-runtime][trace] profile={} step=signature-build",
        profile_id
    );
    let signature = serde_json::to_string(&signature_payload).map_err(|e| e.to_string())?;
    {
        let runtime = state
            .route_runtime
            .lock()
            .map_err(|_| "route runtime lock poisoned".to_string())?;
        if let Some(current) = runtime.sessions.get(&profile_key) {
            if current.signature == signature && session_is_active(current) {
                return Ok(());
            }
        }
    }

    eprintln!(
        "[route-runtime][trace] profile={} step=ensure-tools-start",
        profile_id
    );
    ensure_network_runtime_tools(app_handle, &required_tools)?;
    eprintln!(
        "[route-runtime][trace] profile={} step=ensure-tools-done",
        profile_id
    );
    stop_profile_route_runtime(app_handle, profile_id);
    eprintln!(
        "[route-runtime][trace] profile={} step=stopped-prev-runtime",
        profile_id
    );

    let runtime_dir = state
        .profile_root
        .join(profile_id.to_string())
        .join("runtime");
    fs::create_dir_all(&runtime_dir).map_err(|e| format!("create runtime dir: {e}"))?;
    let sing_box_log_path = runtime_dir.join("sing-box-route.log");

    if let Some(mut container_session) = launch_container::try_launch_container_runtime_impl(
        app_handle,
        profile_id,
        &runtime_dir,
        &nodes,
        uses_container_runtime,
        uses_openvpn,
        uses_amnezia_container,
    )? {
        eprintln!(
            "[route-runtime][trace] profile={} step=container-runtime-launched",
            profile_id
        );
        container_session.signature = signature.clone();
        let mut runtime = state
            .route_runtime
            .lock()
            .map_err(|_| "route runtime lock poisoned".to_string())?;
        runtime.sessions.insert(profile_key, container_session);
        return Ok(());
    }

    if uses_amnezia_native {
        let node = nodes
            .first()
            .ok_or_else(|| "amnezia runtime requires one node".to_string())?;
        let amnezia_binary = resolve_amneziawg_binary_path(app_handle)?
            .to_string_lossy()
            .to_string();
        let launch =
            amnezia::launch_amneziawg_runtime_impl(node, &runtime_dir, profile_id, &amnezia_binary)?;
        let mut runtime = state
            .route_runtime
            .lock()
            .map_err(|_| "route runtime lock poisoned".to_string())?;
        runtime.sessions.insert(
            profile_key,
            RouteRuntimeSession {
                signature,
                pid: None,
                backend: RouteRuntimeBackend::AmneziaWg,
                listen_port: None,
                config_path: launch.config_path,
                cleanup_paths: launch.cleanup_paths,
                tunnel_name: Some(launch.tunnel_name),
                container_name: None,
            },
        );
        return Ok(());
    }
    if uses_openvpn {
        let node = nodes
            .first()
            .ok_or_else(|| "openvpn runtime requires one node".to_string())?;
        let openvpn_binary = resolve_openvpn_binary_path(app_handle)?
            .to_string_lossy()
            .to_string();
        let openvpn = openvpn::launch_openvpn_runtime_impl(
            node,
            &runtime_dir,
            profile_id,
            &openvpn_binary,
        )?;
        let mut runtime = state
            .route_runtime
            .lock()
            .map_err(|_| "route runtime lock poisoned".to_string())?;
        runtime.sessions.insert(
            profile_key,
            RouteRuntimeSession {
                signature,
                pid: Some(openvpn.pid),
                backend: RouteRuntimeBackend::OpenVpn,
                listen_port: None,
                config_path: openvpn.config_path,
                cleanup_paths: openvpn.cleanup_paths,
                tunnel_name: None,
                container_name: None,
            },
        );
        return Ok(());
    }

    let mut host_session = launch_host::launch_host_singbox_runtime_impl(
        app_handle,
        &runtime_dir,
        &nodes,
        &sing_box_log_path,
    )?;
    host_session.signature = signature;
    let mut runtime = state
        .route_runtime
        .lock()
        .map_err(|_| "route runtime lock poisoned".to_string())?;
    runtime.sessions.insert(profile_key, host_session);

    Ok(())
}
