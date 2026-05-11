use std::{
    collections::BTreeMap,
    io::Read,
    net::{TcpStream, ToSocketAddrs, UdpSocket},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use browser_network_policy::vpn_proxy_tab::test_connect;
use browser_network_policy::{
    validate_dns_tab, validate_vpn_proxy_tab, BlocklistSource, DnsBlocklistUpdater, DnsTabPayload,
    NetworkPolicy, NetworkPolicyEngine, PolicyRequest, RouteMode, VpnProxyTabPayload,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::{
    envelope::{ok, UiEnvelope},
    launcher_commands::load_global_security_record,
    network_sandbox::{
        resolve_global_network_sandbox_view, resolve_profile_network_sandbox_view,
        NetworkSandboxProfileView,
    },
    network_sandbox_lifecycle::{ensure_profile_network_stack, stop_profile_network_stack},
    process_tracking::is_process_running as is_pid_running,
    state::{
        persist_network_store, AppState, ConnectionNode, ConnectionTemplate,
        NetworkGlobalRouteSettings,
    },
};
#[path = "network_commands_diagnostics.rs"]
mod diagnostics;
#[path = "network_commands_dns_policy.rs"]
mod dns_policy;
#[path = "network_commands_global_blocklists.rs"]
mod global_blocklists;
#[path = "network_commands_route.rs"]
mod route;
#[path = "network_commands_templates.rs"]
mod templates;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVpnProxyRequest {
    pub profile_id: String,
    pub payload: VpnProxyTabPayload,
    pub selected_template_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDnsRequest {
    pub profile_id: String,
    pub payload: DnsTabPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDecisionView {
    pub action: String,
    pub reason_code: String,
    pub selected_route: String,
    pub matched_rules: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRequestInput {
    pub has_profile_context: bool,
    pub vpn_up: bool,
    pub target_domain: String,
    pub target_service: Option<String>,
    pub tor_up: bool,
    pub dns_over_tor: bool,
    pub active_route: RouteMode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConnectionNodeRequest {
    pub node_id: Option<String>,
    pub connection_type: String,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub bridges: Option<String>,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConnectionTemplateRequest {
    pub template_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub nodes: Vec<SaveConnectionNodeRequest>,
    // Legacy single-node fields are kept for backward compatibility.
    #[serde(default)]
    pub connection_type: String,
    #[serde(default)]
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub bridges: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteConnectionTemplateRequest {
    pub template_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePingRequest {
    pub template_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStateView {
    pub payload: Option<VpnProxyTabPayload>,
    pub selected_template_id: Option<String>,
    pub connection_templates: Vec<ConnectionTemplate>,
    pub global_route: NetworkGlobalRouteSettings,
    pub sandbox: NetworkSandboxProfileView,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveGlobalRouteSettingsRequest {
    pub global_vpn_enabled: bool,
    pub block_without_vpn: bool,
    pub default_template_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionHealthView {
    pub reachable: bool,
    pub status: String,
    pub latency_ms: Option<u128>,
    pub message: String,
}

#[tauri::command]
pub fn get_network_state(
    state: State<AppState>,
    profile_id: String,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    route::get_network_state_impl(state, profile_id, correlation_id)
}

#[tauri::command]
pub fn save_connection_template(
    state: State<AppState>,
    request: SaveConnectionTemplateRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ConnectionTemplate>, String> {
    route::save_connection_template_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn delete_connection_template(
    state: State<AppState>,
    request: DeleteConnectionTemplateRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    route::delete_connection_template_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn ping_connection_template(
    state: State<AppState>,
    request: TemplatePingRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ConnectionHealthView>, String> {
    diagnostics::ping_connection_template_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn test_connection_template_request(
    request: SaveConnectionTemplateRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ConnectionHealthView>, String> {
    diagnostics::test_connection_template_request_impl(request, correlation_id)
}

#[tauri::command]
pub fn save_vpn_proxy_policy(
    state: State<AppState>,
    request: SaveVpnProxyRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    route::save_vpn_proxy_policy_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn save_global_route_settings(
    state: State<AppState>,
    request: SaveGlobalRouteSettingsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    route::save_global_route_settings_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn test_vpn_proxy_policy(
    payload: VpnProxyTabPayload,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    diagnostics::test_vpn_proxy_policy_impl(payload, correlation_id)
}

#[tauri::command]
pub fn save_dns_policy(
    state: State<AppState>,
    request: SaveDnsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    dns_policy::save_dns_policy_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn get_service_catalog(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    dns_policy::get_service_catalog_impl(state, correlation_id)
}

#[tauri::command]
pub fn set_service_block_all(
    state: State<AppState>,
    category: String,
    block_all: bool,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    dns_policy::set_service_block_all_impl(state, category, block_all, correlation_id)
}

#[tauri::command]
pub fn set_service_allowed(
    state: State<AppState>,
    category: String,
    service: String,
    allowed: bool,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    dns_policy::set_service_allowed_impl(state, category, service, allowed, correlation_id)
}

#[tauri::command]
pub fn evaluate_network_policy_demo(
    policy: NetworkPolicy,
    request: PolicyRequestInput,
    correlation_id: String,
) -> Result<UiEnvelope<PolicyDecisionView>, String> {
    dns_policy::evaluate_network_policy_demo_impl(policy, request, correlation_id)
}

fn persist_store(state: &AppState, store: &crate::state::NetworkStore) -> Result<(), String> {
    let path = state.network_store_path(&state.app_handle)?;
    persist_network_store(&path, store)
}

fn refresh_running_profiles_route_runtime(
    state: &AppState,
    profile_ids: &[Uuid],
) -> Result<(), String> {
    if profile_ids.is_empty() {
        return Ok(());
    }
    let launched = state
        .launched_processes
        .lock()
        .map_err(|_| "launch map lock poisoned".to_string())?;
    let running = profile_ids
        .iter()
        .copied()
        .filter(|profile_id| {
            launched
                .get(profile_id)
                .copied()
                .map(is_pid_running)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    drop(launched);
    for profile_id in running {
        ensure_profile_network_stack(&state.app_handle, profile_id)?;
    }
    Ok(())
}

fn collect_running_profile_ids(state: &AppState) -> Result<Vec<Uuid>, String> {
    let launched = state
        .launched_processes
        .lock()
        .map_err(|_| "launch map lock poisoned".to_string())?;
    let running = launched
        .iter()
        .filter_map(|(profile_id, pid)| {
            if is_pid_running(*pid) {
                Some(*profile_id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    Ok(running)
}

fn hydrate_dns_blocklists_from_global_security(
    state: &AppState,
    payload: &mut DnsTabPayload,
) -> Result<(), String> {
    global_blocklists::hydrate_dns_blocklists_from_global_security_impl(state, payload)
}

fn validate_connection_template_request(
    request: &SaveConnectionTemplateRequest,
) -> Result<(), String> {
    templates::validate_connection_template_request_impl(request)
}

fn validate_connection_template(template: &ConnectionTemplate) -> Result<(), String> {
    templates::validate_connection_template_impl(template)
}

fn build_template_id(
    seed: &str,
    existing: &std::collections::BTreeMap<String, ConnectionTemplate>,
) -> String {
    templates::build_template_id_impl(seed, existing)
}

fn build_nodes_from_request(
    request: &SaveConnectionTemplateRequest,
) -> Result<Vec<ConnectionNode>, String> {
    templates::build_nodes_from_request_impl(request)
}

fn sync_legacy_primary_fields(template: &mut ConnectionTemplate) {
    templates::sync_legacy_primary_fields_impl(template)
}

fn now_epoch_ms() -> u128 {
    templates::now_epoch_ms_impl()
}

fn test_connection_template_impl(
    template: &ConnectionTemplate,
) -> Result<ConnectionHealthView, String> {
    templates::test_connection_template_impl_impl(template)
}
