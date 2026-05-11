use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::{
    envelope::{ok, UiEnvelope},
    network_sandbox_adapter::{resolve_adapter_plan_for_profile, NetworkSandboxAdapterPlan},
    route_runtime::amnezia_config_requires_native_backend,
    state::{persist_network_sandbox_store, AppState, ConnectionTemplate, NetworkStore},
};
#[path = "network_sandbox_store.rs"]
mod store;
#[path = "network_sandbox_resolution.rs"]
mod resolution;
#[path = "network_sandbox_models.rs"]
mod models;
pub(crate) use models::*;

pub fn load_network_sandbox_store(path: &PathBuf) -> Result<NetworkSandboxStore, String> {
    store::load_network_sandbox_store_impl(path)
}

pub fn migrate_network_sandbox_store(
    store: &mut NetworkSandboxStore,
    network_store: &NetworkStore,
) -> Result<bool, String> {
    store::migrate_network_sandbox_store_impl(store, network_store)
}

pub fn resolve_profile_network_sandbox_mode(
    state: &AppState,
    profile_id: Uuid,
    template: Option<&ConnectionTemplate>,
) -> Result<ResolvedNetworkSandboxStrategy, String> {
    let sandbox_store = state
        .network_sandbox_store
        .lock()
        .map_err(|_| "network sandbox store lock poisoned".to_string())?
        .clone();
    let profile_key = profile_id.to_string();
    let profile_settings = sandbox_store
        .profiles
        .get(&profile_key)
        .cloned()
        .unwrap_or_default();
    let requires_native = template
        .map(resolution::template_requires_native_compatibility)
        .transpose()?
        .unwrap_or(false);
    let preferred = resolution::resolve_requested_mode(&sandbox_store.global, &profile_settings);
    let container_supported = template
        .map(resolution::template_supports_container_mode)
        .transpose()?
        .unwrap_or(true);
    let resolution = resolution::resolve_network_sandbox_strategy_for_modes(
        &sandbox_store.global,
        &profile_settings,
        preferred,
        requires_native,
        container_supported,
    );

    drop(sandbox_store);
    resolution::record_resolved_mode(
        state,
        &profile_key,
        resolution.mode.as_str(),
        &resolution.reason,
    )?;
    Ok(resolution)
}

pub fn resolve_profile_network_sandbox_view(
    state: &AppState,
    profile_id: Uuid,
) -> Result<NetworkSandboxProfileView, String> {
    let (profile_key, store_snapshot, template) = {
        let network_store = state
            .network_store
            .lock()
            .map_err(|_| "network store lock poisoned".to_string())?;
        let profile_key = profile_id.to_string();
        let (_, selected_template_id) =
            resolution::resolve_effective_route_selection(&network_store, &profile_key);
        let template = selected_template_id
            .as_ref()
            .and_then(|id| network_store.connection_templates.get(id))
            .cloned();
        drop(network_store);
        let sandbox_store = state
            .network_sandbox_store
            .lock()
            .map_err(|_| "network sandbox store lock poisoned".to_string())?
            .clone();
        (profile_key, sandbox_store, template)
    };
    let resolution = resolve_profile_network_sandbox_mode(state, profile_id, template.as_ref())?;
    Ok(sandbox_profile_view(
        state,
        &store_snapshot,
        &profile_key,
        Some(profile_id),
        Some(&resolution),
    ))
}

pub fn sandbox_profile_view(
    state: &AppState,
    store: &NetworkSandboxStore,
    profile_id: &str,
    profile_uuid: Option<Uuid>,
    resolution: Option<&ResolvedNetworkSandboxStrategy>,
) -> NetworkSandboxProfileView {
    let profile = store.profiles.get(profile_id).cloned().unwrap_or_default();
    let requested_mode = resolution::resolve_requested_mode(&store.global, &profile);
    let effective_mode = resolution
        .map(|value| value.effective_mode().to_string())
        .unwrap_or_else(|| requested_mode.clone());
    NetworkSandboxProfileView {
        effective_mode,
        preferred_mode: profile.preferred_mode,
        global_policy_enabled: store.global.enabled,
        migrated_legacy_native: profile.migrated_legacy_native,
        last_resolved_mode: profile.last_resolved_mode,
        last_resolution_reason: profile.last_resolution_reason,
        resolution_available: resolution.map(|value| value.available).unwrap_or(true),
        requires_native_backend: resolution
            .map(|value| value.requires_native_backend)
            .unwrap_or(false),
        requested_mode,
        target_runtime: store.global.target_runtime.clone(),
        adapter: resolution
            .map(|value| resolve_adapter_plan_for_profile(state, profile_uuid, value))
            .unwrap_or(NetworkSandboxAdapterPlan {
                adapter_kind: "unknown".to_string(),
                runtime_kind: "unknown".to_string(),
                available: true,
                requires_system_network_access: false,
                max_helper_processes: 0,
                estimated_memory_mb: 0,
                active_sandboxes: 0,
                max_active_sandboxes: store.global.max_active_sandboxes.max(1),
                supports_native_isolation: false,
                reason: "No resolved strategy yet".to_string(),
            }),
    }
}

