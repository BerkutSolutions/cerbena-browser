use std::{
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use browser_sync_client::{
    BackupSnapshot, ConflictViewItem, RestorePlanner, RestoreRequest, RestoreResult,
    SyncControlsModel, SyncServerConfig, SyncStatusLevel, SyncStatusView,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::{
    envelope::{ok, UiEnvelope},
    state::{persist_sync_store_with_secret, AppState},
    sync_snapshots::{
        create_snapshot_for_profile, restore_snapshot_for_profile, verify_snapshot_integrity,
    },
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSyncControlsRequest {
    pub profile_id: String,
    pub model: SyncControlsModel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSyncConflictRequest {
    pub profile_id: String,
    pub item: ConflictViewItem,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBackupSnapshotRequest {
    pub profile_id: String,
    #[serde(default)]
    pub encrypted_blob_b64: Option<String>,
    #[serde(default)]
    pub sha256_hex: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSnapshotCommandRequest {
    pub request: RestoreRequest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOverviewView {
    pub profile_id: String,
    pub controls: SyncControlsModel,
    pub conflicts: Vec<ConflictViewItem>,
    pub snapshots: Vec<BackupSnapshot>,
}

#[tauri::command]
pub fn save_sync_controls(
    state: State<AppState>,
    request: SaveSyncControlsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    request.model.validate()?;
    let mut store = state
        .sync_store
        .lock()
        .map_err(|_| "sync store lock poisoned".to_string())?;

    let mut controls = request.model.clone();
    let existing_conflicts = store
        .conflicts
        .get(&request.profile_id)
        .cloned()
        .unwrap_or_default();
    if request.model.conflicts.is_empty() {
        controls.conflicts = existing_conflicts;
    } else {
        controls.conflicts = request.model.conflicts.clone();
        store
            .conflicts
            .insert(request.profile_id.clone(), request.model.conflicts.clone());
    }
    store.controls.insert(request.profile_id, controls);
    persist_store(&state, &store)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn get_sync_overview(
    state: State<AppState>,
    profile_id: String,
    correlation_id: String,
) -> Result<UiEnvelope<SyncOverviewView>, String> {
    let store = state
        .sync_store
        .lock()
        .map_err(|_| "sync store lock poisoned".to_string())?;
    let conflicts = store
        .conflicts
        .get(&profile_id)
        .cloned()
        .unwrap_or_default();
    let mut controls = store
        .controls
        .get(&profile_id)
        .cloned()
        .unwrap_or_else(default_sync_controls);
    controls.conflicts = conflicts.clone();
    let snapshots = store
        .snapshots
        .get(&profile_id)
        .cloned()
        .unwrap_or_default();

    Ok(ok(
        correlation_id,
        SyncOverviewView {
            profile_id,
            controls,
            conflicts,
            snapshots,
        },
    ))
}

#[tauri::command]
pub fn add_sync_conflict(
    state: State<AppState>,
    request: AddSyncConflictRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut store = state
        .sync_store
        .lock()
        .map_err(|_| "sync store lock poisoned".to_string())?;
    store
        .conflicts
        .entry(request.profile_id.clone())
        .or_default()
        .push(request.item);
    if let Some(model) = store.controls.get_mut(&request.profile_id) {
        model.status = SyncStatusView {
            level: SyncStatusLevel::Warning,
            message_key: "sync.conflicts.present".to_string(),
            last_sync_unix_ms: model.status.last_sync_unix_ms,
        };
    }
    persist_store(&state, &store)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn clear_sync_conflicts(
    state: State<AppState>,
    profile_id: String,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut store = state
        .sync_store
        .lock()
        .map_err(|_| "sync store lock poisoned".to_string())?;
    store.conflicts.remove(&profile_id);
    if let Some(model) = store.controls.get_mut(&profile_id) {
        model.conflicts.clear();
        if model.server.sync_enabled {
            model.status = SyncStatusView {
                level: SyncStatusLevel::Healthy,
                message_key: "sync.healthy".to_string(),
                last_sync_unix_ms: model.status.last_sync_unix_ms,
            };
        } else {
            model.status = SyncStatusView {
                level: SyncStatusLevel::Warning,
                message_key: "sync.disabled".to_string(),
                last_sync_unix_ms: model.status.last_sync_unix_ms,
            };
        }
    }
    persist_store(&state, &store)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn create_backup_snapshot(
    state: State<AppState>,
    request: CreateBackupSnapshotRequest,
    correlation_id: String,
) -> Result<UiEnvelope<BackupSnapshot>, String> {
    let profile_uuid = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    let (encrypted_blob_b64, sha256_hex, _) = if let (Some(blob), Some(hash)) = (
        request
            .encrypted_blob_b64
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        request
            .sha256_hex
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        (blob.to_string(), hash.to_string(), Vec::new())
    } else {
        create_snapshot_for_profile(&state, profile_uuid)?
    };

    let snapshot = {
        let mut manager = state
            .snapshot_manager
            .lock()
            .map_err(|_| "snapshot manager lock poisoned".to_string())?;
        manager.create_snapshot(profile_uuid, encrypted_blob_b64, sha256_hex.to_lowercase())
    };

    let mut store = state
        .sync_store
        .lock()
        .map_err(|_| "sync store lock poisoned".to_string())?;
    let entries = store
        .snapshots
        .entry(request.profile_id.clone())
        .or_default();
    entries.push(snapshot.clone());
    if entries.len() > 20 {
        let overflow = entries.len() - 20;
        entries.drain(0..overflow);
    }
    if let Some(model) = store.controls.get_mut(&request.profile_id) {
        model.status.last_sync_unix_ms = Some(now_epoch_ms());
    }
    persist_store(&state, &store)?;
    Ok(ok(correlation_id, snapshot))
}

#[tauri::command]
pub fn restore_snapshot(
    state: State<AppState>,
    request: RestoreSnapshotCommandRequest,
    correlation_id: String,
) -> Result<UiEnvelope<RestoreResult>, String> {
    let profile_key = request.request.profile_id.to_string();
    let mut store = state
        .sync_store
        .lock()
        .map_err(|_| "sync store lock poisoned".to_string())?;
    let snapshots = store
        .snapshots
        .get_mut(&profile_key)
        .ok_or_else(|| "snapshot not found".to_string())?;
    let index = snapshots
        .iter()
        .position(|value| value.snapshot_id == request.request.snapshot_id)
        .ok_or_else(|| "snapshot not found".to_string())?;
    let snapshot = snapshots[index].clone();
    let (integrity_ok, payload_paths) = verify_snapshot_integrity(&state, &snapshot)?;
    if !integrity_ok {
        snapshots.remove(index);
        persist_store(&state, &store)?;
        return Err("restore snapshot integrity verification failed".to_string());
    }

    let planner = RestorePlanner;
    let (result, actual_payload_paths) =
        restore_snapshot_for_profile(&state, &request.request, &snapshot)?;
    let _ = planner
        .restore(&request.request, &snapshot, true, &payload_paths)
        .map_err(|e| e.to_string())?;
    if let Some(model) = store.controls.get_mut(&profile_key) {
        model.status = SyncStatusView {
            level: SyncStatusLevel::Healthy,
            message_key: "sync.restored".to_string(),
            last_sync_unix_ms: Some(now_epoch_ms()),
        };
    }
    persist_store(&state, &store)?;
    let _ = actual_payload_paths;
    Ok(ok(correlation_id, result))
}

#[tauri::command]
pub fn sync_health_ping(
    state: State<AppState>,
    profile_id: Option<String>,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut store = state
        .sync_store
        .lock()
        .map_err(|_| "sync store lock poisoned".to_string())?;
    let profile_key = resolve_ping_profile_key(&store, profile_id)?;
    let controls = store
        .controls
        .get_mut(&profile_key)
        .ok_or_else(|| "sync controls are not configured for this profile".to_string())?;
    if !controls.server.sync_enabled {
        controls.status = SyncStatusView {
            level: SyncStatusLevel::Warning,
            message_key: "sync.disabled".to_string(),
            last_sync_unix_ms: controls.status.last_sync_unix_ms,
        };
        persist_store(&state, &store)?;
        return Err("sync is disabled for this profile".to_string());
    }
    let (host, port) = parse_server_endpoint(&controls.server.server_url)?;
    let reachable = endpoint_reachable(&host, port, 1_500);
    controls.status = if reachable {
        SyncStatusView {
            level: SyncStatusLevel::Healthy,
            message_key: "sync.healthy".to_string(),
            last_sync_unix_ms: Some(now_epoch_ms()),
        }
    } else {
        SyncStatusView {
            level: SyncStatusLevel::Error,
            message_key: "sync.unreachable".to_string(),
            last_sync_unix_ms: controls.status.last_sync_unix_ms,
        }
    };
    persist_store(&state, &store)?;
    if !reachable {
        return Err(format!("sync endpoint {host}:{port} is unreachable"));
    }
    Ok(ok(correlation_id, true))
}

#[path = "sync_commands_core_support.rs"]
mod sync_commands_core_support;

use sync_commands_core_support::{
    default_sync_controls,
    endpoint_reachable,
    now_epoch_ms,
    parse_server_endpoint,
    persist_store,
    resolve_ping_profile_key,
};

