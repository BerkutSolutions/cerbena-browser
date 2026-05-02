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
use flate2::read::ZlibDecoder;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::{
    envelope::{ok, UiEnvelope},
    launcher_commands::load_global_security_record,
    network_sandbox_lifecycle::{ensure_profile_network_stack, stop_profile_network_stack},
    network_sandbox::{
        resolve_global_network_sandbox_view, resolve_profile_network_sandbox_view,
        NetworkSandboxProfileView,
    },
    process_tracking::is_process_running as is_pid_running,
    state::{
        persist_network_store, AppState, ConnectionNode, ConnectionTemplate,
        NetworkGlobalRouteSettings,
    },
};

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
    let store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    let payload = store.vpn_proxy.get(&profile_id).cloned();
    let selected_template_id = store.profile_template_selection.get(&profile_id).cloned();
    let global_route = store.global_route_settings.clone();
    let global_template = global_route
        .default_template_id
        .as_ref()
        .and_then(|id| store.connection_templates.get(id))
        .cloned();
    let connection_templates = store
        .connection_templates
        .values()
        .cloned()
        .map(|mut template| {
            sync_legacy_primary_fields(&mut template);
            template
        })
        .collect::<Vec<_>>();
    drop(store);
    let sandbox = if let Ok(id) = Uuid::parse_str(&profile_id) {
        resolve_profile_network_sandbox_view(state.inner(), id)?
    } else {
        resolve_global_network_sandbox_view(state.inner(), global_template.as_ref())?
    };
    let json = serde_json::to_string_pretty(&NetworkStateView {
        payload,
        selected_template_id,
        connection_templates,
        global_route,
        sandbox,
    })
    .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, json))
}

#[tauri::command]
pub fn save_connection_template(
    state: State<AppState>,
    request: SaveConnectionTemplateRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ConnectionTemplate>, String> {
    validate_connection_template_request(&request)?;
    let nodes = build_nodes_from_request(&request)?;
    let mut store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    let id = request
        .template_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| build_template_id(&request.name, &store.connection_templates));
    let mut template = ConnectionTemplate {
        id: id.clone(),
        name: request.name.trim().to_string(),
        nodes,
        connection_type: String::new(),
        protocol: String::new(),
        host: None,
        port: None,
        username: None,
        password: None,
        bridges: None,
        updated_at_epoch_ms: now_epoch_ms(),
    };
    sync_legacy_primary_fields(&mut template);
    validate_connection_template(&template)?;
    store.connection_templates.insert(id, template.clone());
    let affected_profiles = store
        .profile_template_selection
        .iter()
        .filter(|(_, template_id)| *template_id == &template.id)
        .filter_map(|(profile_id, _)| Uuid::parse_str(profile_id).ok())
        .collect::<Vec<_>>();
    persist_store(&state, &store)?;
    drop(store);
    refresh_running_profiles_route_runtime(&state, &affected_profiles)?;
    Ok(ok(correlation_id, template))
}

