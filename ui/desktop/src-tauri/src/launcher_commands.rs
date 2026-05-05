use browser_api_local::{
    ApiRole, ConsentGrant, HomeDashboardModel, LaunchHookPolicy, PanicMode, SearchProvider,
};
use browser_network_policy::{BlocklistSource, DnsBlocklistUpdater};
use browser_profile::ProfileState;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path, time::Duration};
use tauri::{Emitter, State};
use uuid::Uuid;

use crate::{
    certificate_runtime::{display_certificate_issuer, load_certificate_metadata},
    device_posture::{get_or_refresh_device_posture, refresh_device_posture, DevicePostureReport},
    envelope::{ok, UiEnvelope},
    launch_sessions::revoke_launch_session,
    network_sandbox_lifecycle::stop_profile_network_stack,
    process_tracking::{
        clear_profile_process, terminate_process_tree, terminate_profile_processes,
    },
    state::{persist_link_routing_store_with_secret, AppState},
};

pub(crate) const RUNTIME_LOG_EVENT_NAME: &str = "runtime-log-appended";
const RUNTIME_LOG_LIMIT: usize = 1000;

fn append_runtime_log_file(state: &AppState, line: &str) {
    let Ok(path) = state.runtime_log_path(&state.app_handle) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut payload = String::with_capacity(line.len() + 1);
    payload.push_str(line);
    payload.push('\n');
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, payload.as_bytes()));
}

fn read_runtime_log_lines(state: &AppState) -> Result<Vec<String>, String> {
    let path = state.runtime_log_path(&state.app_handle)?;
    if !path.exists() {
        let logs = state
            .runtime_logs
            .lock()
            .map_err(|_| "runtime log lock poisoned".to_string())?;
        return Ok(logs.clone());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("read runtime log file: {e}"))?;
    let mut lines = raw.lines().map(|line| line.to_string()).collect::<Vec<_>>();
    if lines.len() > RUNTIME_LOG_LIMIT {
        let keep_from = lines.len() - RUNTIME_LOG_LIMIT;
        lines.drain(0..keep_from);
    }
    Ok(lines)
}

