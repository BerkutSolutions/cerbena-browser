use std::{collections::BTreeMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::{
    envelope::{ok, UiEnvelope},
    network_sandbox_adapter::{resolve_adapter_plan_for_profile, NetworkSandboxAdapterPlan},
    route_runtime::amnezia_config_requires_native_backend,
    state::{persist_network_sandbox_store, AppState, ConnectionTemplate, NetworkStore},
};

const MODE_AUTO: &str = "auto";
const MODE_ISOLATED: &str = "isolated";
const MODE_COMPAT_NATIVE: &str = "compatibility-native";
const MODE_CONTAINER: &str = "container";
const MODE_BLOCKED: &str = "blocked";

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxStore {
    #[serde(default)]
    pub global: NetworkSandboxGlobalSettings,
    #[serde(default)]
    pub profiles: BTreeMap<String, NetworkSandboxProfileSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxGlobalSettings {
    #[serde(default)]
    pub enabled: bool,
    pub default_mode: String,
    pub allow_native_compatibility_fallback: bool,
    pub target_runtime: String,
    pub max_active_sandboxes: u8,
}

impl Default for NetworkSandboxGlobalSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            default_mode: MODE_AUTO.to_string(),
            allow_native_compatibility_fallback: false,
            target_runtime: "launcher-managed".to_string(),
            max_active_sandboxes: 2,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxProfileSettings {
    pub preferred_mode: Option<String>,
    #[serde(default)]
    pub migrated_legacy_native: bool,
    pub last_resolved_mode: Option<String>,
    pub last_resolution_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxProfileView {
    pub effective_mode: String,
    pub preferred_mode: Option<String>,
    pub global_policy_enabled: bool,
    pub migrated_legacy_native: bool,
    pub last_resolved_mode: Option<String>,
    pub last_resolution_reason: Option<String>,
    pub resolution_available: bool,
    pub requires_native_backend: bool,
    pub requested_mode: String,
    pub target_runtime: String,
    pub adapter: NetworkSandboxAdapterPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedNetworkSandboxMode {
    IsolatedUserspace,
    CompatibilityNative,
    Container,
    Blocked,
}

impl ResolvedNetworkSandboxMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IsolatedUserspace => MODE_ISOLATED,
            Self::CompatibilityNative => MODE_COMPAT_NATIVE,
            Self::Container => MODE_CONTAINER,
            Self::Blocked => MODE_BLOCKED,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedNetworkSandboxStrategy {
    pub mode: ResolvedNetworkSandboxMode,
    pub requested_mode: String,
    pub requires_native_backend: bool,
    pub available: bool,
    pub reason: String,
}

impl ResolvedNetworkSandboxStrategy {
    pub fn effective_mode(&self) -> &'static str {
        self.mode.as_str()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNetworkSandboxProfileRequest {
    pub profile_id: String,
    pub preferred_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNetworkSandboxGlobalRequest {
    pub enabled: bool,
    pub default_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewNetworkSandboxRequest {
    pub profile_id: Option<String>,
    pub route_mode: Option<String>,
    pub template_id: Option<String>,
    pub preferred_mode: Option<String>,
    #[serde(default)]
    pub global_scope: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxPreviewView {
    pub sandbox: NetworkSandboxProfileView,
    pub compatible_modes: Vec<String>,
    pub active_template_id: Option<String>,
    pub route_mode: String,
}

pub fn load_network_sandbox_store(path: &PathBuf) -> Result<NetworkSandboxStore, String> {
    if !path.exists() {
        return Ok(NetworkSandboxStore::default());
    }
    let raw = fs::read(path).map_err(|e| format!("read network sandbox store: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("parse network sandbox store: {e}"))
}

pub fn migrate_network_sandbox_store(
    store: &mut NetworkSandboxStore,
    network_store: &NetworkStore,
) -> Result<bool, String> {
    normalize_global_settings(&mut store.global);

    let mut changed = false;
    let mut profile_keys = network_store.vpn_proxy.keys().cloned().collect::<Vec<_>>();
    for key in network_store.profile_template_selection.keys() {
        if !profile_keys.iter().any(|item| item == key) {
            profile_keys.push(key.clone());
        }
    }

    for profile_key in profile_keys {
        let entry = store.profiles.entry(profile_key.clone()).or_default();
        let legacy_native =
            profile_requires_legacy_native_compatibility(network_store, &profile_key)?;
        if legacy_native && entry.preferred_mode.is_none() {
            entry.preferred_mode = Some(MODE_COMPAT_NATIVE.to_string());
            entry.migrated_legacy_native = true;
            entry.last_resolved_mode = Some(MODE_COMPAT_NATIVE.to_string());
            entry.last_resolution_reason =
                Some("Adapted from a pre-sandbox AmneziaWG profile".to_string());
            changed = true;
        } else if entry.preferred_mode.is_some() {
            let normalized = normalize_mode(entry.preferred_mode.clone());
            if entry.preferred_mode != normalized {
                entry.preferred_mode = normalized;
                changed = true;
            }
        }
    }

    Ok(changed)
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
        .map(template_requires_native_compatibility)
        .transpose()?
        .unwrap_or(false);
    let preferred = resolve_requested_mode(&sandbox_store.global, &profile_settings);
    let container_supported = template
        .map(template_supports_container_mode)
        .transpose()?
        .unwrap_or(true);
    let resolution = resolve_network_sandbox_strategy_for_modes(
        &sandbox_store.global,
        &profile_settings,
        preferred,
        requires_native,
        container_supported,
    );

    drop(sandbox_store);
    record_resolved_mode(
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
            resolve_effective_route_selection(&network_store, &profile_key);
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
    let requested_mode = resolve_requested_mode(&store.global, &profile);
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
    entry.preferred_mode = normalize_mode(request.preferred_mode);
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
        normalize_mode(request.default_mode).unwrap_or_else(|| MODE_AUTO.to_string());
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
    let preview = sandbox_view_for_preview(
        state.inner(),
        request.profile_id.as_deref(),
        request.preferred_mode,
        template.as_ref(),
        request.global_scope,
    )?;
    Ok(ok(correlation_id, preview))
}

fn record_resolved_mode(
    state: &AppState,
    profile_key: &str,
    mode: &str,
    reason: &str,
) -> Result<(), String> {
    let mut store = state
        .network_sandbox_store
        .lock()
        .map_err(|_| "network sandbox store lock poisoned".to_string())?;
    let entry = store.profiles.entry(profile_key.to_string()).or_default();
    let mut changed = false;
    if entry.last_resolved_mode.as_deref() != Some(mode) {
        entry.last_resolved_mode = Some(mode.to_string());
        changed = true;
    }
    if entry.last_resolution_reason.as_deref() != Some(reason) {
        entry.last_resolution_reason = Some(reason.to_string());
        changed = true;
    }
    if changed {
        let path = state.network_sandbox_store_path(&state.app_handle)?;
        persist_network_sandbox_store(&path, &store)?;
    }
    Ok(())
}

fn normalize_global_settings(settings: &mut NetworkSandboxGlobalSettings) {
    settings.default_mode = normalize_mode(Some(settings.default_mode.clone()))
        .unwrap_or_else(|| MODE_AUTO.to_string());
    if settings.target_runtime.trim().is_empty() {
        settings.target_runtime = "launcher-managed".to_string();
    }
    if settings.max_active_sandboxes == 0 {
        settings.max_active_sandboxes = 2;
    }
}

fn resolve_requested_mode(
    global: &NetworkSandboxGlobalSettings,
    profile: &NetworkSandboxProfileSettings,
) -> String {
    profile.preferred_mode.clone().unwrap_or_else(|| {
        if global.enabled {
            normalize_mode(Some(global.default_mode.clone()))
                .unwrap_or_else(|| MODE_AUTO.to_string())
        } else {
            MODE_AUTO.to_string()
        }
    })
}

fn resolve_network_sandbox_strategy_for_modes(
    global: &NetworkSandboxGlobalSettings,
    profile: &NetworkSandboxProfileSettings,
    requested_mode: String,
    requires_native: bool,
    container_supported: bool,
) -> ResolvedNetworkSandboxStrategy {
    if requested_mode == MODE_CONTAINER && !container_supported {
        return ResolvedNetworkSandboxStrategy {
            mode: ResolvedNetworkSandboxMode::Blocked,
            requested_mode,
            requires_native_backend: requires_native,
            available: false,
            reason: "Selected route is not compatible with container isolation yet".to_string(),
        };
    }
    let (mode, available, reason) = if !requires_native {
        let resolved = if requested_mode == MODE_CONTAINER {
            ResolvedNetworkSandboxMode::Container
        } else {
            ResolvedNetworkSandboxMode::IsolatedUserspace
        };
        (
            resolved,
            true,
            "Template is compatible with isolated userspace runtime".to_string(),
        )
    } else {
        match requested_mode.as_str() {
            MODE_COMPAT_NATIVE => (
                ResolvedNetworkSandboxMode::CompatibilityNative,
                true,
                "Profile is pinned to compatibility-native mode".to_string(),
            ),
            MODE_CONTAINER => (
                ResolvedNetworkSandboxMode::Container,
                true,
                "Container sandbox mode is selected; launcher will validate the host runtime and per-profile sandbox capacity during launch".to_string(),
            ),
            MODE_AUTO if profile.migrated_legacy_native => (
                ResolvedNetworkSandboxMode::CompatibilityNative,
                true,
                "Legacy profile was auto-adapted to compatibility-native mode".to_string(),
            ),
            MODE_AUTO if global.enabled && global.allow_native_compatibility_fallback => (
                ResolvedNetworkSandboxMode::CompatibilityNative,
                true,
                "Global sandbox policy allows compatibility-native fallback".to_string(),
            ),
            _ => (
                ResolvedNetworkSandboxMode::Blocked,
                false,
                "This Amnezia profile requires a machine-wide compatibility backend; isolated mode forbids that path".to_string(),
            ),
        }
    };
    ResolvedNetworkSandboxStrategy {
        mode,
        requested_mode,
        requires_native_backend: requires_native,
        available,
        reason,
    }
}

fn normalize_mode(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let normalized = raw.trim().to_lowercase();
        match normalized.as_str() {
            MODE_AUTO | MODE_ISOLATED | MODE_COMPAT_NATIVE | MODE_CONTAINER => Some(normalized),
            "" => None,
            _ => Some(MODE_AUTO.to_string()),
        }
    })
}

fn compatible_modes_for_template(requires_native: bool, container_supported: bool) -> Vec<String> {
    if requires_native {
        let mut modes = vec![MODE_COMPAT_NATIVE.to_string()];
        if container_supported {
            modes.push(MODE_CONTAINER.to_string());
        }
        modes
    } else {
        let mut modes = vec![MODE_ISOLATED.to_string()];
        if container_supported {
            modes.push(MODE_CONTAINER.to_string());
        }
        modes
    }
}

fn resolve_effective_route_selection(
    store: &NetworkStore,
    profile_key: &str,
) -> (String, Option<String>) {
    let profile_route_mode = store
        .vpn_proxy
        .get(profile_key)
        .map(|value| value.route_mode.trim().to_lowercase())
        .unwrap_or_else(|| "direct".to_string());
    if profile_route_mode == "direct" {
        return ("direct".to_string(), None);
    }
    if store.global_route_settings.global_vpn_enabled {
        let template_id = store
            .global_route_settings
            .default_template_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        return ("vpn".to_string(), template_id);
    }
    let template_id = store.profile_template_selection.get(profile_key).cloned();
    (profile_route_mode, template_id)
}

fn sandbox_view_for_preview(
    state: &AppState,
    profile_id: Option<&str>,
    preferred_mode: Option<String>,
    template: Option<&ConnectionTemplate>,
    global_scope: bool,
) -> Result<NetworkSandboxPreviewView, String> {
    let sandbox_store = state
        .network_sandbox_store
        .lock()
        .map_err(|_| "network sandbox store lock poisoned".to_string())?
        .clone();
    let profile_key = profile_id.unwrap_or("__preview__").to_string();
    let mut profile_settings = profile_id
        .and_then(|id| sandbox_store.profiles.get(id))
        .cloned()
        .unwrap_or_default();
    profile_settings.preferred_mode = normalize_mode(preferred_mode);
    let requires_native = template
        .map(template_requires_native_compatibility)
        .transpose()?
        .unwrap_or(false);
    let container_supported = template
        .map(template_supports_container_mode)
        .transpose()?
        .unwrap_or(true);
    let requested_mode = if global_scope {
        if sandbox_store.global.enabled {
            normalize_mode(
                profile_settings
                    .preferred_mode
                    .clone()
                    .or_else(|| Some(sandbox_store.global.default_mode.clone())),
            )
            .unwrap_or_else(|| MODE_AUTO.to_string())
        } else {
            MODE_AUTO.to_string()
        }
    } else {
        resolve_requested_mode(&sandbox_store.global, &profile_settings)
    };
    let resolution = resolve_network_sandbox_strategy_for_modes(
        &sandbox_store.global,
        &profile_settings,
        requested_mode,
        requires_native,
        container_supported,
    );
    let view = sandbox_profile_view(
        state,
        &sandbox_store,
        &profile_key,
        profile_id.and_then(|id| Uuid::parse_str(id).ok()),
        Some(&resolution),
    );
    Ok(NetworkSandboxPreviewView {
        sandbox: view,
        compatible_modes: compatible_modes_for_template(requires_native, container_supported),
        active_template_id: template.map(|item| item.id.clone()),
        route_mode: template
            .map(|_| "vpn".to_string())
            .unwrap_or_else(|| "direct".to_string()),
    })
}

pub fn resolve_global_network_sandbox_view(
    state: &AppState,
    template: Option<&ConnectionTemplate>,
) -> Result<NetworkSandboxProfileView, String> {
    Ok(sandbox_view_for_preview(state, None, None, template, true)?.sandbox)
}

fn profile_requires_legacy_native_compatibility(
    store: &NetworkStore,
    profile_key: &str,
) -> Result<bool, String> {
    let payload = store.vpn_proxy.get(profile_key);
    let route_mode = payload
        .map(|value| value.route_mode.trim().to_lowercase())
        .unwrap_or_else(|| "direct".to_string());
    if route_mode == "direct" {
        return Ok(false);
    }
    let Some(template_id) = store.profile_template_selection.get(profile_key) else {
        return Ok(false);
    };
    let Some(template) = store.connection_templates.get(template_id) else {
        return Ok(false);
    };
    template_requires_native_compatibility(template)
}

fn template_supports_container_mode(template: &ConnectionTemplate) -> Result<bool, String> {
    let nodes = if !template.nodes.is_empty() {
        template.nodes.clone()
    } else if !template.connection_type.trim().is_empty() && !template.protocol.trim().is_empty() {
        vec![crate::state::ConnectionNode {
            id: template.id.clone(),
            connection_type: template.connection_type.clone(),
            protocol: template.protocol.clone(),
            host: template.host.clone(),
            port: template.port,
            username: template.username.clone(),
            password: template.password.clone(),
            bridges: template.bridges.clone(),
            settings: BTreeMap::new(),
        }]
    } else {
        Vec::new()
    };
    if nodes.is_empty() {
        return Ok(false);
    }
    let single_node = nodes.len() == 1;
    Ok(nodes.iter().all(|node| {
        match (
            node.connection_type.trim().to_ascii_lowercase().as_str(),
            node.protocol.trim().to_ascii_lowercase().as_str(),
        ) {
            ("proxy", "http" | "socks4" | "socks5") => true,
            ("v2ray", "vmess" | "vless" | "trojan" | "shadowsocks") => true,
            ("vpn", "wireguard" | "amnezia") => true,
            ("vpn", "openvpn") => single_node,
            ("tor", "none" | "obfs4" | "snowflake" | "meek") => true,
            _ => false,
        }
    }))
}

fn template_requires_native_compatibility(template: &ConnectionTemplate) -> Result<bool, String> {
    let nodes = if !template.nodes.is_empty() {
        template.nodes.clone()
    } else if template.connection_type.trim().eq_ignore_ascii_case("vpn")
        && template.protocol.trim().eq_ignore_ascii_case("amnezia")
    {
        vec![crate::state::ConnectionNode {
            id: template.id.clone(),
            connection_type: template.connection_type.clone(),
            protocol: template.protocol.clone(),
            host: template.host.clone(),
            port: template.port,
            username: template.username.clone(),
            password: template.password.clone(),
            bridges: template.bridges.clone(),
            settings: BTreeMap::new(),
        }]
    } else {
        Vec::new()
    };

    if nodes.len() != 1 {
        return Ok(false);
    }
    let node = &nodes[0];
    if !node.connection_type.trim().eq_ignore_ascii_case("vpn")
        || !node.protocol.trim().eq_ignore_ascii_case("amnezia")
    {
        return Ok(false);
    }
    let Some(key) = node.settings.get("amneziaKey") else {
        return Ok(false);
    };
    amnezia_config_requires_native_backend(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ConnectionNode, ConnectionTemplate, NetworkStore};
    use browser_network_policy::VpnProxyTabPayload;

    fn global_settings() -> NetworkSandboxGlobalSettings {
        NetworkSandboxGlobalSettings::default()
    }

    fn amnezia_template() -> ConnectionTemplate {
        ConnectionTemplate {
            id: "tpl-amnezia".to_string(),
            name: "Amnezia".to_string(),
            nodes: vec![ConnectionNode {
                id: "node-1".to_string(),
                connection_type: "vpn".to_string(),
                protocol: "amnezia".to_string(),
                host: None,
                port: None,
                username: None,
                password: None,
                bridges: None,
                settings: BTreeMap::from([(
                    "amneziaKey".to_string(),
                    "[Interface]\nAddress = 10.8.1.84/32\nPrivateKey = PRIVATE\nJc = 4\nJmin = 10\nJmax = 50\n\n[Peer]\nPublicKey = PUBLIC\nAllowedIPs = 0.0.0.0/0\nEndpoint = 5.129.225.48:32542\n".to_string(),
                )]),
            }],
            connection_type: String::new(),
            protocol: String::new(),
            host: None,
            port: None,
            username: None,
            password: None,
            bridges: None,
            updated_at_epoch_ms: 1,
        }
    }

    #[test]
    fn migrate_marks_legacy_native_amnezia_profile() {
        let mut network_store = NetworkStore::default();
        network_store.vpn_proxy.insert(
            "profile-1".to_string(),
            VpnProxyTabPayload {
                route_mode: "vpn".to_string(),
                proxy: None,
                vpn: None,
                kill_switch_enabled: true,
            },
        );
        network_store
            .profile_template_selection
            .insert("profile-1".to_string(), "tpl-amnezia".to_string());
        network_store
            .connection_templates
            .insert("tpl-amnezia".to_string(), amnezia_template());

        let mut sandbox_store = NetworkSandboxStore::default();
        let changed =
            migrate_network_sandbox_store(&mut sandbox_store, &network_store).expect("migrate");
        assert!(changed);
        let profile = sandbox_store.profiles.get("profile-1").expect("profile");
        assert_eq!(profile.preferred_mode.as_deref(), Some(MODE_COMPAT_NATIVE));
        assert!(profile.migrated_legacy_native);
    }

    #[test]
    fn normalize_mode_rejects_empty_and_normalizes_unknown_to_auto() {
        assert_eq!(normalize_mode(Some(String::new())), None);
        assert_eq!(
            normalize_mode(Some("compatibility-native".to_string())).as_deref(),
            Some(MODE_COMPAT_NATIVE)
        );
        assert_eq!(
            normalize_mode(Some("unexpected".to_string())).as_deref(),
            Some(MODE_AUTO)
        );
    }

    #[test]
    fn container_mode_supports_tor_bridge_variants() {
        let build = |protocol: &str| ConnectionTemplate {
            id: format!("tpl-{protocol}"),
            name: format!("TOR {protocol}"),
            nodes: vec![ConnectionNode {
                id: "node-1".to_string(),
                connection_type: "tor".to_string(),
                protocol: protocol.to_string(),
                host: None,
                port: None,
                username: None,
                password: None,
                bridges: Some("Bridge example".to_string()),
                settings: BTreeMap::new(),
            }],
            connection_type: String::new(),
            protocol: String::new(),
            host: None,
            port: None,
            username: None,
            password: None,
            bridges: None,
            updated_at_epoch_ms: 1,
        };

        assert!(template_supports_container_mode(&build("obfs4")).expect("obfs4"));
        assert!(template_supports_container_mode(&build("snowflake")).expect("snowflake"));
        assert!(template_supports_container_mode(&build("meek")).expect("meek"));
    }

    #[test]
    fn traffic_isolation_prefers_userspace_for_non_native_routes() {
        let strategy = resolve_network_sandbox_strategy_for_modes(
            &global_settings(),
            &NetworkSandboxProfileSettings::default(),
            MODE_AUTO.to_string(),
            false,
            true,
        );

        assert_eq!(strategy.mode, ResolvedNetworkSandboxMode::IsolatedUserspace);
        assert!(strategy.available);
        assert_eq!(strategy.effective_mode(), MODE_ISOLATED);
        assert!(!strategy.requires_native_backend);
    }

    #[test]
    fn traffic_isolation_blocks_native_routes_without_explicit_fallback() {
        let strategy = resolve_network_sandbox_strategy_for_modes(
            &global_settings(),
            &NetworkSandboxProfileSettings::default(),
            MODE_AUTO.to_string(),
            true,
            true,
        );

        assert_eq!(strategy.mode, ResolvedNetworkSandboxMode::Blocked);
        assert!(!strategy.available);
        assert!(strategy.requires_native_backend);
        assert!(strategy
            .reason
            .contains("requires a machine-wide compatibility backend"));
    }

    #[test]
    fn traffic_isolation_allows_native_container_mode_when_requested() {
        let strategy = resolve_network_sandbox_strategy_for_modes(
            &global_settings(),
            &NetworkSandboxProfileSettings::default(),
            MODE_CONTAINER.to_string(),
            true,
            true,
        );

        assert_eq!(strategy.mode, ResolvedNetworkSandboxMode::Container);
        assert!(strategy.available);
        assert!(strategy.requires_native_backend);
        assert_eq!(strategy.effective_mode(), MODE_CONTAINER);
    }

    #[test]
    fn traffic_isolation_uses_global_native_fallback_for_migrated_profiles() {
        let strategy = resolve_network_sandbox_strategy_for_modes(
            &NetworkSandboxGlobalSettings {
                enabled: true,
                allow_native_compatibility_fallback: true,
                ..NetworkSandboxGlobalSettings::default()
            },
            &NetworkSandboxProfileSettings::default(),
            MODE_AUTO.to_string(),
            true,
            true,
        );

        assert_eq!(
            strategy.mode,
            ResolvedNetworkSandboxMode::CompatibilityNative
        );
        assert!(strategy.available);
        assert!(strategy.requires_native_backend);
        assert!(strategy.reason.contains("Global sandbox policy allows"));
    }

    #[test]
    fn traffic_isolation_blocks_unsupported_container_requests() {
        let strategy = resolve_network_sandbox_strategy_for_modes(
            &global_settings(),
            &NetworkSandboxProfileSettings::default(),
            MODE_CONTAINER.to_string(),
            true,
            false,
        );

        assert_eq!(strategy.mode, ResolvedNetworkSandboxMode::Blocked);
        assert!(!strategy.available);
        assert!(strategy
            .reason
            .contains("not compatible with container isolation"));
    }
}