#[tauri::command]
pub fn delete_connection_template(
    state: State<AppState>,
    request: DeleteConnectionTemplateRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    store.connection_templates.remove(&request.template_id);
    let affected_profiles = store
        .profile_template_selection
        .iter()
        .filter(|(_, value)| *value == &request.template_id)
        .filter_map(|(profile_id, _)| Uuid::parse_str(profile_id).ok())
        .collect::<Vec<_>>();
    store
        .profile_template_selection
        .retain(|_, value| value != &request.template_id);
    if store
        .global_route_settings
        .default_template_id
        .as_ref()
        .map(|value| value == &request.template_id)
        .unwrap_or(false)
    {
        store.global_route_settings.default_template_id = None;
    }
    persist_store(&state, &store)?;
    drop(store);
    for profile_id in &affected_profiles {
        stop_profile_network_stack(&state.app_handle, *profile_id);
    }
    refresh_running_profiles_route_runtime(&state, &affected_profiles)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn ping_connection_template(
    state: State<AppState>,
    request: TemplatePingRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ConnectionHealthView>, String> {
    let store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    let template = store
        .connection_templates
        .get(&request.template_id)
        .ok_or_else(|| "connection template not found".to_string())?
        .clone();
    drop(store);
    let health = test_connection_template_impl(&template)?;
    Ok(ok(correlation_id, health))
}

#[tauri::command]
pub fn test_connection_template_request(
    request: SaveConnectionTemplateRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ConnectionHealthView>, String> {
    validate_connection_template_request(&request)?;
    let mut template = ConnectionTemplate {
        id: "transient-check".to_string(),
        name: request.name.trim().to_string(),
        nodes: build_nodes_from_request(&request)?,
        connection_type: String::new(),
        protocol: String::new(),
        host: None,
        port: None,
        username: None,
        password: None,
        bridges: None,
        updated_at_epoch_ms: now_epoch_ms(),
    };
    sync_legacy_primary_fields(&mut template);
    let health = test_connection_template_impl(&template)?;
    Ok(ok(correlation_id, health))
}

#[tauri::command]
pub fn save_vpn_proxy_policy(
    state: State<AppState>,
    request: SaveVpnProxyRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    validate_vpn_proxy_tab(&request.payload)?;
    let mut store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    let route_mode = request.payload.route_mode.trim().to_ascii_lowercase();
    if !store.global_route_settings.global_vpn_enabled && route_mode != "direct" {
        let has_template = request
            .selected_template_id
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        if !has_template {
            return Err(
                "selected connection template is required for non-direct route mode".to_string(),
            );
        }
    }
    if let Some(template_id) = request
        .selected_template_id
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        if !store.connection_templates.contains_key(&template_id) {
            return Err("selected connection template not found".to_string());
        }
        store
            .profile_template_selection
            .insert(request.profile_id.clone(), template_id);
    } else {
        store.profile_template_selection.remove(&request.profile_id);
    }
    let profile_id = request.profile_id.clone();
    store.vpn_proxy.insert(request.profile_id, request.payload);
    persist_store(&state, &store)?;
    drop(store);

    if let Ok(profile_uuid) = Uuid::parse_str(&profile_id) {
        let maybe_pid = state
            .launched_processes
            .lock()
            .ok()
            .and_then(|map| map.get(&profile_uuid).copied());
        if let Some(pid) = maybe_pid {
            if is_pid_running(pid) {
                ensure_profile_network_stack(&state.app_handle, profile_uuid)?;
            }
        }
    }
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn save_global_route_settings(
    state: State<AppState>,
    request: SaveGlobalRouteSettingsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    let default_template_id = request
        .default_template_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(template_id) = default_template_id.as_deref() {
        if !store.connection_templates.contains_key(template_id) {
            return Err("default connection template not found".to_string());
        }
    }
    store.global_route_settings = NetworkGlobalRouteSettings {
        global_vpn_enabled: request.global_vpn_enabled,
        block_without_vpn: request.block_without_vpn,
        default_template_id,
    };
    persist_store(&state, &store)?;
    drop(store);

    let running_profiles = collect_running_profile_ids(&state)?;
    refresh_running_profiles_route_runtime(&state, &running_profiles)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn test_vpn_proxy_policy(
    payload: VpnProxyTabPayload,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let (proxy, vpn) = test_connect(&payload, 3_000)?;
    let result = serde_json::json!({
        "validated": true,
        "route_mode": payload.route_mode.clone(),
        "has_proxy": payload.proxy.is_some(),
        "has_vpn": payload.vpn.is_some(),
        "proxy_reachable": proxy.as_ref().map(|value| value.reachable),
        "vpn_reachable": vpn.as_ref().map(|value| value.connected),
        "proxy_message": proxy.as_ref().map(|value| value.message.clone()),
        "vpn_message": vpn.as_ref().map(|value| value.message.clone())
    });
    let json = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, json))
}

#[tauri::command]
pub fn save_dns_policy(
    state: State<AppState>,
    request: SaveDnsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut payload = request.payload;
    hydrate_dns_blocklists_from_global_security(&state, &mut payload)?;

    let catalog = state
        .service_catalog
        .lock()
        .map_err(|_| "catalog lock poisoned".to_string())?;
    validate_dns_tab(&payload, Some(&catalog))?;
    drop(catalog);

    let mut store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    store.dns.insert(request.profile_id, payload);
    persist_store(&state, &store)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn get_service_catalog(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let catalog = state
        .service_catalog
        .lock()
        .map_err(|_| "catalog lock poisoned".to_string())?;
    let json = serde_json::to_string_pretty(&catalog.state).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, json))
}

#[tauri::command]
pub fn set_service_block_all(
    state: State<AppState>,
    category: String,
    block_all: bool,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut catalog = state
        .service_catalog
        .lock()
        .map_err(|_| "catalog lock poisoned".to_string())?;
    catalog
        .set_category_block_all(&category, block_all)
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn set_service_allowed(
    state: State<AppState>,
    category: String,
    service: String,
    allowed: bool,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut catalog = state
        .service_catalog
        .lock()
        .map_err(|_| "catalog lock poisoned".to_string())?;
    catalog
        .set_service_allowed(&category, &service, allowed)
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn evaluate_network_policy_demo(
    policy: NetworkPolicy,
    request: PolicyRequestInput,
    correlation_id: String,
) -> Result<UiEnvelope<PolicyDecisionView>, String> {
    let runtime_request = PolicyRequest {
        has_profile_context: request.has_profile_context,
        vpn_up: request.vpn_up,
        target_domain: request.target_domain,
        target_service: request.target_service,
        tor_up: request.tor_up,
        dns_over_tor: request.dns_over_tor,
        active_route: request.active_route,
    };
    let engine = NetworkPolicyEngine;
    let decision = engine
        .evaluate(&policy, &runtime_request)
        .map_err(|e| e.to_string())?;
    Ok(ok(
        correlation_id,
        PolicyDecisionView {
            action: format!("{:?}", decision.action),
            reason_code: decision.reason_code,
            selected_route: match decision.selected_route {
                RouteMode::Direct => "direct".to_string(),
                RouteMode::Proxy => "proxy".to_string(),
                RouteMode::Vpn => "vpn".to_string(),
                RouteMode::Tor => "tor".to_string(),
                RouteMode::Hybrid => "hybrid".to_string(),
            },
            matched_rules: decision.matched_rules,
        },
    ))
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

#[derive(Debug, Clone)]
struct GlobalBlocklistRecord {
    source_kind: String,
    source_value: String,
    domains: Vec<String>,
    updated_at_epoch: u64,
}

fn hydrate_dns_blocklists_from_global_security(
    state: &AppState,
    payload: &mut DnsTabPayload,
) -> Result<(), String> {
    if payload.selected_blocklists.is_empty() {
        return Ok(());
    }
    let records = load_global_security_blocklists(state)?;
    if records.is_empty() {
        return Ok(());
    }
    let updater = DnsBlocklistUpdater::new();
    for list in &mut payload.selected_blocklists {
        if !list.domains.is_empty() {
            continue;
        }
        let Some(record) = records.get(&list.list_id) else {
            continue;
        };
        if !record.domains.is_empty() {
            list.domains = normalize_blocklist_domains(record.domains.clone());
            if list.updated_at_epoch == 0 {
                list.updated_at_epoch = record.updated_at_epoch;
            }
            continue;
        }
        let source = global_blocklist_source(record)?;
        let snapshot = updater
            .update_from_source(&list.list_id, &source)
            .map_err(|e| e.to_string())?;
        list.domains = snapshot.domains;
        list.updated_at_epoch = snapshot.updated_at_epoch;
    }
    Ok(())
}

fn load_global_security_blocklists(
    state: &AppState,
) -> Result<BTreeMap<String, GlobalBlocklistRecord>, String> {
    let items = load_global_security_record(state)?.blocklists;
    let mut out = BTreeMap::new();
    for item in items {
        let id = item.id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        out.insert(
            id.clone(),
            GlobalBlocklistRecord {
                source_kind: item.source_kind,
                source_value: item.source_value,
                domains: item.domains,
                updated_at_epoch: item.updated_at_epoch,
            },
        );
    }
    Ok(out)
}

fn global_blocklist_source(record: &GlobalBlocklistRecord) -> Result<BlocklistSource, String> {
    match record.source_kind.as_str() {
        "url" => {
            if record.source_value.trim().is_empty() {
                return Err("global blocklist URL is empty".to_string());
            }
            Ok(BlocklistSource::RemoteUrl {
                url: record.source_value.clone(),
                require_https: true,
                expected_sha256: None,
            })
        }
        "file" => {
            if record.source_value.trim().is_empty() {
                return Err("global blocklist file path is empty".to_string());
            }
            Ok(BlocklistSource::LocalFile {
                path: std::path::PathBuf::from(&record.source_value),
            })
        }
        _ => Ok(BlocklistSource::InlineDomains {
            domains: record.domains.clone(),
        }),
    }
}

fn normalize_blocklist_domains(domains: Vec<String>) -> Vec<String> {
    let mut unique = std::collections::BTreeSet::new();
    for domain in domains {
        let normalized = domain.trim().to_lowercase();
        if !normalized.is_empty() {
            unique.insert(normalized);
        }
    }
    unique.into_iter().collect()
}

fn validate_connection_template_request(
    request: &SaveConnectionTemplateRequest,
) -> Result<(), String> {
    if request.name.trim().is_empty() {
        return Err("connection template name is required".to_string());
    }
    let mut template = ConnectionTemplate {
        id: "validation-template".to_string(),
        name: request.name.trim().to_string(),
        nodes: build_nodes_from_request(request)?,
        connection_type: String::new(),
        protocol: String::new(),
        host: None,
        port: None,
        username: None,
        password: None,
        bridges: None,
        updated_at_epoch_ms: 0,
    };
    sync_legacy_primary_fields(&mut template);
    validate_connection_template(&template)
}

fn validate_connection_template(template: &ConnectionTemplate) -> Result<(), String> {
    let nodes = normalize_template_nodes(template);
    if nodes.is_empty() {
        return Err("at least one connection node is required".to_string());
    }
    if nodes.len() > 3 {
        return Err("maximum three connection nodes are supported".to_string());
    }
    for node in &nodes {
        validate_connection_node(node)?;
    }
    Ok(())
}

fn validate_connection_node(node: &ConnectionNode) -> Result<(), String> {
    match node.connection_type.as_str() {
        "vpn" => {
            let valid = ["wireguard", "openvpn", "amnezia"];
            if !valid.contains(&node.protocol.as_str()) {
                return Err("unsupported VPN protocol".to_string());
            }
            if node.protocol == "amnezia" {
                let amnezia_key = node
                    .settings
                    .get("amneziaKey")
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "amnezia key is required".to_string())?;
                let _ = parse_amnezia_key_endpoint(&amnezia_key)?;
            } else {
                validate_host_port(node.host.as_deref(), node.port)?;
            }
        }
        "proxy" => {
            validate_host_port(node.host.as_deref(), node.port)?;
            let valid = ["http", "socks4", "socks5"];
            if !valid.contains(&node.protocol.as_str()) {
                return Err("unsupported proxy protocol".to_string());
            }
        }
        "v2ray" => {
            validate_host_port(node.host.as_deref(), node.port)?;
            let valid = ["vmess", "vless", "trojan", "shadowsocks"];
            if !valid.contains(&node.protocol.as_str()) {
                return Err("unsupported V2Ray/XRay protocol".to_string());
            }
            if matches!(node.protocol.as_str(), "vmess" | "vless") {
                let uuid = node
                    .settings
                    .get("uuid")
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "uuid is required for vmess/vless".to_string())?;
                if !looks_like_uuid(&uuid) {
                    return Err("uuid format is invalid".to_string());
                }
            }
            if node.protocol == "vless" {
                let security_mode = node
                    .settings
                    .get("securityMode")
                    .map(String::as_str)
                    .map(str::trim)
                    .map(str::to_ascii_lowercase)
                    .unwrap_or_default();
                if security_mode == "reality" {
                    let reality_public_key = node
                        .settings
                        .get("realityPublicKey")
                        .map(String::as_str)
                        .map(str::trim)
                        .unwrap_or_default();
                    if reality_public_key.is_empty() {
                        return Err("vless reality requires pbk/public key".to_string());
                    }
                }
            }
            if matches!(node.protocol.as_str(), "trojan" | "shadowsocks")
                && node
                    .password
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
            {
                return Err("password is required for trojan/shadowsocks".to_string());
            }
        }
        "tor" => {
            let valid = ["obfs4", "snowflake", "meek", "none"];
            if !valid.contains(&node.protocol.as_str()) {
                return Err("unsupported TOR transport".to_string());
            }
            if node.protocol == "obfs4" && trim_option(node.bridges.clone()).is_none() {
                return Err("TOR bridges are required for obfs4".to_string());
            }
        }
        _ => {
            return Err("unsupported connection type".to_string());
        }
    }
    Ok(())
}

fn validate_host_port(host: Option<&str>, port: Option<u16>) -> Result<(), String> {
    if host.unwrap_or_default().trim().is_empty() {
        return Err("host is required".to_string());
    }
    if port.unwrap_or_default() == 0 {
        return Err("port must be non-zero".to_string());
    }
    Ok(())
}

fn build_template_id(
    seed: &str,
    existing: &std::collections::BTreeMap<String, ConnectionTemplate>,
) -> String {
    let mut base = seed
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if base.is_empty() {
        base = "connection-template".to_string();
    }
    let mut candidate = base.clone();
    let mut index = 2u32;
    while existing.contains_key(&candidate) {
        candidate = format!("{base}-{index}");
        index += 1;
    }
    candidate
}

fn build_nodes_from_request(
    request: &SaveConnectionTemplateRequest,
) -> Result<Vec<ConnectionNode>, String> {
    let mut nodes = if !request.nodes.is_empty() {
        request
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let node_id = node
                    .node_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("node-{}", index + 1));
                Ok(ConnectionNode {
                    id: node_id,
                    connection_type: normalize_connection_type(&node.connection_type),
                    protocol: normalize_protocol(node.protocol.trim()),
                    host: trim_option(node.host.clone()),
                    port: node.port,
                    username: trim_option(node.username.clone()),
                    password: trim_option(node.password.clone()),
                    bridges: trim_option(node.bridges.clone()),
                    settings: normalize_settings(node.settings.clone()),
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        vec![ConnectionNode {
            id: "node-1".to_string(),
            connection_type: normalize_connection_type(&request.connection_type),
            protocol: normalize_protocol(request.protocol.trim()),
            host: trim_option(request.host.clone()),
            port: request.port,
            username: trim_option(request.username.clone()),
            password: trim_option(request.password.clone()),
            bridges: trim_option(request.bridges.clone()),
            settings: BTreeMap::new(),
        }]
    };
    if nodes.is_empty() {
        return Err("at least one connection node is required".to_string());
    }
    if nodes.len() > 3 {
        return Err("maximum three connection nodes are supported".to_string());
    }
    for (index, node) in nodes.iter_mut().enumerate() {
        if node.id.trim().is_empty() {
            node.id = format!("node-{}", index + 1);
        }
        hydrate_amnezia_endpoint(node)?;
    }
    Ok(nodes)
}

fn trim_option(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn normalize_connection_type(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "vpn" => "vpn".to_string(),
        "v2ray" | "xray" => "v2ray".to_string(),
        "proxy" => "proxy".to_string(),
        "tor" => "tor".to_string(),
        _ => value.trim().to_lowercase(),
    }
}

fn normalize_protocol(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "ss" => "shadowsocks".to_string(),
        protocol => protocol.to_string(),
    }
}

fn normalize_settings(raw: BTreeMap<String, String>) -> BTreeMap<String, String> {
    raw.into_iter()
        .filter_map(|(key, value)| {
            let key = key.trim().to_string();
            if key.is_empty() {
                return None;
            }
            Some((key, value.trim().to_string()))
        })
        .collect()
}

fn normalize_template_nodes(template: &ConnectionTemplate) -> Vec<ConnectionNode> {
    if !template.nodes.is_empty() {
        return template
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| ConnectionNode {
                id: if node.id.trim().is_empty() {
                    format!("node-{}", index + 1)
                } else {
                    node.id.clone()
                },
                connection_type: normalize_connection_type(&node.connection_type),
                protocol: normalize_protocol(&node.protocol),
                host: trim_option(node.host.clone()),
                port: node.port,
                username: trim_option(node.username.clone()),
                password: trim_option(node.password.clone()),
                bridges: trim_option(node.bridges.clone()),
                settings: normalize_settings(node.settings.clone()),
            })
            .collect();
    }
    let connection_type = normalize_connection_type(&template.connection_type);
    let protocol = normalize_protocol(&template.protocol);
    if connection_type.is_empty() || protocol.is_empty() {
        return Vec::new();
    }
    vec![ConnectionNode {
        id: "node-1".to_string(),
        connection_type,
        protocol,
        host: trim_option(template.host.clone()),
        port: template.port,
        username: trim_option(template.username.clone()),
        password: trim_option(template.password.clone()),
        bridges: trim_option(template.bridges.clone()),
        settings: BTreeMap::new(),
    }]
}

fn sync_legacy_primary_fields(template: &mut ConnectionTemplate) {
    let nodes = normalize_template_nodes(template);
    if let Some(primary) = nodes.first() {
        template.connection_type = primary.connection_type.clone();
        template.protocol = primary.protocol.clone();
        template.host = primary.host.clone();
        template.port = primary.port;
        template.username = primary.username.clone();
        template.password = primary.password.clone();
        template.bridges = primary.bridges.clone();
        template.nodes = nodes;
    } else {
        template.connection_type.clear();
        template.protocol.clear();
        template.host = None;
        template.port = None;
        template.username = None;
        template.password = None;
        template.bridges = None;
        template.nodes.clear();
    }
}

fn looks_like_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (idx, byte) in bytes.iter().enumerate() {
        if [8, 13, 18, 23].contains(&idx) {
            if *byte != b'-' {
                return false;
            }
            continue;
        }
        if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn test_connection_template_impl(
    template: &ConnectionTemplate,
) -> Result<ConnectionHealthView, String> {
    let nodes = normalize_template_nodes(template);
    if nodes.is_empty() {
        return Err("connection template does not contain nodes".to_string());
    }
    let mut reachable = true;
    let mut latency_sum = 0u128;
    let mut latency_count = 0u128;
    let mut messages = Vec::new();
    for node in &nodes {
        let health = test_connection_node(node)?;
        if let Some(latency) = health.latency_ms {
            latency_sum += latency;
            latency_count += 1;
        }
        reachable = reachable && health.reachable;
        messages.push(format!(
            "[{}:{}] {}",
            node.connection_type, node.protocol, health.message
        ));
    }
    Ok(ConnectionHealthView {
        reachable,
        status: if reachable { "ok" } else { "unavailable" }.to_string(),
        latency_ms: if latency_count > 0 {
            Some(latency_sum / latency_count)
        } else {
            None
        },
        message: messages.join(" | "),
    })
}

fn test_connection_node(node: &ConnectionNode) -> Result<ConnectionHealthView, String> {
    match node.connection_type.as_str() {
        "tor" => {
            if node.protocol != "obfs4" {
                return Ok(ConnectionHealthView {
                    reachable: true,
                    status: "ok".to_string(),
                    latency_ms: None,
                    message: "TOR transport does not require static bridge endpoint".to_string(),
                });
            }
            let Some(first_bridge) = node
                .bridges
                .as_deref()
                .and_then(parse_first_bridge_endpoint)
            else {
                return Ok(ConnectionHealthView {
                    reachable: false,
                    status: "unavailable".to_string(),
                    latency_ms: None,
                    message: "TOR obfs4 bridge endpoint not found".to_string(),
                });
            };
            test_tcp_endpoint(
                first_bridge.0.as_str(),
                first_bridge.1,
                "TOR obfs4 bridge".to_string(),
            )
        }
        "vpn" => {
            if node.protocol == "amnezia" {
                let amnezia_key = node
                    .settings
                    .get("amneziaKey")
                    .map(String::as_str)
                    .unwrap_or_default();
                let (host, port, transport) = parse_amnezia_key_details(amnezia_key)?;
                if matches!(transport.as_deref(), Some("tcp")) {
                    return test_tcp_endpoint(
                        host.as_str(),
                        port,
                        "AMNEZIA TCP endpoint".to_string(),
                    );
                }
                return test_udp_endpoint(host.as_str(), port, "AMNEZIA UDP endpoint".to_string());
            }
            if node.protocol == "openvpn" {
                let host = node.host.clone().unwrap_or_default();
                let port = node.port.unwrap_or_default();
                let transport = node
                    .settings
                    .get("transport")
                    .map(String::as_str)
                    .map(str::trim)
                    .unwrap_or("udp");
                if transport.eq_ignore_ascii_case("udp") {
                    return test_udp_endpoint(
                        host.as_str(),
                        port,
                        "OPENVPN UDP endpoint".to_string(),
                    );
                }
                return test_tcp_endpoint(host.as_str(), port, "OPENVPN TCP endpoint".to_string());
            }
            let (host, port, label) = {
                let host = node.host.clone().unwrap_or_default();
                let port = node.port.unwrap_or_default();
                (
                    host,
                    port,
                    format!("{} endpoint", node.protocol.to_uppercase()),
                )
            };
            test_tcp_endpoint(host.as_str(), port, label)
        }
        "proxy" | "v2ray" => {
            let host = node.host.clone().unwrap_or_default();
            let port = node.port.unwrap_or_default();
            test_tcp_endpoint(
                host.as_str(),
                port,
                format!("{} endpoint", node.protocol.to_uppercase()),
            )
        }
        _ => Err("unsupported connection node type".to_string()),
    }
}

fn test_udp_endpoint(host: &str, port: u16, label: String) -> Result<ConnectionHealthView, String> {
    if host.trim().is_empty() || port == 0 {
        return Err("host and port are required for connectivity check".to_string());
    }
    let started = std::time::Instant::now();
    let mut addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve endpoint failed: {e}"))?;
    let Some(addr) = addrs.next() else {
        return Err("no endpoint address resolved".to_string());
    };
    let bind_addr = if addr.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let socket = UdpSocket::bind(bind_addr).map_err(|e| format!("udp bind failed: {e}"))?;
    socket
        .set_write_timeout(Some(Duration::from_millis(3_000)))
        .map_err(|e| format!("udp timeout setup failed: {e}"))?;
    socket
        .connect(addr)
        .map_err(|e| format!("udp connect failed: {e}"))?;
    let elapsed_ms = started.elapsed().as_millis().max(1);
    match socket.send(&[0u8]) {
        Ok(_) => Ok(ConnectionHealthView {
            reachable: true,
            status: "ok".to_string(),
            latency_ms: Some(elapsed_ms),
            message: format!("{label} probe sent"),
        }),
        Err(error) => Ok(ConnectionHealthView {
            reachable: false,
            status: "unavailable".to_string(),
            latency_ms: Some(elapsed_ms),
            message: format!("{label} probe failed: {error}"),
        }),
    }
}

fn test_tcp_endpoint(host: &str, port: u16, label: String) -> Result<ConnectionHealthView, String> {
    if host.trim().is_empty() || port == 0 {
        return Err("host and port are required for connectivity check".to_string());
    }
    let started = std::time::Instant::now();
    let mut addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve endpoint failed: {e}"))?;
    let Some(addr) = addrs.next() else {
        return Err("no endpoint address resolved".to_string());
    };
    let timeout = Duration::from_millis(3_000);
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(_) => Ok(ConnectionHealthView {
            reachable: true,
            status: "ok".to_string(),
            latency_ms: Some(started.elapsed().as_millis().max(1)),
            message: format!("{label} is reachable"),
        }),
        Err(error) => Ok(ConnectionHealthView {
            reachable: false,
            status: "unavailable".to_string(),
            latency_ms: Some(started.elapsed().as_millis().max(1)),
            message: format!("{label} is not reachable: {error}"),
        }),
    }
}

fn parse_first_bridge_endpoint(bridges: &str) -> Option<(String, u16)> {
    for line in bridges
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        if let Some((host, port)) = parts[1].rsplit_once(':') {
            if let Ok(port) = port.parse::<u16>() {
                return Some((host.to_string(), port));
            }
        }
    }
    None
}

