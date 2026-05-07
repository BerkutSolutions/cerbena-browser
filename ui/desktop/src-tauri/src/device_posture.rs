use std::{collections::BTreeSet, fs, path::Path};

use browser_profile::ProfileMetadata;
use serde::{Deserialize, Serialize};

use crate::{profile_security::assess_profile, state::AppState};

pub const ERR_CONFIRM_REQUIRED_PREFIX: &str = "device_posture.confirm_required";
pub const ERR_REFUSED_PREFIX: &str = "device_posture.refused";

const SEVERE_PROCESS_MARKERS: &[&str] = &[
    "fiddler",
    "burp",
    "wireshark",
    "charles",
    "mitmproxy",
    "frida",
    "x64dbg",
    "ollydbg",
    "ida",
    "processhacker",
    "procmon",
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DevicePostureStore {
    pub latest_report: Option<DevicePostureReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePostureReport {
    pub report_id: String,
    pub checked_at_epoch_ms: u128,
    pub host_name: String,
    pub exe_path: String,
    pub status: String,
    pub reaction: String,
    pub findings: Vec<DevicePostureFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePostureFinding {
    pub code: String,
    pub severity: String,
    pub label_key: String,
    pub detail: String,
}

pub fn load_device_posture_store(path: &Path) -> Result<DevicePostureStore, String> {
    if !path.exists() {
        return Ok(DevicePostureStore::default());
    }
    let raw = fs::read(path).map_err(|e| format!("read device posture store: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("parse device posture store: {e}"))
}

pub fn persist_device_posture_store(path: &Path, store: &DevicePostureStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create posture dir: {e}"))?;
    }
    let bytes =
        serde_json::to_vec_pretty(store).map_err(|e| format!("serialize posture store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write posture store: {e}"))
}

pub fn get_or_refresh_device_posture(state: &AppState) -> Result<DevicePostureReport, String> {
    if let Some(existing) = state
        .device_posture_store
        .lock()
        .map_err(|_| "device posture store lock poisoned".to_string())?
        .latest_report
        .clone()
    {
        return Ok(existing);
    }
    refresh_device_posture(state)
}

pub fn refresh_device_posture(state: &AppState) -> Result<DevicePostureReport, String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let process_names = enumerate_process_names().unwrap_or_default();
    let report = build_report(&current_exe, &process_names);
    {
        let mut store = state
            .device_posture_store
            .lock()
            .map_err(|_| "device posture store lock poisoned".to_string())?;
        store.latest_report = Some(report.clone());
        let path = state.device_posture_store_path(&state.app_handle)?;
        persist_device_posture_store(&path, &store)?;
    }
    Ok(report)
}

pub fn enforce_launch_posture(
    state: &AppState,
    profile: &ProfileMetadata,
    acknowledgement_report_id: Option<&str>,
) -> Result<(), String> {
    let assessment = assess_profile(profile);
    if !assessment.protected_profile {
        return Ok(());
    }
    let mut report = refresh_device_posture(state)?;
    report.reaction = reaction_for_profile(&assessment.policy_level, &report.findings).to_string();
    {
        let mut store = state
            .device_posture_store
            .lock()
            .map_err(|_| "device posture store lock poisoned".to_string())?;
        store.latest_report = Some(report.clone());
        let path = state.device_posture_store_path(&state.app_handle)?;
        persist_device_posture_store(&path, &store)?;
    }
    match report.reaction.as_str() {
        "refuse" => Err(format!("{ERR_REFUSED_PREFIX}:{}", report.report_id)),
        "confirm" => {
            if acknowledgement_report_id == Some(report.report_id.as_str()) {
                Ok(())
            } else {
                Err(format!(
                    "{ERR_CONFIRM_REQUIRED_PREFIX}:{}",
                    report.report_id
                ))
            }
        }
        _ => Ok(()),
    }
}

fn build_report(current_exe: &Path, process_names: &[String]) -> DevicePostureReport {
    let mut findings = Vec::new();

    for key in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                findings.push(DevicePostureFinding {
                    code: format!("env_proxy_{}", key.to_ascii_lowercase()),
                    severity: "warning".to_string(),
                    label_key: "devicePosture.finding.proxyEnv".to_string(),
                    detail: format!("{key}={trimmed}"),
                });
            }
        }
    }

    if let Ok(session_name) = std::env::var("SESSIONNAME") {
        if session_name.to_ascii_uppercase().starts_with("RDP-") {
            findings.push(DevicePostureFinding {
                code: "remote_session".to_string(),
                severity: "warning".to_string(),
                label_key: "devicePosture.finding.remoteSession".to_string(),
                detail: session_name,
            });
        }
    }

    let temp_dir = std::env::temp_dir().to_string_lossy().to_ascii_lowercase();
    let exe_path = current_exe.to_string_lossy().to_string();
    let exe_path_lower = exe_path.to_ascii_lowercase();
    if exe_path_lower.contains(&temp_dir) || exe_path_lower.contains("\\appdata\\local\\temp\\") {
        findings.push(DevicePostureFinding {
            code: "executable_in_temp".to_string(),
            severity: "severe".to_string(),
            label_key: "devicePosture.finding.executableInTemp".to_string(),
            detail: exe_path.clone(),
        });
    }

    let suspicious = suspicious_processes(process_names);
    if !suspicious.is_empty() {
        findings.push(DevicePostureFinding {
            code: "suspicious_processes".to_string(),
            severity: "severe".to_string(),
            label_key: "devicePosture.finding.suspiciousProcesses".to_string(),
            detail: suspicious.join(", "),
        });
    }

    let status = max_severity(&findings).to_string();
    DevicePostureReport {
        report_id: format!("posture-{}", now_epoch_ms()),
        checked_at_epoch_ms: now_epoch_ms(),
        host_name: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown-host".to_string()),
        exe_path,
        status,
        reaction: "allow".to_string(),
        findings,
    }
}

