use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    route_runtime::runtime_session_snapshot,
    state::{app_local_data_root, AppState},
};

const PROFILE_LOG_RETENTION_MS: u64 = 24 * 60 * 60 * 1000;
const PROFILE_LOG_MAX_ENTRIES: usize = 4_000;
const LIVE_FILE_TAIL_LINES: usize = 80;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ProfileLogStore {
    pub entries: Vec<ProfileLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileLogEntry {
    pub timestamp_epoch_ms: u64,
    pub profile_id: String,
    pub source: String,
    pub message: String,
}

pub fn load_profile_log_store(path: &Path) -> Result<ProfileLogStore, String> {
    if !path.exists() {
        return Ok(ProfileLogStore::default());
    }
    let bytes = fs::read(path).map_err(|e| format!("read profile log store: {e}"))?;
    let mut store: ProfileLogStore =
        serde_json::from_slice(&bytes).map_err(|e| format!("parse profile log store: {e}"))?;
    prune_store(&mut store);
    Ok(store)
}

pub fn append_profile_log(
    app_handle: &AppHandle,
    profile_id: Uuid,
    source: &str,
    message: impl Into<String>,
) {
    let state = app_handle.state::<AppState>();
    let mut store = match state.profile_logs.lock() {
        Ok(value) => value,
        Err(_) => return,
    };
    store.entries.push(ProfileLogEntry {
        timestamp_epoch_ms: now_epoch_ms(),
        profile_id: profile_id.to_string(),
        source: source.to_string(),
        message: message.into(),
    });
    prune_store(&mut store);
    if let Ok(path) = state.profile_log_store_path(app_handle) {
        let _ = persist_profile_log_store(&path, &store);
    }
}

pub fn read_profile_log_lines(app_handle: &AppHandle, profile_id: Uuid) -> Result<Vec<String>, String> {
    let state = app_handle.state::<AppState>();
    let mut lines = Vec::new();
    let profile_key = profile_id.to_string();

    {
        let mut store = state
            .profile_logs
            .lock()
            .map_err(|_| "profile log store lock poisoned".to_string())?;
        let before_len = store.entries.len();
        prune_store(&mut store);
        if store.entries.len() != before_len {
            let path = state.profile_log_store_path(app_handle)?;
            persist_profile_log_store(&path, &store)?;
        }
        let stored = store
            .entries
            .iter()
            .filter(|entry| entry.profile_id == profile_key)
            .map(format_entry_line)
            .collect::<Vec<_>>();
        if !stored.is_empty() {
            lines.push("=== Launcher log (last 24h) ===".to_string());
            lines.extend(stored);
        }
    }

    let runtime_lines = state
        .runtime_logs
        .lock()
        .map_err(|_| "runtime log lock poisoned".to_string())?
        .iter()
        .filter(|line| line.contains(&profile_key))
        .cloned()
        .collect::<Vec<_>>();
    if !runtime_lines.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push("=== Current launcher runtime log ===".to_string());
        lines.extend(runtime_lines);
    }

    let artifact_sections = collect_live_runtime_sections(state.inner(), profile_id);
    if !artifact_sections.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.extend(artifact_sections);
    }

    if lines.is_empty() {
        lines.push("no profile logs available for the last 24 hours".to_string());
    }

    Ok(lines)
}