fn parse_amnezia_key_endpoint(value: &str) -> Result<(String, u16), String> {
    let (host, port, _) = parse_amnezia_key_details(value)?;
    Ok((host, port))
}

fn parse_amnezia_key_details(value: &str) -> Result<(String, u16, Option<String>), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("amnezia key is required".to_string());
    }
    if looks_like_amnezia_conf(trimmed) {
        return parse_amnezia_conf_details(trimmed);
    }
    let encoded = match trimmed.get(0..6) {
        Some(prefix) if prefix.eq_ignore_ascii_case("vpn://") => {
            trimmed.get(6..).unwrap_or_default().trim()
        }
        _ => trimmed,
    };
    if encoded.is_empty() {
        return Err("amnezia key payload is empty".to_string());
    }

    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| URL_SAFE.decode(encoded))
        .map_err(|_| "amnezia key payload encoding is invalid".to_string())?;

    let inflated = if decoded.len() > 4 {
        inflate_zlib_to_string(&decoded[4..]).or_else(|_| inflate_zlib_to_string(&decoded))
    } else {
        inflate_zlib_to_string(&decoded)
    }
    .map_err(|_| "amnezia key payload compression is invalid".to_string())?;

    let json: serde_json::Value =
        serde_json::from_str(&inflated).map_err(|_| "amnezia key JSON is invalid".to_string())?;
    let endpoint = if let Some(endpoint) = extract_endpoint_from_json(&json) {
        endpoint
    } else if let (Some(host), Some(port)) = (
        extract_host_hint_from_json(&json),
        extract_port_hint_from_json(&json),
    ) {
        (host, port)
    } else {
        return Err("amnezia key does not contain endpoint".to_string());
    };
    let transport = extract_transport_hint_from_json(&json);
    Ok((endpoint.0, endpoint.1, transport))
}