fn reaction_for_profile(policy_level: &str, findings: &[DevicePostureFinding]) -> &'static str {
    let has_severe = findings.iter().any(|item| item.severity == "severe");
    let has_warning = findings.iter().any(|item| item.severity == "warning");
    if has_severe {
        return "refuse";
    }
    if has_warning && policy_level == "maximum" {
        return "refuse";
    }
    if has_warning && policy_level == "high" {
        return "confirm";
    }
    if has_warning {
        return "warn";
    }
    "allow"
}

fn max_severity(findings: &[DevicePostureFinding]) -> &'static str {
    if findings.iter().any(|item| item.severity == "severe") {
        return "severe";
    }
    if findings.iter().any(|item| item.severity == "warning") {
        return "warning";
    }
    "healthy"
}

fn suspicious_processes(process_names: &[String]) -> Vec<String> {
    let mut out = BTreeSet::new();
    for name in process_names {
        let lower = name.to_ascii_lowercase();
        if SEVERE_PROCESS_MARKERS
            .iter()
            .any(|marker| lower.contains(marker))
        {
            out.insert(name.clone());
        }
    }
    out.into_iter().collect()
}

fn enumerate_process_names() -> Result<Vec<String>, String> {
    #[cfg(target_os = "windows")]
    {
        let mut command = std::process::Command::new("tasklist");
        command.args(["/fo", "csv", "/nh"]);
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let output = command
            .output()
            .map_err(|e| format!("tasklist failed: {e}"))?;
        if !output.status.success() {
            return Err(format!("tasklist exit status {}", output.status));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        return Ok(parse_tasklist_csv(&text));
    }
    #[allow(unreachable_code)]
    Ok(Vec::new())
}

#[cfg(any(target_os = "windows", test))]
fn parse_tasklist_csv(raw: &str) -> Vec<String> {
    raw.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            trimmed
                .strip_prefix('"')
                .and_then(|value| value.split("\",").next())
                .map(|value| value.trim().to_string())
        })
        .filter(|value| !value.is_empty())
        .collect()
}

fn now_epoch_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::{build_report, parse_tasklist_csv, reaction_for_profile, DevicePostureFinding};
    use std::path::Path;

    #[test]
    fn tasklist_parser_extracts_process_names() {
        let names = parse_tasklist_csv("\"chrome.exe\",\"1234\",\"Console\",\"1\",\"10,000 K\"\n");
        assert_eq!(names, vec!["chrome.exe".to_string()]);
    }

    #[test]
    fn high_policy_requires_confirmation_for_warning_findings() {
        let findings = vec![DevicePostureFinding {
            code: "remote_session".to_string(),
            severity: "warning".to_string(),
            label_key: "devicePosture.finding.remoteSession".to_string(),
            detail: "RDP-Tcp#1".to_string(),
        }];
        assert_eq!(reaction_for_profile("high", &findings), "confirm");
        assert_eq!(reaction_for_profile("maximum", &findings), "refuse");
    }

    #[test]
    fn severe_findings_mark_report_as_severe() {
        let report = build_report(
            Path::new("C:/Temp/cerbena.exe"),
            &["procmon.exe".to_string()],
        );
        assert_eq!(report.status, "severe");
        assert!(!report.findings.is_empty());
    }
}