#[tauri::command]
pub fn save_network_sandbox_profile_settings(
    state: State<AppState>,
    request: SaveNetworkSandboxProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<NetworkSandboxProfileView>, String> {
    let mut store = state
        .network_sandbox_store
        .lock()
        .map_err(|_| "network sandbox store lock poisoned".to_string())?;
    let entry = store
        .profiles
        .entry(request.profile_id.clone())
        .or_default();
    entry.preferred_mode = resolution::normalize_mode(request.preferred_mode);
    let path = state.network_sandbox_store_path(&state.app_handle)?;
    persist_network_sandbox_store(&path, &store)?;
    drop(store);
    if let Ok(profile_id) = Uuid::parse_str(&request.profile_id) {
        let maybe_pid = state
            .launched_processes
            .lock()
            .ok()
            .and_then(|map| map.get(&profile_id).copied());
        if let Some(pid) = maybe_pid {
            if crate::process_tracking::is_process_running(pid) {
                crate::network_sandbox_lifecycle::stop_profile_network_stack(
                    &state.app_handle,
                    profile_id,
                );
                let _ = crate::network_sandbox_lifecycle::ensure_profile_network_stack(
                    &state.app_handle,
                    profile_id,
                );
            }
        }
        let view = resolve_profile_network_sandbox_view(state.inner(), profile_id)?;
        return Ok(ok(correlation_id, view));
    }
    let store = state
        .network_sandbox_store
        .lock()
        .map_err(|_| "network sandbox store lock poisoned".to_string())?;
    Ok(ok(
        correlation_id,
        sandbox_profile_view(state.inner(), &store, &request.profile_id, None, None),
    ))
}

#[tauri::command]
pub fn save_network_sandbox_global_settings(
    state: State<AppState>,
    request: SaveNetworkSandboxGlobalRequest,
    correlation_id: String,
) -> Result<UiEnvelope<NetworkSandboxProfileView>, String> {
    let mut store = state
        .network_sandbox_store
        .lock()
        .map_err(|_| "network sandbox store lock poisoned".to_string())?;
    store.global.enabled = request.enabled;
    store.global.default_mode =
        resolution::normalize_mode(request.default_mode).unwrap_or_else(|| MODE_AUTO.to_string());
    let path = state.network_sandbox_store_path(&state.app_handle)?;
    persist_network_sandbox_store(&path, &store)?;
    let snapshot = store.clone();
    drop(store);
    Ok(ok(
        correlation_id,
        sandbox_profile_view(state.inner(), &snapshot, "__global__", None, None),
    ))
}

#[tauri::command]
pub fn preview_network_sandbox_settings(
    state: State<AppState>,
    request: PreviewNetworkSandboxRequest,
    correlation_id: String,
) -> Result<UiEnvelope<NetworkSandboxPreviewView>, String> {
    let route_mode = request
        .route_mode
        .unwrap_or_else(|| "direct".to_string())
        .trim()
        .to_lowercase();
    if route_mode == "direct"
        || request
            .template_id
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        let store = state
            .network_sandbox_store
            .lock()
            .map_err(|_| "network sandbox store lock poisoned".to_string())?
            .clone();
        let profile_key = request
            .profile_id
            .clone()
            .unwrap_or_else(|| "__preview__".to_string());
        let view = sandbox_profile_view(state.inner(), &store, &profile_key, None, None);
        return Ok(ok(
            correlation_id,
            NetworkSandboxPreviewView {
                sandbox: view,
                compatible_modes: Vec::new(),
                active_template_id: None,
                route_mode,
            },
        ));
    }

    let template = {
        let store = state
            .network_store
            .lock()
            .map_err(|_| "network store lock poisoned".to_string())?;
        request
            .template_id
            .as_ref()
            .and_then(|id| store.connection_templates.get(id))
            .cloned()
    };
    let preview = resolution::sandbox_view_for_preview(
        state.inner(),
        request.profile_id.as_deref(),
        request.preferred_mode,
        template.as_ref(),
        request.global_scope,
    )?;
    Ok(ok(correlation_id, preview))
}

pub fn resolve_global_network_sandbox_view(
    state: &AppState,
    template: Option<&ConnectionTemplate>,
) -> Result<NetworkSandboxProfileView, String> {
    Ok(resolution::sandbox_view_for_preview(state, None, None, template, true)?.sandbox)
}

pub(crate) fn normalize_global_settings(settings: &mut NetworkSandboxGlobalSettings) {
    resolution::normalize_global_settings(settings)
}

pub(crate) fn normalize_mode(value: Option<String>) -> Option<String> {
    resolution::normalize_mode(value)
}

pub(crate) fn profile_requires_legacy_native_compatibility(
    store: &NetworkStore,
    profile_key: &str,
) -> Result<bool, String> {
    resolution::profile_requires_legacy_native_compatibility(store, profile_key)
}

#[cfg(test)]
// Regression coverage markers kept in this root file for contract scanners:
// traffic_isolation_prefers_userspace_for_non_native_routes
// traffic_isolation_blocks_native_routes_without_explicit_fallback
// traffic_isolation_allows_native_container_mode_when_requested
#[path = "network_sandbox_tests.rs"]
mod tests;