fn looks_like_amnezia_conf(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("[interface]") && lower.contains("[peer]")
}

fn parse_amnezia_conf_details(value: &str) -> Result<(String, u16, Option<String>), String> {
    let sections = parse_ini_sections(value);
    let endpoint = sections
        .get("peer")
        .and_then(|peer| peer.get("endpoint"))
        .map(String::as_str)
        .and_then(parse_host_port_pair)
        .ok_or_else(|| "amnezia config does not contain endpoint".to_string())?;
    let transport = sections
        .get("interface")
        .and_then(|iface| {
            iface
                .get("protocol")
                .or_else(|| iface.get("transport"))
                .or_else(|| iface.get("transport_proto"))
        })
        .map(String::as_str)
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .filter(|value| value == "udp" || value == "tcp")
        .or_else(|| Some("udp".to_string()));
    Ok((endpoint.0, endpoint.1, transport))
}

fn parse_ini_sections(value: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut sections = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut current_section = String::new();
    for raw_line in value.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        if current_section.is_empty() {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        sections.entry(current_section.clone()).or_default().insert(
            key.trim().to_ascii_lowercase(),
            raw_value.trim().to_string(),
        );
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    fn build_amnezia_key(payload: &str) -> String {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(payload.as_bytes())
            .expect("write amnezia payload");
        let compressed = encoder.finish().expect("finish compression");

        let mut framed = Vec::with_capacity(compressed.len() + 4);
        let len = payload.len() as u32;
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(&compressed);

        format!("vpn://{}", URL_SAFE_NO_PAD.encode(framed))
    }

    #[test]
    fn parse_amnezia_key_endpoint_extracts_host_and_port() {
        let key = build_amnezia_key(r#"{"endpoint":"demo.example:443"}"#);
        let endpoint = parse_amnezia_key_endpoint(&key).expect("parse endpoint");
        assert_eq!(endpoint.0, "demo.example");
        assert_eq!(endpoint.1, 443);
    }

    #[test]
    fn parse_amnezia_key_endpoint_supports_split_host_and_port() {
        let key = build_amnezia_key(
            r#"{"hostName":"91.186.212.196","containers":[{"awg":{"port":"44017"}}]}"#,
        );
        let endpoint = parse_amnezia_key_endpoint(&key).expect("parse split endpoint");
        assert_eq!(endpoint.0, "91.186.212.196");
        assert_eq!(endpoint.1, 44017);
    }

    #[test]
    fn parse_amnezia_key_details_extracts_udp_transport() {
        let key = build_amnezia_key(
            r#"{"hostName":"91.186.212.196","containers":[{"awg":{"port":"44017","transport_proto":"udp"}}]}"#,
        );
        let (_, _, transport) = parse_amnezia_key_details(&key).expect("parse amnezia details");
        assert_eq!(transport.as_deref(), Some("udp"));
    }

    #[test]
    fn parse_amnezia_conf_details_extracts_endpoint_and_udp_transport() {
        let conf = r#"
[Interface]
Address = 10.8.1.84/32
PrivateKey = PRIVATE
Jc = 4

[Peer]
PublicKey = PUBLIC
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = 5.129.225.48:32542
"#;
        let (host, port, transport) =
            parse_amnezia_key_details(conf).expect("parse amnezia conf details");
        assert_eq!(host, "5.129.225.48");
        assert_eq!(port, 32542);
        assert_eq!(transport.as_deref(), Some("udp"));
    }

    #[test]
    fn hydrate_amnezia_endpoint_fills_host_and_port() {
        let key = build_amnezia_key(r#"{"endpoint":"1.2.3.4:6553"}"#);
        let mut node = ConnectionNode {
            id: "node-1".to_string(),
            connection_type: "vpn".to_string(),
            protocol: "amnezia".to_string(),
            host: None,
            port: None,
            username: None,
            password: None,
            bridges: None,
            settings: BTreeMap::from([(String::from("amneziaKey"), key)]),
        };

        hydrate_amnezia_endpoint(&mut node).expect("hydrate endpoint");
        assert_eq!(node.host.as_deref(), Some("1.2.3.4"));
        assert_eq!(node.port, Some(6553));
    }

    #[test]
    fn hydrate_amnezia_endpoint_supports_awg_conf() {
        let conf = r#"
[Interface]
Address = 10.8.1.84/32
PrivateKey = PRIVATE

[Peer]
PublicKey = PUBLIC
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = 5.129.225.48:32542
"#;
        let mut node = ConnectionNode {
            id: "node-1".to_string(),
            connection_type: "vpn".to_string(),
            protocol: "amnezia".to_string(),
            host: None,
            port: None,
            username: None,
            password: None,
            bridges: None,
            settings: BTreeMap::from([(String::from("amneziaKey"), conf.to_string())]),
        };
        hydrate_amnezia_endpoint(&mut node).expect("hydrate endpoint from conf");
        assert_eq!(node.host.as_deref(), Some("5.129.225.48"));
        assert_eq!(node.port, Some(32542));
    }
}

fn hydrate_amnezia_endpoint(node: &mut ConnectionNode) -> Result<(), String> {
    if node.connection_type != "vpn" || node.protocol != "amnezia" {
        return Ok(());
    }
    let amnezia_key = node
        .settings
        .get("amneziaKey")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "amnezia key is required".to_string())?;
    let (host, port) = parse_amnezia_key_endpoint(&amnezia_key)?;
    node.host = Some(host);
    node.port = Some(port);
    Ok(())
}

fn inflate_zlib_to_string(bytes: &[u8]) -> Result<String, String> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut output = String::new();
    decoder
        .read_to_string(&mut output)
        .map_err(|_| "failed to inflate".to_string())?;
    if output.trim().is_empty() {
        return Err("inflated payload is empty".to_string());
    }
    Ok(output)
}

fn extract_endpoint_from_json(value: &serde_json::Value) -> Option<(String, u16)> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(endpoint) = map.get("endpoint").and_then(serde_json::Value::as_str) {
                if let Some(parsed) = parse_host_port_pair(endpoint) {
                    return Some(parsed);
                }
            }

            let host = object_host_hint(map);
            let port = map.get("port").and_then(parse_json_port_value);
            if let (Some(host), Some(port)) = (host, port) {
                return Some((host, port));
            }

            if let Some(server) = map.get("server").and_then(serde_json::Value::as_str) {
                if let Some(parsed) = parse_host_port_pair(server) {
                    return Some(parsed);
                }
            }

            for nested in map.values() {
                if let Some(parsed) = extract_endpoint_from_json(nested) {
                    return Some(parsed);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(parsed) = extract_endpoint_from_json(item) {
                    return Some(parsed);
                }
            }
            None
        }
        _ => None,
    }
}

