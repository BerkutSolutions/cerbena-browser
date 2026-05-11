use super::*;

pub(crate) fn runtime_proxy_endpoint_impl(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Option<(String, u16)> {
    let state = app_handle.state::<AppState>();
    let runtime = state.route_runtime.lock().ok()?;
    let session = runtime.sessions.get(&profile_id.to_string())?;
    if !session_is_active_impl(session) {
        return None;
    }
    match session.backend {
        RouteRuntimeBackend::SingBox | RouteRuntimeBackend::ContainerSocks => {
            Some(("127.0.0.1".to_string(), session.listen_port?))
        }
        RouteRuntimeBackend::OpenVpn | RouteRuntimeBackend::AmneziaWg => None,
    }
}

pub(crate) fn runtime_session_active_impl(app_handle: &AppHandle, profile_id: Uuid) -> bool {
    let state = app_handle.state::<AppState>();
    let runtime = match state.route_runtime.lock() {
        Ok(value) => value,
        Err(_) => return false,
    };
    runtime
        .sessions
        .get(&profile_id.to_string())
        .map(session_is_active_impl)
        .unwrap_or(false)
}

pub(crate) fn runtime_session_snapshot_impl(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Option<RouteRuntimeSessionSnapshot> {
    let state = app_handle.state::<AppState>();
    let runtime = state.route_runtime.lock().ok()?;
    let session = runtime.sessions.get(&profile_id.to_string())?.clone();
    Some(RouteRuntimeSessionSnapshot {
        backend: session.backend,
        listen_port: session.listen_port,
        config_path: session.config_path,
        cleanup_paths: session.cleanup_paths,
        tunnel_name: session.tunnel_name,
        container_name: session.container_name,
    })
}

pub(crate) fn session_is_active_impl(session: &RouteRuntimeSession) -> bool {
    match session.backend {
        RouteRuntimeBackend::SingBox | RouteRuntimeBackend::OpenVpn => {
            session.pid.map(is_process_running).unwrap_or(false)
        }
        RouteRuntimeBackend::ContainerSocks => session
            .container_name
            .as_deref()
            .map(is_container_runtime_active)
            .unwrap_or(false),
        RouteRuntimeBackend::AmneziaWg => session
            .tunnel_name
            .as_deref()
            .map(amnezia::is_amnezia_tunnel_active_impl)
            .unwrap_or(false),
    }
}

pub(crate) fn route_runtime_required_for_profile_impl(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> bool {
    let state = app_handle.state::<AppState>();
    let store = match state.network_store.lock() {
        Ok(value) => value,
        Err(_) => return false,
    };
    let profile_key = profile_id.to_string();
    let (route_mode, selected_template_id) =
        resolve_effective_route_selection(&store, &profile_key);
    if route_mode == "direct" {
        return false;
    }
    if store.global_route_settings.global_vpn_enabled {
        return true;
    }
    let Some(template_id) = selected_template_id.as_ref() else {
        return false;
    };
    let Some(template) = store.connection_templates.get(template_id) else {
        return false;
    };
    let nodes = normalized_nodes(template);
    if nodes.is_empty() {
        return false;
    }
    nodes.len() > 1
        || nodes
            .iter()
            .any(|node| !matches!(node.connection_type.as_str(), "proxy"))
}

pub(crate) fn stop_profile_route_runtime_impl(app_handle: &AppHandle, profile_id: Uuid) {
    let state = app_handle.state::<AppState>();
    let key = profile_id.to_string();
    let session = {
        let mut runtime = match state.route_runtime.lock() {
            Ok(value) => value,
            Err(_) => return,
        };
        runtime.sessions.remove(&key)
    };
    if let Some(session) = session {
        append_profile_log(
            app_handle,
            profile_id,
            "route-runtime",
            format!(
                "Stopping route runtime backend={}",
                route_runtime_backend_label_impl(session.backend)
            ),
        );
        if let Some(pid) = session.pid {
            terminate_pid(pid);
        }
        if session.backend == RouteRuntimeBackend::AmneziaWg {
            if let Some(tunnel_name) = session.tunnel_name.as_deref() {
                let _ = amnezia::stop_amnezia_tunnel_service_impl(tunnel_name);
                let _ = amnezia::wait_amnezia_tunnel_state_impl(tunnel_name, false, 8_000);
                if let Ok(binary) = resolve_amneziawg_binary_path(app_handle) {
                    let _ = amnezia::uninstall_amnezia_tunnel_impl(&binary, tunnel_name);
                }
            }
        }
        if session.backend == RouteRuntimeBackend::ContainerSocks {
            if let Some(container_name) = session.container_name.as_deref() {
                stop_container_runtime(container_name);
            }
        }
        let _ = fs::remove_file(session.config_path);
        for path in session.cleanup_paths {
            let _ = fs::remove_file(path);
        }
    }
}

pub(crate) fn route_runtime_backend_label_impl(backend: RouteRuntimeBackend) -> &'static str {
    match backend {
        RouteRuntimeBackend::SingBox => "sing-box",
        RouteRuntimeBackend::OpenVpn => "openvpn",
        RouteRuntimeBackend::AmneziaWg => "amneziawg",
        RouteRuntimeBackend::ContainerSocks => "container-socks",
    }
}

pub(crate) fn stop_all_route_runtime_impl(app_handle: &AppHandle) {
    let sessions = {
        let state = app_handle.state::<AppState>();
        let runtime = match state.route_runtime.lock() {
            Ok(value) => value,
            Err(_) => return,
        };
        runtime
            .sessions
            .keys()
            .filter_map(|value| Uuid::parse_str(value).ok())
            .collect::<Vec<_>>()
    };
    for profile_id in sessions {
        stop_profile_route_runtime_impl(app_handle, profile_id);
    }
}
