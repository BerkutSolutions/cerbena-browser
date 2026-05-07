use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Wry};
use uuid::Uuid;

use crate::{process_tracking::is_process_running, shell_commands, state::app_local_data_root};

const HEARTBEAT_FILE_NAME: &str = "primary-instance-heartbeat.json";
const SINGLE_INSTANCE_LOCK_FILE_NAME: &str = "launcher-single-instance.lock";
const HANDOFF_QUEUE_DIR_NAME: &str = "incoming-events";
const LINK_EVENT_NAME: &str = "external-link-received";
const ACTIVATE_EVENT_NAME: &str = "launcher-activate-requested";
const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(1500);
const QUEUE_POLL_INTERVAL: Duration = Duration::from_millis(500);

static PRIMARY_HANDOFF_STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrimaryInstanceHeartbeat {
    pid: u32,
    updated_at_epoch_ms: u128,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExternalLinkHandoff {
    id: String,
    kind: String,
    url: String,
    created_at_epoch_ms: u128,
}

pub fn forward_link_to_primary_data_root(app_data_root: &Path, url: &str) -> Result<bool, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }
    let Some(heartbeat) = read_primary_heartbeat_path(app_data_root)? else {
        return Ok(false);
    };
    let current_pid = std::process::id();
    if heartbeat.pid == current_pid || !is_process_running(heartbeat.pid) {
        return Ok(false);
    }
    enqueue_handoff_event(app_data_root, "link", trimmed)?;
    Ok(true)
}

#[allow(dead_code)]
pub fn signal_primary_activation_data_root(app_data_root: &Path) -> Result<bool, String> {
    let Some(heartbeat) = read_primary_heartbeat_path(app_data_root)? else {
        return Ok(false);
    };
    let current_pid = std::process::id();
    if heartbeat.pid == current_pid || !is_process_running(heartbeat.pid) {
        return Ok(false);
    }
    enqueue_handoff_event(app_data_root, "activate", "")?;
    Ok(true)
}

#[allow(dead_code)]
pub fn acquire_single_instance_guard(app_data_root: &Path) -> Result<bool, String> {
    let path = single_instance_lock_path(app_data_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create single-instance dir {}: {e}", parent.display()))?;
    }
    let current_pid = std::process::id();
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(current_pid.to_string().as_bytes())
                .map_err(|e| format!("write single-instance lock {}: {e}", path.display()))?;
            Ok(true)
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing_pid = fs::read_to_string(&path)
                .ok()
                .and_then(|raw| raw.trim().parse::<u32>().ok());
            if let Some(pid) = existing_pid {
                if pid != current_pid && is_process_running(pid) {
                    return Ok(false);
                }
            }
            let _ = fs::remove_file(&path);
            acquire_single_instance_guard(app_data_root)
        }
        Err(err) => Err(format!(
            "open single-instance lock {}: {err}",
            path.display()
        )),
    }
}

fn enqueue_handoff_event(app_data_root: &Path, kind: &str, url: &str) -> Result<(), String> {
    let queue_dir = handoff_queue_dir_path(app_data_root);
    fs::create_dir_all(&queue_dir)
        .map_err(|e| format!("create launcher handoff dir {}: {e}", queue_dir.display()))?;
    let entry = ExternalLinkHandoff {
        id: Uuid::new_v4().to_string(),
        kind: kind.to_string(),
        url: url.to_string(),
        created_at_epoch_ms: now_epoch_ms(),
    };
    let entry_path = queue_dir.join(format!("{}-{}.json", entry.created_at_epoch_ms, entry.id));
    let bytes = serde_json::to_vec_pretty(&entry)
        .map_err(|e| format!("serialize launcher handoff entry: {e}"))?;
    fs::write(&entry_path, bytes)
        .map_err(|e| format!("write launcher handoff entry {}: {e}", entry_path.display()))
}

pub fn forward_link_to_primary(app: &AppHandle, url: &str) -> Result<bool, String> {
    forward_link_to_primary_data_root(&app_local_data_root(app)?, url)
}

pub fn setup_primary_instance_bridge(app: &tauri::App<Wry>) -> Result<(), String> {
    if PRIMARY_HANDOFF_STARTED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    write_primary_heartbeat(app.handle())?;

    let heartbeat_handle = app.handle().clone();
    std::thread::spawn(move || loop {
        let _ = write_primary_heartbeat(&heartbeat_handle);
        std::thread::sleep(HEARTBEAT_INTERVAL);
    });

    let queue_handle = app.handle().clone();
    std::thread::spawn(move || loop {
        let _ = emit_queued_links(&queue_handle);
        std::thread::sleep(QUEUE_POLL_INTERVAL);
    });

    Ok(())
}