fn object_host_hint(map: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    for key in ["host", "hostname", "host_name", "hostName", "server"] {
        if let Some(value) = map.get(key).and_then(serde_json::Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                if let Some((host, port)) = parse_host_port_pair(trimmed) {
                    if port > 0 {
                        return Some(host);
                    }
                }
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn extract_host_hint_from_json(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(host) = object_host_hint(map) {
                return Some(host);
            }
            for nested in map.values() {
                if let Some(host) = extract_host_hint_from_json(nested) {
                    return Some(host);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(host) = extract_host_hint_from_json(item) {
                    return Some(host);
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_port_hint_from_json(value: &serde_json::Value) -> Option<u16> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(port) = map
                .get("endpoint_port")
                .and_then(parse_json_port_value)
                .or_else(|| map.get("remote_port").and_then(parse_json_port_value))
                .or_else(|| map.get("port").and_then(parse_json_port_value))
            {
                return Some(port);
            }
            for nested in map.values() {
                if let Some(port) = extract_port_hint_from_json(nested) {
                    return Some(port);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(port) = extract_port_hint_from_json(item) {
                    return Some(port);
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_transport_hint_from_json(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            for (key, nested) in map {
                if matches!(
                    key.as_str(),
                    "transport_proto" | "transport" | "proto" | "protocol"
                ) {
                    if let Some(raw) = nested.as_str() {
                        let normalized = raw.trim().to_lowercase();
                        if normalized == "udp" || normalized == "tcp" {
                            return Some(normalized);
                        }
                    }
                }
            }
            for nested in map.values() {
                if let Some(transport) = extract_transport_hint_from_json(nested) {
                    return Some(transport);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(transport) = extract_transport_hint_from_json(item) {
                    return Some(transport);
                }
            }
            None
        }
        _ => None,
    }
}

fn parse_json_port_value(value: &serde_json::Value) -> Option<u16> {
    match value {
        serde_json::Value::Number(number) => number
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value > 0),
        serde_json::Value::String(raw) => raw.trim().parse::<u16>().ok().filter(|value| *value > 0),
        _ => None,
    }
}

fn parse_host_port_pair(raw: &str) -> Option<(String, u16)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('[') {
        let end = trimmed.find(']')?;
        let host = trimmed[1..end].trim();
        let rest = trimmed[end + 1..].trim();
        let port = rest.strip_prefix(':')?.trim().parse::<u16>().ok()?;
        if !host.is_empty() && port > 0 {
            return Some((host.to_string(), port));
        }
    }

    let (host, port_raw) = trimmed.rsplit_once(':')?;
    let host = host.trim();
    let port = port_raw.trim().parse::<u16>().ok()?;
    if host.is_empty() || port == 0 {
        return None;
    }
    Some((host.to_string(), port))
}