pub(crate) fn push_runtime_log(state: &AppState, entry: impl Into<String>) {
    let line = entry.into();
    if let Ok(mut logs) = state.runtime_logs.lock() {
        logs.push(line.clone());
        if logs.len() > RUNTIME_LOG_LIMIT {
            let overflow = logs.len() - RUNTIME_LOG_LIMIT;
            logs.drain(0..overflow);
        }
    }
    append_runtime_log_file(state, &line);
    let _ = state.app_handle.emit(RUNTIME_LOG_EVENT_NAME, line);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildHomeRequest {
    pub profile_id: String,
    pub dns_blocked: u64,
    pub tracker_blocked: u64,
    pub service_blocked: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanicRequest {
    pub profile_id: String,
    pub mode: PanicMode,
    pub retain_paths: Vec<String>,
    pub confirm_phrase: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultProfileRequest {
    pub profile_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTypeBindingRequest {
    pub link_type: String,
    pub profile_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTypeRequest {
    pub link_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchLinkRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTypeBindingView {
    pub link_type: String,
    pub label_key: String,
    pub profile_id: Option<String>,
    pub uses_global_default: bool,
    pub allow_global_default: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRoutingOverview {
    pub global_profile_id: Option<String>,
    pub supported_types: Vec<LinkTypeBindingView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchLinkResolution {
    pub status: String,
    pub link_type: String,
    pub url: String,
    pub target_profile_id: Option<String>,
    pub resolution_scope: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteHookRequest {
    pub policy: LaunchHookPolicy,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvePipRequest {
    pub requested: browser_api_local::PipMode,
    pub platform_supported: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSearchRequest {
    pub providers: Vec<SearchProvider>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultSearchRequest {
    pub provider_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailCheckRequest {
    pub token: String,
    pub role: ApiRole,
    pub operation: String,
    pub profile_id: String,
    pub granted_profile_ids: Vec<String>,
    pub grant: Option<ConsentGrant>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailCheckResult {
    pub rate_ok: bool,
    pub rbac_ok: bool,
    pub consent_ok: bool,
    pub scope_ok: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedCertificateInput {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub apply_globally: bool,
    #[serde(default)]
    pub profile_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManagedCertificateRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub issuer_name: Option<String>,
    #[serde(default)]
    pub subject_name: Option<String>,
    pub apply_globally: bool,
    pub profile_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedBlocklistInput {
    pub id: String,
    pub name: String,
    pub source_kind: String,
    pub source_value: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub domains: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManagedBlocklistRecord {
    pub id: String,
    pub name: String,
    pub source_kind: String,
    pub source_value: String,
    pub active: bool,
    pub domains: Vec<String>,
    pub updated_at_epoch: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSecuritySettingsRecord {
    pub startup_page: Option<String>,
    pub certificates: Vec<ManagedCertificateRecord>,
    pub blocked_domain_suffixes: Vec<String>,
    pub blocklists: Vec<ManagedBlocklistRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSecuritySettingsRequest {
    pub startup_page: Option<String>,
    #[serde(default)]
    pub certificates: Vec<ManagedCertificateInput>,
    pub blocked_domain_suffixes: Vec<String>,
    #[serde(default)]
    pub blocklists: Vec<ManagedBlocklistInput>,
}

#[tauri::command]
pub fn get_device_posture_report(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<DevicePostureReport>, String> {
    Ok(ok(correlation_id, get_or_refresh_device_posture(&state)?))
}

#[tauri::command]
pub fn refresh_device_posture_report(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<DevicePostureReport>, String> {
    Ok(ok(correlation_id, refresh_device_posture(&state)?))
}

const DEFAULT_DNS_BLOCKLISTS: &[(&str, &str)] = &[
    (
        "AdGuard DNS filter",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_1.txt",
    ),
    (
        "AdAway Default Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_2.txt",
    ),
    (
        "Phishing URL Blocklist (PhishTank and OpenPhish)",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_30.txt",
    ),
    (
        "Dandelion Sprout's Anti-Malware List",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_12.txt",
    ),
    (
        "HaGeZi's Badware Hoster Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_55.txt",
    ),
    (
        "HaGeZi's DNS Rebind Protection",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_71.txt",
    ),
    (
        "NoCoin Filter List",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_8.txt",
    ),
    (
        "HaGeZi's Threat Intelligence Feeds",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_44.txt",
    ),
    (
        "HaGeZi's URL Shortener Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_68.txt",
    ),
    (
        "Phishing Army",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_18.txt",
    ),
    (
        "Scam Blocklist by DurableNapkin",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_10.txt",
    ),
    (
        "ShadowWhisperer's Malware List",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_42.txt",
    ),
    (
        "Stalkerware Indicators List",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_31.txt",
    ),
    (
        "The Big List of Hacked Malware Web Sites",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_9.txt",
    ),
    (
        "uBlock filters - Badware risks",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_50.txt",
    ),
    (
        "Malicious URL Blocklist (URLHaus)",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_11.txt",
    ),
    (
        "AdGuard DNS Popup Hosts filter",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_59.txt",
    ),
    (
        "HaGeZi's Ultimate Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_49.txt",
    ),
    (
        "HaGeZi's Xiaomi Tracker Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_60.txt",
    ),
    (
        "HaGeZi's OPPO & Realme Tracker Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_66.txt",
    ),
    (
        "HaGeZi's Samsung Tracker Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_61.txt",
    ),
    (
        "HaGeZi's Vivo Tracker Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_65.txt",
    ),
    (
        "HaGeZi's Windows/Office Tracker Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_63.txt",
    ),
    (
        "Ukrainian Security Filter",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_62.txt",
    ),
    (
        "Dandelion Sprout's Anti Push Notifications",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_39.txt",
    ),
    (
        "HaGeZi's Apple Tracker Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_67.txt",
    ),
    (
        "HaGeZi's Gambling Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_47.txt",
    ),
    (
        "No Google",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_37.txt",
    ),
    (
        "Perflyst and Dandelion Sprout's Smart-TV Blocklist",
        "https://adguardteam.github.io/HostlistsRegistry/assets/filter_7.txt",
    ),
    (
        "anudeepND blacklist",
        "https://raw.githubusercontent.com/anudeepND/blacklist/master/adservers.txt",
    ),
    (
        "Ultimate Hosts Blacklist (UHB)",
        "https://raw.githubusercontent.com/Ultimate-Hosts-Blacklist/Ultimate.Hosts.Blacklist/master/hosts/hosts0",
    ),
];

const SUPPORTED_LINK_TYPES: &[(&str, &str, bool)] = &[
    ("http", "links.type.http", true),
    ("https", "links.type.https", true),
    ("ftp", "links.type.ftp", false),
    ("mailto", "links.type.mailto", false),
    ("irc", "links.type.irc", false),
    ("mms", "links.type.mms", false),
    ("news", "links.type.news", false),
    ("nntp", "links.type.nntp", false),
    ("sms", "links.type.sms", false),
    ("smsto", "links.type.smsto", false),
    ("snews", "links.type.snews", false),
    ("tel", "links.type.tel", false),
    ("urn", "links.type.urn", false),
    ("webcal", "links.type.webcal", false),
    ("magnet", "links.type.magnet", false),
    ("tg", "links.type.tg", false),
    ("discord", "links.type.discord", false),
    ("slack", "links.type.slack", false),
    ("zoommtg", "links.type.zoommtg", false),
    ("file:mht", "links.type.fileMht", false),
    ("file:mhtml", "links.type.fileMhtml", false),
    ("file:pdf", "links.type.filePdf", false),
    ("file:shtml", "links.type.fileShtml", false),
    ("file:svg", "links.type.fileSvg", false),
    ("file:xhtml", "links.type.fileXhtml", false),
];

#[tauri::command]
pub fn build_home_dashboard(
    state: State<AppState>,
    request: BuildHomeRequest,
    correlation_id: String,
) -> Result<UiEnvelope<HomeDashboardModel>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    let manager = state
        .manager
        .lock()
        .map_err(|_| "manager lock poisoned".to_string())?;
    let profile = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    drop(manager);

    let service = state
        .home_service
        .lock()
        .map_err(|_| "home service lock poisoned".to_string())?;
    let dashboard = service.build_dashboard(
        profile_id,
        request.dns_blocked,
        request.tracker_blocked,
        request.service_blocked,
        profile.state == ProfileState::Running,
    );
    Ok(ok(correlation_id, dashboard))
}

#[tauri::command]
pub fn panic_wipe_profile(
    state: State<AppState>,
    request: PanicRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    let manager = state
        .manager
        .lock()
        .map_err(|_| "manager lock poisoned".to_string())?;
    let profile = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    let mut retain_paths = merge_panic_retain_paths(&profile, &request.retain_paths);
    for path in extension_panic_retain_paths(&state, profile.id)? {
        if !retain_paths.iter().any(|item| item == &path) {
            retain_paths.push(path);
        }
    }
    drop(manager);

    let tracked_pid = {
        let launched = state
            .launched_processes
            .lock()
            .map_err(|_| "launch map lock poisoned".to_string())?;
        launched.get(&profile_id).copied()
    };
    let user_data_dir = state
        .profile_root
        .join(profile_id.to_string())
        .join("engine-profile");
    terminate_profile_processes(&user_data_dir);
    if let Some(pid) = tracked_pid {
        terminate_process_tree(pid);
        let _ = revoke_launch_session(&state, profile_id, Some(pid));
        stop_profile_network_stack(&state.app_handle, profile_id);
        clear_profile_process(&state.app_handle, profile_id, pid, false);
    }

    let manager = state
        .manager
        .lock()
        .map_err(|_| "manager lock poisoned".to_string())?;
    let service = state
        .panic_service
        .lock()
        .map_err(|_| "panic service lock poisoned".to_string())?;
    let summary = service
        .execute(
            &manager,
            profile_id,
            request.mode,
            profile.panic_protected_sites.clone(),
            retain_paths,
            &request.confirm_phrase,
            "ui",
        )
        .map_err(|e| e.to_string())?;
    Ok(ok(
        correlation_id,
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?,
    ))
}

fn merge_panic_retain_paths(
    profile: &browser_profile::ProfileMetadata,
    explicit_paths: &[String],
) -> Vec<String> {
    let mut merged = explicit_paths.to_vec();
    for domain in &profile.panic_protected_sites {
        let normalized = domain.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }
        for path in [
            format!("data/cookies/{normalized}"),
            format!("data/history/{normalized}"),
        ] {
            if !merged.iter().any(|item| item == &path) {
                merged.push(path);
            }
        }
    }
    merged
}

fn extension_panic_retain_paths(state: &AppState, profile_id: Uuid) -> Result<Vec<String>, String> {
    let library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let profile_key = profile_id.to_string();
    let has_preserve_extension = library.items.values().any(|item| {
        item.preserve_on_panic_wipe
            && item
                .assigned_profile_ids
                .iter()
                .any(|id| id == &profile_key)
    });
    let has_protected_extension_data = library.items.values().any(|item| {
        item.protect_data_from_panic_wipe
            && item
                .assigned_profile_ids
                .iter()
                .any(|id| id == &profile_key)
    });
    let mut retain = Vec::new();
    if has_preserve_extension || has_protected_extension_data {
        retain.push("extensions".to_string());
    }
    if has_protected_extension_data {
        retain.extend([
            "engine-profile/Default/Local Extension Settings".to_string(),
            "engine-profile/Default/Extension State".to_string(),
            "engine-profile/storage/default".to_string(),
            "engine-profile/browser-extension-data".to_string(),
        ]);
    }
    Ok(retain)
}

fn supported_link_type_label_key(link_type: &str) -> Option<&'static str> {
    SUPPORTED_LINK_TYPES
        .iter()
        .find(|(key, _, _)| *key == link_type)
        .map(|(_, label_key, _)| *label_key)
}

fn normalize_link_type(link_type: &str) -> Option<String> {
    let normalized = link_type.trim().to_ascii_lowercase();
    if supported_link_type_label_key(&normalized).is_some() {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_file_extension(extension: &str) -> String {
    match extension.trim().to_ascii_lowercase().as_str() {
        "xht" | "xhy" => "xhtml".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn detect_link_type(raw_url: &str) -> Result<String, String> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err("link URL is required".to_string());
    }
    if trimmed.starts_with("--") {
        return Err("CLI flags are not external links".to_string());
    }
    if let Ok(parsed) = reqwest::Url::parse(trimmed) {
        if parsed.scheme().eq_ignore_ascii_case("file") {
            let path = parsed.path().trim();
            if let Some(extension) = std::path::Path::new(path)
                .extension()
                .and_then(|value| value.to_str())
            {
                let file_type = format!("file:{}", normalize_file_extension(extension));
                return normalize_link_type(&file_type)
                    .ok_or_else(|| format!("unsupported link type: .{}", extension));
            }
        }
        return normalize_link_type(parsed.scheme())
            .ok_or_else(|| format!("unsupported link type: {}", parsed.scheme()));
    }
    if let Some(extension) = std::path::Path::new(trimmed)
        .extension()
        .and_then(|value| value.to_str())
    {
        let file_type = format!("file:{}", normalize_file_extension(extension));
        if let Some(normalized) = normalize_link_type(&file_type) {
            return Ok(normalized);
        }
    }
    if !trimmed.contains("://") {
        return Ok("https".to_string());
    }
    Err("invalid link URL".to_string())
}

fn link_routing_overview(state: &AppState) -> Result<LinkRoutingOverview, String> {
    let store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    let manager = state
        .manager
        .lock()
        .map_err(|_| "manager lock poisoned".to_string())?;
    let profile_ids = manager
        .list_profiles()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|profile| profile.id.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let global_profile_id = store
        .global_profile_id
        .clone()
        .filter(|profile_id| profile_ids.contains(profile_id));
    let supported_types = SUPPORTED_LINK_TYPES
        .iter()
        .map(|(link_type, label_key, allow_global_default)| {
            let bound = store
                .type_bindings
                .get(*link_type)
                .cloned()
                .filter(|profile_id| profile_ids.contains(profile_id));
            LinkTypeBindingView {
                link_type: (*link_type).to_string(),
                label_key: (*label_key).to_string(),
                uses_global_default: *allow_global_default
                    && bound.is_none()
                    && global_profile_id.is_some(),
                allow_global_default: *allow_global_default,
                profile_id: bound,
            }
        })
        .collect();
    Ok(LinkRoutingOverview {
        global_profile_id,
        supported_types,
    })
}

fn persist_link_routing(state: &AppState) -> Result<(), String> {
    let path = state.link_routing_store_path(&state.app_handle)?;
    let store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    persist_link_routing_store_with_secret(&path, &state.sensitive_store_secret, &store)
}

#[tauri::command]
pub fn set_default_profile_for_links(
    state: State<AppState>,
    request: DefaultProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "manager lock poisoned".to_string())?;
        let _ = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    }
    let mut store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    store.global_profile_id = Some(profile_id.to_string());
    drop(store);
    persist_link_routing(&state)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn clear_default_profile_for_links(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    store.global_profile_id = None;
    drop(store);
    persist_link_routing(&state)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn get_link_routing_overview(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<LinkRoutingOverview>, String> {
    Ok(ok(correlation_id, link_routing_overview(&state)?))
}

#[tauri::command]
pub fn save_link_type_profile_binding(
    state: State<AppState>,
    request: LinkTypeBindingRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    let link_type = normalize_link_type(&request.link_type)
        .ok_or_else(|| "unsupported link type".to_string())?;
    {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "manager lock poisoned".to_string())?;
        let _ = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    }
    let mut store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    store
        .type_bindings
        .insert(link_type, profile_id.to_string());
    drop(store);
    persist_link_routing(&state)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn remove_link_type_profile_binding(
    state: State<AppState>,
    request: LinkTypeRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let link_type = normalize_link_type(&request.link_type)
        .ok_or_else(|| "unsupported link type".to_string())?;
    let mut store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    store.type_bindings.remove(&link_type);
    drop(store);
    persist_link_routing(&state)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn dispatch_external_link(
    state: State<AppState>,
    request: DispatchLinkRequest,
    correlation_id: String,
) -> Result<UiEnvelope<DispatchLinkResolution>, String> {
    let link_type = detect_link_type(&request.url)?;
    let overview = link_routing_overview(&state)?;
    let row = overview
        .supported_types
        .iter()
        .find(|item| item.link_type == link_type)
        .ok_or_else(|| "unsupported link type".to_string())?;
    let (status, target_profile_id, resolution_scope) = if let Some(profile_id) = &row.profile_id {
        (
            "resolved".to_string(),
            Some(profile_id.clone()),
            Some("type".to_string()),
        )
    } else if row.allow_global_default {
        if let Some(profile_id) = &overview.global_profile_id {
            (
                "resolved".to_string(),
                Some(profile_id.clone()),
                Some("global".to_string()),
            )
        } else {
            ("prompt".to_string(), None, None)
        }
    } else if let Some(profile_id) = &overview.global_profile_id {
        (
            "prompt".to_string(),
            Some(profile_id.clone()),
            Some("global-disabled".to_string()),
        )
    } else {
        ("prompt".to_string(), None, None)
    };
    Ok(ok(
        correlation_id,
        DispatchLinkResolution {
            status,
            link_type,
            url: request.url.trim().to_string(),
            target_profile_id,
            resolution_scope,
        },
    ))
}

#[tauri::command]
pub fn consume_pending_external_link(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Option<String>>, String> {
    let mut pending = state
        .pending_external_link
        .lock()
        .map_err(|_| "pending external link lock poisoned".to_string())?;
    Ok(ok(correlation_id, pending.take()))
}

#[tauri::command]
pub fn execute_launch_hook(
    state: State<AppState>,
    request: ExecuteHookRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let service = state
        .launch_hook_service
        .lock()
        .map_err(|_| "launch hook lock poisoned".to_string())?;
    service.validate(&request.policy)?;
    drop(service);

    let timeout = Duration::from_millis(request.policy.timeout_ms.max(1));
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("launch hook client error: {e}"))?;
    let response = client
        .get(request.policy.url.clone())
        .send()
        .map_err(|e| format!("launch hook request failed: {e}"))?;
    let status = response.status();
    let executed = status.is_success() || status.is_redirection();
    let result = serde_json::json!({
        "accepted": true,
        "executed": executed,
        "statusCode": status.as_u16(),
        "messageKey": launch_hook_message_key(status, executed)
    });
    if !executed {
        return Err(format!(
            "launch hook endpoint returned unexpected status: {}",
            status
        ));
    }

    Ok(ok(
        correlation_id,
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?,
    ))
}

fn launch_hook_message_key(status: StatusCode, executed: bool) -> &'static str {
    if executed {
        return "launch_hook.executed";
    }
    if status.is_client_error() || status.is_server_error() {
        return "launch_hook.http_error";
    }
    "launch_hook.unexpected_status"
}

#[tauri::command]
pub fn resolve_pip_policy(
    state: State<AppState>,
    request: ResolvePipRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let service = state
        .pip_service
        .lock()
        .map_err(|_| "pip lock poisoned".to_string())?;
    let result = service.resolve(request.requested, request.platform_supported);
    Ok(ok(
        correlation_id,
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?,
    ))
}

#[tauri::command]
pub fn import_search_providers(
    state: State<AppState>,
    request: ImportSearchRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut registry = state
        .search_registry
        .lock()
        .map_err(|_| "search registry lock poisoned".to_string())?;
    registry.import_presets(request.providers)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn set_default_search_provider(
    state: State<AppState>,
    request: DefaultSearchRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let registry = state
        .search_registry
        .lock()
        .map_err(|_| "search registry lock poisoned".to_string())?;
    let provider = registry.set_default(&request.provider_id)?;
    Ok(ok(
        correlation_id,
        serde_json::to_string_pretty(provider).map_err(|e| e.to_string())?,
    ))
}

#[tauri::command]
pub fn run_guardrail_check(
    state: State<AppState>,
    request: GuardrailCheckRequest,
    correlation_id: String,
) -> Result<UiEnvelope<GuardrailCheckResult>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    let granted: Result<Vec<Uuid>, String> = request
        .granted_profile_ids
        .iter()
        .map(|v| Uuid::parse_str(v).map_err(|e| e.to_string()))
        .collect();
    let granted = granted?;

    let mut guardrails = state
        .security_guardrails
        .lock()
        .map_err(|_| "guardrails lock poisoned".to_string())?;

    let rate_ok = guardrails.enforce_rate_limit(&request.token).is_ok();
    let rbac_ok = guardrails
        .enforce_rbac(request.role, &request.operation)
        .is_ok();
    let consent_ok = guardrails
        .enforce_consent(
            request.grant.as_ref(),
            profile_id,
            &request.operation,
            now_unix_ms(),
        )
        .is_ok();
    let scope_ok = guardrails
        .enforce_no_scope_escalation(profile_id, &granted)
        .is_ok();

    Ok(ok(
        correlation_id,
        GuardrailCheckResult {
            rate_ok,
            rbac_ok,
            consent_ok,
            scope_ok,
        },
    ))
}

#[tauri::command]
pub fn append_runtime_log(
    state: State<AppState>,
    entry: String,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    push_runtime_log(state.inner(), entry);
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn read_runtime_logs(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<String>>, String> {
    Ok(ok(correlation_id, read_runtime_log_lines(&state)?))
}

#[tauri::command]
pub fn get_global_security_settings(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let data = load_global_security_record(&state)?;
    Ok(ok(
        correlation_id,
        serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?,
    ))
}

#[tauri::command]
pub fn save_global_security_settings(
    state: State<AppState>,
    request: GlobalSecuritySettingsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let existing = load_global_security_record(&state)?;
    let payload = GlobalSecuritySettingsRecord {
        startup_page: request
            .startup_page
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        certificates: normalize_certificates(&state, request.certificates, &existing.certificates)?,
        blocked_domain_suffixes: normalize_suffixes(request.blocked_domain_suffixes),
        blocklists: normalize_blocklists(&state, request.blocklists, &existing.blocklists)?,
    };
    persist_global_security_record(&state, &payload)?;
    cleanup_unused_managed_certificates(&state, &existing.certificates, &payload.certificates);

    if let Some(start_page) = payload.startup_page.clone() {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "manager lock poisoned".to_string())?;
        let profiles = manager.list_profiles().map_err(|e| e.to_string())?;
        drop(manager);
        for profile in profiles {
            if profile.default_start_page.is_some() {
                continue;
            }
            let manager = state
                .manager
                .lock()
                .map_err(|_| "manager lock poisoned".to_string())?;
            let _ = manager.update_profile(
                profile.id,
                browser_profile::PatchProfileInput {
                    default_start_page: Some(Some(start_page.to_string())),
                    ..browser_profile::PatchProfileInput::default()
                },
            );
        }
    }
    Ok(ok(correlation_id, true))
}

pub(crate) fn load_global_security_record(
    state: &AppState,
) -> Result<GlobalSecuritySettingsRecord, String> {
    let path = state.global_security_store_path(&state.app_handle)?;
    let legacy_path = state.global_security_legacy_path();
    load_global_security_record_from_paths(&path, &legacy_path, &state.sensitive_store_secret)
}

fn load_global_security_record_from_paths(
    path: &Path,
    legacy_path: &Path,
    secret_material: &str,
) -> Result<GlobalSecuritySettingsRecord, String> {
    let mut record = GlobalSecuritySettingsRecord {
        startup_page: None,
        certificates: Vec::new(),
        blocked_domain_suffixes: Vec::new(),
        blocklists: Vec::new(),
    };
    if path.exists() {
        record = crate::sensitive_store::load_sensitive_json_or_default(
            path,
            "global-security-store",
            secret_material,
        )?;
    } else if legacy_path.exists() {
        let raw = std::fs::read_to_string(legacy_path).map_err(|e| e.to_string())?;
        let parsed: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        record = parse_global_security_record_from_value(&parsed);
    }
    record.blocklists = merge_default_dns_blocklists(record.blocklists);
    Ok(record)
}

pub(crate) fn persist_global_security_record(
    state: &AppState,
    payload: &GlobalSecuritySettingsRecord,
) -> Result<(), String> {
    let path = state.global_security_store_path(&state.app_handle)?;
    let legacy_path = state.global_security_legacy_path();
    persist_global_security_record_to_paths(
        &path,
        &legacy_path,
        &state.sensitive_store_secret,
        payload,
    )
}

fn persist_global_security_record_to_paths(
    path: &Path,
    legacy_path: &Path,
    secret_material: &str,
    payload: &GlobalSecuritySettingsRecord,
) -> Result<(), String> {
    crate::sensitive_store::persist_sensitive_json(
        path,
        "global-security-store",
        secret_material,
        payload,
    )?;
    if legacy_path.exists() {
        let _ = std::fs::remove_file(legacy_path);
    }
    Ok(())
}

fn parse_global_security_record_from_value(parsed: &Value) -> GlobalSecuritySettingsRecord {
    GlobalSecuritySettingsRecord {
        startup_page: parsed
            .get("startup_page")
            .or_else(|| parsed.get("startupPage"))
            .and_then(Value::as_str)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        certificates: if let Some(items) = parsed.get("certificates").and_then(Value::as_array) {
            if items.iter().all(Value::is_string) {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|path| ManagedCertificateRecord {
                        id: slugify(path),
                        name: certificate_name_from_path(path),
                        path: path.trim().to_string(),
                        issuer_name: None,
                        subject_name: None,
                        apply_globally: true,
                        profile_ids: Vec::new(),
                    })
                    .collect()
            } else {
                serde_json::from_value::<Vec<ManagedCertificateRecord>>(json!(items))
                    .unwrap_or_default()
            }
        } else {
            Vec::new()
        },
        blocked_domain_suffixes: normalize_suffixes(
            parsed
                .get("blocked_domain_suffixes")
                .or_else(|| parsed.get("blockedDomainSuffixes"))
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        ),
        blocklists: serde_json::from_value::<Vec<ManagedBlocklistRecord>>(
            parsed
                .get("blocklists")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .unwrap_or_default(),
    }
}

fn normalize_certificates(
    state: &AppState,
    items: Vec<ManagedCertificateInput>,
    existing: &[ManagedCertificateRecord],
) -> Result<Vec<ManagedCertificateRecord>, String> {
    let root = state.managed_certificates_root(&state.app_handle)?;
    std::fs::create_dir_all(&root).map_err(|error| format!("create managed certificates dir: {error}"))?;
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let existing_by_id = existing
        .iter()
        .map(|item| (item.id.clone(), item.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    for item in items {
        let path = item.path.trim().to_string();
        if path.is_empty() || !seen.insert(path.clone()) {
            continue;
        }
        let id = if item.id.trim().is_empty() {
            slugify(&path)
        } else {
            slugify(&item.id)
        };
        let profile_ids = item
            .profile_ids
            .into_iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let existing_record = existing_by_id.get(&id);
        let managed_path = materialize_managed_certificate(
            &root,
            &path,
            &id,
            existing_record.map(|record| record.path.as_str()),
        )?;
        let (subject_name, issuer_name) = load_certificate_metadata(&managed_path).unwrap_or((None, None));
        out.push(ManagedCertificateRecord {
            id,
            name: if item.name.trim().is_empty() {
                certificate_display_name(
                    subject_name.clone(),
                    &managed_path.to_string_lossy(),
                )
            } else {
                item.name.trim().to_string()
            },
            path: managed_path.to_string_lossy().to_string(),
            issuer_name: display_certificate_issuer(issuer_name, subject_name.clone()),
            subject_name,
            apply_globally: item.apply_globally,
            profile_ids,
        });
    }
    Ok(out)
}

fn materialize_managed_certificate(
    root: &Path,
    source_path: &str,
    id: &str,
    existing_managed_path: Option<&str>,
) -> Result<std::path::PathBuf, String> {
    let trimmed = source_path.trim();
    if trimmed.is_empty() {
        return Err("certificate path is required".to_string());
    }
    let source = Path::new(trimmed);
    if !source.exists() {
        return Err(format!("certificate file not found: {}", source.display()));
    }

    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().trim_start_matches('.'))
        .filter(|value| !value.is_empty())
        .unwrap_or("crt");
    let target = root.join(format!("{id}.{extension}"));
    if source == target {
        return Ok(target);
    }

    if let Some(existing_path) = existing_managed_path {
        let existing = Path::new(existing_path);
        if existing == source && existing.exists() {
            return Ok(existing.to_path_buf());
        }
    }
    std::fs::copy(source, &target)
        .map_err(|error| format!("copy certificate {}: {error}", source.display()))?;
    Ok(target)
}

fn cleanup_unused_managed_certificates(
    state: &AppState,
    previous: &[ManagedCertificateRecord],
    next: &[ManagedCertificateRecord],
) {
    let Ok(root) = state.managed_certificates_root(&state.app_handle) else {
        return;
    };
    let keep = next
        .iter()
        .map(|item| item.path.trim().to_string())
        .collect::<std::collections::BTreeSet<_>>();
    for path in previous.iter().map(|item| item.path.trim()).filter(|value| !value.is_empty()) {
        let candidate = Path::new(path);
        if !candidate.starts_with(&root) {
            continue;
        }
        if keep.contains(path) {
            continue;
        }
        let _ = std::fs::remove_file(candidate);
    }
}

fn certificate_display_name(subject_name: Option<String>, fallback_path: &str) -> String {
    subject_name
        .and_then(|subject| {
            certificate_common_name(&subject)
                .or_else(|| {
                    let compact = subject.trim().to_string();
                    if compact.is_empty() {
                        None
                    } else {
                        Some(compact)
                    }
                })
        })
        .unwrap_or_else(|| certificate_name_from_path(fallback_path))
}

fn certificate_common_name(subject_name: &str) -> Option<String> {
    subject_name
        .split(',')
        .map(str::trim)
        .find_map(|part| {
            let (key, value) = part.split_once('=')?;
            if key.trim().eq_ignore_ascii_case("CN") {
                let clean = value.trim();
                if clean.is_empty() {
                    None
                } else {
                    Some(clean.to_string())
                }
            } else {
                None
            }
        })
}

fn normalize_blocklists(
    state: &AppState,
    items: Vec<ManagedBlocklistInput>,
    existing: &[ManagedBlocklistRecord],
) -> Result<Vec<ManagedBlocklistRecord>, String> {
    let updater = DnsBlocklistUpdater::new();
    let mut out = Vec::new();
    let mut seen_ids = std::collections::BTreeSet::new();
    let mut seen_sources = std::collections::BTreeSet::new();
    let defaults_applied = merge_default_dns_blocklist_inputs(items);
    let existing_by_id = existing
        .iter()
        .map(|item| (item.id.clone(), item.clone()))
        .collect::<BTreeMap<_, _>>();
    let existing_by_source = existing
        .iter()
        .map(|item| {
            (
                blocklist_source_key(&item.source_kind, &item.source_value),
                item.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let active_total = defaults_applied
        .iter()
        .filter(|item| item.active)
        .count()
        .max(1);
    let mut processed_active = 0usize;
    let started_at = std::time::Instant::now();
    for item in defaults_applied {
        let source_kind = normalize_source_kind(&item.source_kind);
        let source_value = item.source_value.trim().to_string();
        if matches!(source_kind.as_str(), "url" | "file") && source_value.is_empty() {
            continue;
        }
        let id_seed = if item.id.trim().is_empty() {
            if source_value.is_empty() {
                item.name.trim().to_string()
            } else {
                source_value.clone()
            }
        } else {
            item.id.clone()
        };
        let id = slugify(&id_seed);
        if id.is_empty() || !seen_ids.insert(id.clone()) {
            continue;
        }
        let source_key = blocklist_source_key(&source_kind, &source_value);
        if !seen_sources.insert(source_key.clone()) {
            continue;
        }
        let previous = existing_by_id
            .get(&id)
            .or_else(|| existing_by_source.get(&source_key));

        let fallback_name = if item.name.trim().is_empty() {
            previous
                .map(|record| record.name.clone())
                .unwrap_or_else(|| fallback_blocklist_name(&source_value))
        } else {
            item.name.trim().to_string()
        };

        if item.active {
            let source = blocklist_source_from_fields(&source_kind, &source_value, &item.domains)?;
            let _ = state.app_handle.emit(
                "dns-blocklist-progress",
                json!({
                    "stage": "downloading",
                    "name": fallback_name,
                    "progress": if active_total == 0 { 0.0 } else { (processed_active as f64 / active_total as f64) * 100.0 },
                    "processed": processed_active,
                    "total": active_total,
                    "elapsedSeconds": started_at.elapsed().as_secs_f64()
                }),
            );
            let snapshot = updater
                .update_from_source(&id, &source)
                .map_err(|e| e.to_string())?;
            processed_active += 1;
            let resolved_name = resolve_blocklist_title(&source_kind, &source_value)
                .unwrap_or_else(|| fallback_name.clone());
            out.push(ManagedBlocklistRecord {
                id,
                name: resolved_name,
                source_kind: source_kind.clone(),
                source_value: source_value.clone(),
                active: true,
                domains: snapshot.domains,
                updated_at_epoch: snapshot.updated_at_epoch,
            });
            let _ = state.app_handle.emit(
                "dns-blocklist-progress",
                json!({
                    "stage": "downloading",
                    "name": out.last().map(|value| value.name.clone()).unwrap_or_default(),
                    "progress": (processed_active as f64 / active_total as f64) * 100.0,
                    "processed": processed_active,
                    "total": active_total,
                    "elapsedSeconds": started_at.elapsed().as_secs_f64()
                }),
            );
            continue;
        }

        let domains = previous
            .map(|record| record.domains.clone())
            .filter(|values| !values.is_empty())
            .unwrap_or_else(|| normalize_inline_domains(item.domains));
        let updated_at_epoch = previous.map(|record| record.updated_at_epoch).unwrap_or(0);
        out.push(ManagedBlocklistRecord {
            id,
            name: fallback_name,
            source_kind,
            source_value,
            active: false,
            domains,
            updated_at_epoch,
        });
    }
    let _ = state.app_handle.emit(
        "dns-blocklist-progress",
        json!({
            "stage": "completed",
            "name": out.last().map(|value| value.name.clone()).unwrap_or_default(),
            "progress": 100.0,
            "processed": processed_active,
            "total": active_total,
            "elapsedSeconds": started_at.elapsed().as_secs_f64()
        }),
    );
    Ok(out)
}

fn merge_default_dns_blocklist_inputs(
    mut items: Vec<ManagedBlocklistInput>,
) -> Vec<ManagedBlocklistInput> {
    let mut seen_sources = items
        .iter()
        .map(|item| blocklist_source_key(&item.source_kind, &item.source_value))
        .collect::<std::collections::BTreeSet<_>>();
    for (name, url) in DEFAULT_DNS_BLOCKLISTS {
        let source_key = blocklist_source_key("url", url);
        if seen_sources.insert(source_key) {
            items.push(ManagedBlocklistInput {
                id: slugify(url),
                name: (*name).to_string(),
                source_kind: "url".to_string(),
                source_value: (*url).to_string(),
                active: false,
                domains: Vec::new(),
            });
        }
    }
    items
}

fn merge_default_dns_blocklists(
    existing: Vec<ManagedBlocklistRecord>,
) -> Vec<ManagedBlocklistRecord> {
    let mut by_source = existing
        .into_iter()
        .map(|item| {
            (
                blocklist_source_key(&item.source_kind, &item.source_value),
                item,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut merged = Vec::new();
    for (name, url) in DEFAULT_DNS_BLOCKLISTS {
        let key = blocklist_source_key("url", url);
        if let Some(mut current) = by_source.remove(&key) {
            current.id = if current.id.trim().is_empty() {
                slugify(url)
            } else {
                slugify(&current.id)
            };
            current.source_kind = "url".to_string();
            current.source_value = (*url).to_string();
            if current.name.trim().is_empty() {
                current.name = (*name).to_string();
            }
            merged.push(current);
        } else {
            merged.push(ManagedBlocklistRecord {
                id: slugify(url),
                name: (*name).to_string(),
                source_kind: "url".to_string(),
                source_value: (*url).to_string(),
                active: false,
                domains: Vec::new(),
                updated_at_epoch: 0,
            });
        }
    }
    merged.extend(by_source.into_values());
    merged
}

fn normalize_source_kind(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "url" => "url".to_string(),
        "file" => "file".to_string(),
        _ => "inline".to_string(),
    }
}

fn blocklist_source_key(source_kind: &str, source_value: &str) -> String {
    format!(
        "{}:{}",
        normalize_source_kind(source_kind),
        source_value.trim().to_ascii_lowercase()
    )
}

fn blocklist_source_from_fields(
    source_kind: &str,
    source_value: &str,
    domains: &[String],
) -> Result<BlocklistSource, String> {
    match source_kind {
        "url" => {
            if source_value.trim().is_empty() {
                return Err("blocklist URL is required".to_string());
            }
            Ok(BlocklistSource::RemoteUrl {
                url: source_value.to_string(),
                require_https: true,
                expected_sha256: None,
            })
        }
        "file" => {
            if source_value.trim().is_empty() {
                return Err("blocklist file path is required".to_string());
            }
            Ok(BlocklistSource::LocalFile {
                path: std::path::PathBuf::from(source_value),
            })
        }
        _ => Ok(BlocklistSource::InlineDomains {
            domains: normalize_inline_domains(domains.to_vec()),
        }),
    }
}

fn normalize_inline_domains(domains: Vec<String>) -> Vec<String> {
    domains
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn fallback_blocklist_name(source_value: &str) -> String {
    let trimmed = source_value.trim();
    if trimmed.is_empty() {
        return "DNS blocklist".to_string();
    }
    if let Ok(url) = reqwest::Url::parse(trimmed) {
        if let Some(segment) = url
            .path_segments()
            .and_then(|segments| segments.filter(|item| !item.is_empty()).last())
        {
            return segment.to_string();
        }
    }
    std::path::Path::new(trimmed)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| trimmed.to_string())
}

fn resolve_blocklist_title(source_kind: &str, source_value: &str) -> Option<String> {
    let text = match source_kind {
        "url" => {
            let url = reqwest::Url::parse(source_value.trim()).ok()?;
            let client = reqwest::blocking::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(8))
                .timeout(std::time::Duration::from_secs(20))
                .user_agent("Cerbena/0.1")
                .build()
                .ok()?;
            let response = client.get(url).send().ok()?;
            if !response.status().is_success() {
                return None;
            }
            response.text().ok()?
        }
        "file" => std::fs::read_to_string(source_value.trim()).ok()?,
        _ => return None,
    };
    extract_blocklist_title(&text)
}

fn extract_blocklist_title(content: &str) -> Option<String> {
    for raw_line in content.lines().take(120) {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let mut stripped = line;
        if stripped.starts_with('!') || stripped.starts_with('#') {
            stripped = stripped[1..].trim_start();
        }
        let lower = stripped.to_ascii_lowercase();
        if !lower.starts_with("title:") {
            continue;
        }
        let value = stripped
            .split_once(':')
            .map(|(_, right)| right.trim())
            .unwrap_or_default();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn normalize_suffixes(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|v| v.trim().trim_start_matches('.').to_lowercase())
        .filter(|v| !v.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn certificate_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|v| v.to_str())
        .map(|v| v.to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| path.to_string())
}

fn now_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensitive_store::derive_app_secret_material;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("cerbena-{label}-{unique}.json"))
    }

    fn temp_dir_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("cerbena-{label}-{unique}"))
    }

    #[test]
    fn extract_blocklist_title_reads_comment_title_marker() {
        let content = r#"
!
! Title: AdGuard DNS filter
! Description: sample
!
||example.com^
"#;
        let title = extract_blocklist_title(content).expect("title");
        assert_eq!(title, "AdGuard DNS filter");
    }

    #[test]
    fn merge_default_dns_blocklists_keeps_existing_activity() {
        let existing = vec![ManagedBlocklistRecord {
            id: "custom-id".to_string(),
            name: "Custom name".to_string(),
            source_kind: "url".to_string(),
            source_value: "https://adguardteam.github.io/HostlistsRegistry/assets/filter_1.txt"
                .to_string(),
            active: true,
            domains: vec!["example.com".to_string()],
            updated_at_epoch: 123,
        }];
        let merged = merge_default_dns_blocklists(existing);
        let item = merged
            .into_iter()
            .find(|value| {
                value.source_value
                    == "https://adguardteam.github.io/HostlistsRegistry/assets/filter_1.txt"
            })
            .expect("default list is present");
        assert!(item.active);
        assert_eq!(item.domains, vec!["example.com".to_string()]);
    }

    #[test]
    fn detect_link_type_rejects_cli_flags() {
        assert!(detect_link_type("--updater").is_err());
        assert!(detect_link_type("--updater-preview").is_err());
    }

    #[test]
    fn global_security_store_encrypts_startup_page_and_certificate_paths() {
        let path = temp_path("global-security-store");
        let legacy = temp_path("global-security-legacy");
        let app_data_dir = temp_dir_path("global-security-store-app-data");
        let binary_path = app_data_dir.join("cerbena.exe");
        let secret = derive_app_secret_material(&app_data_dir, &binary_path, "dev.cerbena.app")
            .expect("derive secret");
        let payload = GlobalSecuritySettingsRecord {
            startup_page: Some("https://duckduckgo.com".to_string()),
            certificates: vec![ManagedCertificateRecord {
                id: "cert-a".to_string(),
                name: "Cert A".to_string(),
                path: "C:/secret/cert.pem".to_string(),
                issuer_name: None,
                subject_name: None,
                apply_globally: true,
                profile_ids: Vec::new(),
            }],
            blocked_domain_suffixes: vec!["example".to_string()],
            blocklists: Vec::new(),
        };

        persist_global_security_record_to_paths(&path, &legacy, &secret, &payload)
            .expect("persist");
        let on_disk = fs::read_to_string(&path).expect("read");
        assert!(!on_disk.contains("duckduckgo"));
        assert!(!on_disk.contains("C:/secret/cert.pem"));

        let loaded = load_global_security_record_from_paths(&path, &legacy, &secret).expect("load");
        assert_eq!(loaded.startup_page, payload.startup_page);
        assert_eq!(
            loaded.certificates[0].path,
            "C:/secret/cert.pem".to_string()
        );

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(app_data_dir);
    }

    #[test]
    fn global_security_store_reads_legacy_plaintext_file() {
        let path = temp_path("global-security-store-new");
        let legacy = temp_path("global-security-legacy-old");
        let app_data_dir = temp_dir_path("global-security-legacy-app-data");
        let binary_path = app_data_dir.join("cerbena.exe");
        let secret = derive_app_secret_material(&app_data_dir, &binary_path, "dev.cerbena.app")
            .expect("derive secret");
        fs::write(
            &legacy,
            r#"{"startup_page":"https://legacy.test","certificates":["C:/legacy/cert.pem"],"blocked_domain_suffixes":["legacy"]}"#,
        )
        .expect("write legacy");

        let loaded =
            load_global_security_record_from_paths(&path, &legacy, &secret).expect("load legacy");
        assert_eq!(loaded.startup_page, Some("https://legacy.test".to_string()));
        assert_eq!(
            loaded.certificates[0].path,
            "C:/legacy/cert.pem".to_string()
        );

        let _ = fs::remove_file(legacy);
        let _ = fs::remove_dir_all(app_data_dir);
    }

    #[test]
    fn merge_panic_retain_paths_normalizes_domains_and_avoids_duplicates() {
        let profile = browser_profile::ProfileMetadata {
            id: uuid::Uuid::new_v4(),
            name: "Panic".to_string(),
            description: None,
            tags: Vec::new(),
            engine: browser_profile::Engine::Wayfern,
            state: browser_profile::ProfileState::Ready,
            default_start_page: None,
            default_search_provider: None,
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: true,
            panic_frame_color: None,
            panic_protected_sites: vec![
                " Example.COM ".to_string(),
                "example.com".to_string(),
                "".to_string(),
                "Sub.Domain.test".to_string(),
            ],
            crypto_version: 1,
            ephemeral_retain_paths: Vec::new(),
            created_at: "2026-05-01T00:00:00Z".to_string(),
            updated_at: "2026-05-01T00:00:00Z".to_string(),
        };

        let merged = merge_panic_retain_paths(
            &profile,
            &[
                "manual/path".to_string(),
                "data/cookies/example.com".to_string(),
                "data/history/sub.domain.test".to_string(),
            ],
        );

        assert_eq!(
            merged,
            vec![
                "manual/path".to_string(),
                "data/cookies/example.com".to_string(),
                "data/history/sub.domain.test".to_string(),
                "data/history/example.com".to_string(),
                "data/cookies/sub.domain.test".to_string(),
            ]
        );
    }
}