fn collect_live_runtime_sections(state: &AppState, profile_id: Uuid) -> Vec<String> {
    let mut sections = Vec::new();
    let runtime_dir = state.network_runtime_root.join(profile_id.to_string());
    let profile_root = state.profile_root.join(profile_id.to_string());
    let candidates = [
        "sing-box-route.log",
        "openvpn-route.log",
        "container-openvpn.log",
    ];

    for file_name in candidates {
        let path = runtime_dir.join(file_name);
        if !path.exists() {
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let tail = tail_lines(&content, LIVE_FILE_TAIL_LINES);
        if tail.is_empty() {
            continue;
        }
        sections.push(format!("=== Live runtime file: {file_name} ==="));
        sections.extend(tail.lines().map(ToString::to_string));
        sections.push(String::new());
    }

    let browser_log_candidates = [profile_root.join("engine-profile").join("chrome_debug.log")];

    for path in browser_log_candidates {
        if !path.exists() {
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let tail = tail_lines(&content, LIVE_FILE_TAIL_LINES);
        if tail.is_empty() {
            continue;
        }
        sections.push(format!(
            "=== Browser log file: {} ===",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("browser.log")
        ));
        sections.extend(tail.lines().map(ToString::to_string));
        sections.push(String::new());
    }

    if let Some(snapshot) = runtime_session_snapshot(&state.app_handle, profile_id) {
        if let Some(container_name) = snapshot.container_name {
            let docker_tail = read_docker_logs(&container_name);
            if !docker_tail.is_empty() {
                sections.push(format!("=== Docker log: {container_name} ==="));
                sections.extend(docker_tail.lines().map(ToString::to_string));
                sections.push(String::new());
            }
        }
    }

    while sections.last().is_some_and(|line| line.is_empty()) {
        sections.pop();
    }
    sections
}

fn read_docker_logs(container_name: &str) -> String {
    let output = hidden_command("docker")
        .args(["logs", "--tail", "80", container_name])
        .output();
    output
        .ok()
        .filter(|value| value.status.success())
        .map(|value| {
            let stderr = String::from_utf8_lossy(&value.stderr).trim().to_string();
            if !stderr.is_empty() {
                return stderr;
            }
            String::from_utf8_lossy(&value.stdout).trim().to_string()
        })
        .unwrap_or_default()
}

fn hidden_command(program: &str) -> std::process::Command {
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new(program);
    #[cfg(not(target_os = "windows"))]
    let command = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

fn persist_profile_log_store(path: &PathBuf, store: &ProfileLogStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create profile log dir: {e}"))?;
    }
    let bytes =
        serde_json::to_vec_pretty(store).map_err(|e| format!("serialize profile log store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write profile log store: {e}"))
}

fn prune_store(store: &mut ProfileLogStore) {
    let cutoff = now_epoch_ms().saturating_sub(PROFILE_LOG_RETENTION_MS);
    store
        .entries
        .retain(|entry| entry.timestamp_epoch_ms >= cutoff && !entry.profile_id.trim().is_empty());
    if store.entries.len() > PROFILE_LOG_MAX_ENTRIES {
        let overflow = store.entries.len() - PROFILE_LOG_MAX_ENTRIES;
        store.entries.drain(0..overflow);
    }
}

fn format_entry_line(entry: &ProfileLogEntry) -> String {
    format!(
        "[{}][{}] {}",
        entry.timestamp_epoch_ms, entry.source, entry.message
    )
}

fn tail_lines(value: &str, limit: usize) -> String {
    let lines = value.lines().rev().take(limit).collect::<Vec<_>>();
    lines.into_iter().rev().collect::<Vec<_>>().join("\n")
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn profile_log_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data = app_local_data_root(app)?;
    Ok(app_data.join("profile_runtime_logs.json"))
}

#[cfg(test)]
mod tests {
    use super::{prune_store, ProfileLogEntry, ProfileLogStore, PROFILE_LOG_RETENTION_MS};

    #[test]
    fn prune_store_removes_stale_entries() {
        let now = super::now_epoch_ms();
        let mut store = ProfileLogStore {
            entries: vec![
                ProfileLogEntry {
                    timestamp_epoch_ms: now.saturating_sub(PROFILE_LOG_RETENTION_MS + 1),
                    profile_id: "old".to_string(),
                    source: "launcher".to_string(),
                    message: "stale".to_string(),
                },
                ProfileLogEntry {
                    timestamp_epoch_ms: now,
                    profile_id: "new".to_string(),
                    source: "launcher".to_string(),
                    message: "fresh".to_string(),
                },
            ],
        };

        prune_store(&mut store);

        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].profile_id, "new");
    }
}
