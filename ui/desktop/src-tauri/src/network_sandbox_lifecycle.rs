use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    network_sandbox::resolve_profile_network_sandbox_mode,
    network_sandbox_adapter::{resolve_adapter_plan_for_profile, NetworkSandboxAdapterPlan},
    network_sandbox_container::{
        cleanup_stale_container_environments, ensure_profile_container_environment,
        remove_profile_container_environment,
    },
    process_tracking::is_process_running,
    route_runtime::{
        cleanup_legacy_route_runtime, cleanup_stale_route_runtime_artifacts,
        ensure_profile_route_runtime, runtime_session_snapshot, stop_all_route_runtime,
        stop_profile_route_runtime, RouteRuntimeBackend,
    },
    state::AppState,
    traffic_gateway::{
        ensure_profile_gateway, stop_all_profile_gateways, stop_profile_gateway,
        GatewayLaunchConfig,
    },
};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxLifecycleState {
    pub active_profiles: BTreeMap<String, NetworkSandboxLifecycleRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxLifecycleRecord {
    pub profile_id: String,
    pub gateway_port: u16,
    pub adapter: NetworkSandboxAdapterPlan,
    pub runtime_backend: Option<String>,
    pub runtime_listen_port: Option<u16>,
    pub config_path: Option<String>,
    pub cleanup_paths: Vec<String>,
    pub tunnel_name: Option<String>,
    pub container_network_name: Option<String>,
}

pub fn ensure_profile_network_stack(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Result<GatewayLaunchConfig, String> {
    let strategy = resolve_strategy_for_profile(app_handle, profile_id)?;
    let state = app_handle.state::<AppState>();
    let adapter = resolve_adapter_plan_for_profile(state.inner(), Some(profile_id), &strategy);
    if !adapter.available {
        return Err(format!(
            "network sandbox adapter `{}` is not available: {}",
            adapter.adapter_kind, adapter.reason
        ));
    }
    let container_network_name =
        if strategy.mode == crate::network_sandbox::ResolvedNetworkSandboxMode::Container {
            Some(ensure_profile_container_environment(
                app_handle, profile_id,
            )?)
        } else {
            None
        };
    let gateway = ensure_profile_gateway(app_handle, profile_id)?;
    eprintln!(
        "[network-sandbox] profile={} gateway_port={} adapter={}",
        profile_id, gateway.port, adapter.adapter_kind
    );
    if let Err(error) = ensure_profile_route_runtime(app_handle, profile_id) {
        stop_profile_gateway(app_handle, profile_id);
        stop_profile_route_runtime(app_handle, profile_id);
        if container_network_name.is_some() {
            remove_profile_container_environment(app_handle, profile_id);
        }
        unregister_active_stack(app_handle, profile_id);
        return Err(error);
    }
    register_active_stack(
        app_handle,
        profile_id,
        gateway.port,
        adapter,
        container_network_name,
    );
    Ok(gateway)
}

pub fn stop_profile_network_stack(app_handle: &AppHandle, profile_id: Uuid) {
    eprintln!("[network-sandbox] stopping stack for profile={profile_id}");
    stop_profile_route_runtime(app_handle, profile_id);
    stop_profile_gateway(app_handle, profile_id);
    remove_profile_container_environment(app_handle, profile_id);
    unregister_active_stack(app_handle, profile_id);
}

pub fn stop_all_profile_network_stacks(app_handle: &AppHandle) {
    let container_profiles = {
        let state = app_handle.state::<AppState>();
        state
            .network_sandbox_lifecycle
            .lock()
            .ok()
            .map(|lifecycle| {
                lifecycle
                    .active_profiles
                    .keys()
                    .filter_map(|value| Uuid::parse_str(value).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    stop_all_route_runtime(app_handle);
    stop_all_profile_gateways(app_handle);
    for profile_id in container_profiles {
        remove_profile_container_environment(app_handle, profile_id);
    }
    clear_active_stacks(app_handle);
}

pub fn cleanup_network_sandbox_janitor(app_handle: &AppHandle) {
    cleanup_legacy_route_runtime(app_handle);
    let active_profiles = collect_active_profiles(app_handle);
    prune_orphan_profile_gateways(app_handle, &active_profiles);
    cleanup_stale_route_runtime_artifacts(app_handle, &active_profiles);
    cleanup_stale_container_environments(app_handle, &active_profiles);
    prune_stale_lifecycle_records(app_handle, &active_profiles);
}

fn resolve_strategy_for_profile(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Result<crate::network_sandbox::ResolvedNetworkSandboxStrategy, String> {
    let state = app_handle.state::<AppState>();
    let template = {
        let store = state
            .network_store
            .lock()
            .map_err(|_| "network store lock poisoned".to_string())?;
        let profile_key = profile_id.to_string();
        let profile_route_mode = store
            .vpn_proxy
            .get(&profile_key)
            .map(|value| value.route_mode.trim().to_lowercase())
            .unwrap_or_else(|| "direct".to_string());
        if profile_route_mode == "direct" {
            None
        } else if store.global_route_settings.global_vpn_enabled {
            store
                .global_route_settings
                .default_template_id
                .as_ref()
                .and_then(|id| store.connection_templates.get(id))
                .cloned()
        } else {
            store
                .profile_template_selection
                .get(&profile_key)
                .and_then(|id| store.connection_templates.get(id))
                .cloned()
        }
    };
    resolve_profile_network_sandbox_mode(state.inner(), profile_id, template.as_ref())
}

fn register_active_stack(
    app_handle: &AppHandle,
    profile_id: Uuid,
    gateway_port: u16,
    adapter: NetworkSandboxAdapterPlan,
    container_network_name: Option<String>,
) {
    let state = app_handle.state::<AppState>();
    let runtime = runtime_session_snapshot(app_handle, profile_id);
    let record = NetworkSandboxLifecycleRecord {
        profile_id: profile_id.to_string(),
        gateway_port,
        adapter,
        runtime_backend: runtime
            .as_ref()
            .map(|value| runtime_backend_label(value.backend).to_string()),
        runtime_listen_port: runtime.as_ref().and_then(|value| value.listen_port),
        config_path: runtime
            .as_ref()
            .map(|value| value.config_path.to_string_lossy().to_string()),
        cleanup_paths: runtime
            .as_ref()
            .map(|value| {
                value
                    .cleanup_paths
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        tunnel_name: runtime.and_then(|value| value.tunnel_name),
        container_network_name,
    };
    if let Ok(mut lifecycle) = state.network_sandbox_lifecycle.lock() {
        lifecycle
            .active_profiles
            .insert(profile_id.to_string(), record);
    };
}

fn unregister_active_stack(app_handle: &AppHandle, profile_id: Uuid) {
    let state = app_handle.state::<AppState>();
    if let Ok(mut lifecycle) = state.network_sandbox_lifecycle.lock() {
        lifecycle.active_profiles.remove(&profile_id.to_string());
    };
}

fn clear_active_stacks(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    if let Ok(mut lifecycle) = state.network_sandbox_lifecycle.lock() {
        lifecycle.active_profiles.clear();
    };
}

fn collect_active_profiles(app_handle: &AppHandle) -> BTreeSet<Uuid> {
    let state = app_handle.state::<AppState>();
    let active = match state.launched_processes.lock() {
        Ok(value) => value
            .iter()
            .filter_map(|(profile_id, pid)| {
                if is_process_running(*pid) {
                    Some(*profile_id)
                } else {
                    None
                }
            })
            .collect::<BTreeSet<_>>(),
        Err(_) => BTreeSet::new(),
    };
    active
}

fn prune_orphan_profile_gateways(app_handle: &AppHandle, active_profiles: &BTreeSet<Uuid>) {
    let state = app_handle.state::<AppState>();
    let listeners = match state.traffic_gateway.lock() {
        Ok(value) => value
            .listeners
            .keys()
            .filter_map(|value| Uuid::parse_str(value).ok())
            .collect::<Vec<_>>(),
        Err(_) => return,
    };
    for profile_id in listeners {
        if !active_profiles.contains(&profile_id) {
            stop_profile_gateway(app_handle, profile_id);
        }
    }
}

fn prune_stale_lifecycle_records(app_handle: &AppHandle, active_profiles: &BTreeSet<Uuid>) {
    let state = app_handle.state::<AppState>();
    if let Ok(mut lifecycle) = state.network_sandbox_lifecycle.lock() {
        lifecycle.active_profiles.retain(|profile_id, _| {
            Uuid::parse_str(profile_id)
                .ok()
                .map(|id| active_profiles.contains(&id))
                .unwrap_or(false)
        });
    };
}

fn runtime_backend_label(backend: RouteRuntimeBackend) -> &'static str {
    match backend {
        RouteRuntimeBackend::SingBox => "sing-box",
        RouteRuntimeBackend::OpenVpn => "openvpn",
        RouteRuntimeBackend::AmneziaWg => "amneziawg",
        RouteRuntimeBackend::ContainerSocks => "container-socks",
    }
}
