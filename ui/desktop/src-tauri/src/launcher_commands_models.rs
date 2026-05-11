use super::*;

pub(crate) const RUNTIME_LOG_EVENT_NAME: &str = "runtime-log-appended";
pub(crate) const RUNTIME_LOG_LIMIT: usize = 1000;

pub(crate) fn append_runtime_log_file(state: &AppState, line: &str) {
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

pub(crate) fn read_runtime_log_lines(state: &AppState) -> Result<Vec<String>, String> {
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

pub(crate) const DEFAULT_DNS_BLOCKLISTS: &[(&str, &str)] = &[
    ("AdGuard DNS filter", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_1.txt"),
    ("AdAway Default Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_2.txt"),
    ("Phishing URL Blocklist (PhishTank and OpenPhish)", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_30.txt"),
    ("Dandelion Sprout's Anti-Malware List", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_12.txt"),
    ("HaGeZi's Badware Hoster Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_55.txt"),
    ("HaGeZi's DNS Rebind Protection", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_71.txt"),
    ("NoCoin Filter List", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_8.txt"),
    ("HaGeZi's Threat Intelligence Feeds", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_44.txt"),
    ("HaGeZi's URL Shortener Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_68.txt"),
    ("Phishing Army", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_18.txt"),
    ("Scam Blocklist by DurableNapkin", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_10.txt"),
    ("ShadowWhisperer's Malware List", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_42.txt"),
    ("Stalkerware Indicators List", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_31.txt"),
    ("The Big List of Hacked Malware Web Sites", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_9.txt"),
    ("uBlock filters - Badware risks", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_50.txt"),
    ("Malicious URL Blocklist (URLHaus)", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_11.txt"),
    ("AdGuard DNS Popup Hosts filter", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_59.txt"),
    ("HaGeZi's Ultimate Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_49.txt"),
    ("HaGeZi's Xiaomi Tracker Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_60.txt"),
    ("HaGeZi's OPPO & Realme Tracker Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_66.txt"),
    ("HaGeZi's Samsung Tracker Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_61.txt"),
    ("HaGeZi's Vivo Tracker Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_65.txt"),
    ("HaGeZi's Windows/Office Tracker Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_63.txt"),
    ("Ukrainian Security Filter", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_62.txt"),
    ("Dandelion Sprout's Anti Push Notifications", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_39.txt"),
    ("HaGeZi's Apple Tracker Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_67.txt"),
    ("HaGeZi's Gambling Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_47.txt"),
    ("No Google", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_37.txt"),
    ("Perflyst and Dandelion Sprout's Smart-TV Blocklist", "https://adguardteam.github.io/HostlistsRegistry/assets/filter_7.txt"),
    ("anudeepND blacklist", "https://raw.githubusercontent.com/anudeepND/blacklist/master/adservers.txt"),
    ("Ultimate Hosts Blacklist (UHB)", "https://raw.githubusercontent.com/Ultimate-Hosts-Blacklist/Ultimate.Hosts.Blacklist/master/hosts/hosts0"),
];