pub fn cleanup_primary_instance(app: &AppHandle) {
    let path = match app_local_data_root(app).map(|root| heartbeat_path(&root)) {
        Ok(path) => path,
        Err(_) => return,
    };
    let current_pid = std::process::id();
    let remove = match fs::read(&path) {
        Ok(raw) => serde_json::from_slice::<PrimaryInstanceHeartbeat>(&raw)
            .map(|heartbeat| heartbeat.pid == current_pid)
            .unwrap_or(false),
        Err(_) => false,
    };
    if remove {
        let _ = fs::remove_file(path);
    }
    cleanup_single_instance_lock(app);
}

fn emit_queued_links(app: &AppHandle) -> Result<(), String> {
    let queue_dir = handoff_queue_dir_path(&app_local_data_root(app)?);
    if !queue_dir.exists() {
        return Ok(());
    }
    let mut entries = fs::read_dir(&queue_dir)
        .map_err(|e| format!("read launcher handoff dir {}: {e}", queue_dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let raw = match fs::read(&path) {
            Ok(raw) => raw,
            Err(_) => {
                let _ = fs::remove_file(&path);
                continue;
            }
        };
        let entry = match serde_json::from_slice::<ExternalLinkHandoff>(&raw) {
            Ok(entry) => entry,
            Err(_) => {
                let _ = fs::remove_file(&path);
                continue;
            }
        };
        let _ = fs::remove_file(&path);
        let _ = shell_commands::restore_main_window(app);
        if entry.kind == "activate" {
            let _ = app.emit(
                ACTIVATE_EVENT_NAME,
                serde_json::json!({
                    "id": entry.id,
                    "createdAtEpochMs": entry.created_at_epoch_ms,
                }),
            );
            continue;
        }
        let _ = app.emit(
            LINK_EVENT_NAME,
            serde_json::json!({
                "id": entry.id,
                "url": entry.url,
                "createdAtEpochMs": entry.created_at_epoch_ms,
            }),
        );
    }
    Ok(())
}

fn write_primary_heartbeat(app: &AppHandle) -> Result<(), String> {
    let path = heartbeat_path(&app_local_data_root(app)?);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create primary heartbeat dir {}: {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(&PrimaryInstanceHeartbeat {
        pid: std::process::id(),
        updated_at_epoch_ms: now_epoch_ms(),
    })
    .map_err(|e| format!("serialize primary heartbeat: {e}"))?;
    fs::write(&path, bytes).map_err(|e| format!("write primary heartbeat {}: {e}", path.display()))
}

fn read_primary_heartbeat_path(
    app_data_root: &Path,
) -> Result<Option<PrimaryInstanceHeartbeat>, String> {
    let path = heartbeat_path(app_data_root);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read(&path).map_err(|e| format!("read primary heartbeat {}: {e}", path.display()))?;
    let heartbeat = serde_json::from_slice::<PrimaryInstanceHeartbeat>(&raw)
        .map_err(|e| format!("parse primary heartbeat {}: {e}", path.display()))?;
    Ok(Some(heartbeat))
}

fn heartbeat_path(app_data_root: &Path) -> PathBuf {
    app_data_root.join(HEARTBEAT_FILE_NAME)
}

fn single_instance_lock_path(app_data_root: &Path) -> PathBuf {
    app_data_root.join(SINGLE_INSTANCE_LOCK_FILE_NAME)
}

fn handoff_queue_dir_path(app_data_root: &Path) -> PathBuf {
    app_data_root.join(HANDOFF_QUEUE_DIR_NAME)
}

fn cleanup_single_instance_lock(app: &AppHandle) {
    let path = match app_local_data_root(app).map(|root| single_instance_lock_path(&root)) {
        Ok(path) => path,
        Err(_) => return,
    };
    let current_pid = std::process::id();
    let remove = match fs::read_to_string(&path) {
        Ok(raw) => raw
            .trim()
            .parse::<u32>()
            .map(|pid| pid == current_pid)
            .unwrap_or(false),
        Err(_) => false,
    };
    if remove {
        let _ = fs::remove_file(path);
    }
}

fn now_epoch_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
