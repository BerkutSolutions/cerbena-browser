use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    process_tracking::is_process_running,
    sensitive_store::{load_sensitive_json_or_default, persist_sensitive_json},
    state::AppState,
};

const LAUNCH_SESSION_SCOPE: &str = "launch-session-store";

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct LaunchSessionStore {
    pub sessions: BTreeMap<String, LaunchSessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSessionRecord {
    pub profile_id: String,
    pub session_id: String,
    pub session_token: String,
    pub session_token_hash: String,
    pub pid: u32,
    pub engine: String,
    pub profile_root: String,
    pub workspace_dir: String,
    pub workspace_fingerprint: String,
    pub started_at_epoch_ms: u128,
    pub last_verified_at_epoch_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSessionMarker {
    session_id: String,
    session_token_hash: String,
    workspace_fingerprint: String,
    pid: u32,
}

pub fn load_launch_session_store(
    path: &Path,
    secret_material: &str,
) -> Result<LaunchSessionStore, String> {
    load_sensitive_json_or_default(path, LAUNCH_SESSION_SCOPE, secret_material)
}

pub fn persist_launch_session_store(
    path: &Path,
    secret_material: &str,
    store: &LaunchSessionStore,
) -> Result<(), String> {
    persist_sensitive_json(path, LAUNCH_SESSION_SCOPE, secret_material, store)
}

pub fn trusted_session_for_profile(
    state: &AppState,
    profile_id: Uuid,
    expected_pid: u32,
    engine: &str,
    profile_root: &Path,
    workspace_dir: &Path,
) -> Result<Option<LaunchSessionRecord>, String> {
    let store = state
        .launch_session_store
        .lock()
        .map_err(|_| "launch session store lock poisoned".to_string())?;
    let record = store.sessions.get(&profile_id.to_string()).cloned();
    drop(store);
    let Some(record) = record else {
        return Ok(None);
    };
    if validate_record(&record, expected_pid, engine, profile_root, workspace_dir)? {
        touch_session(state, &record)?;
        return Ok(Some(record));
    }
    revoke_launch_session(state, profile_id, Some(expected_pid))?;
    Ok(None)
}

pub fn issue_launch_session(
    state: &AppState,
    profile_id: Uuid,
    pid: u32,
    engine: &str,
    profile_root: &Path,
    workspace_dir: &Path,
) -> Result<LaunchSessionRecord, String> {
    let session_token = random_token();
    let record = LaunchSessionRecord {
        profile_id: profile_id.to_string(),
        session_id: Uuid::new_v4().to_string(),
        session_token_hash: sha256_hex(session_token.as_bytes()),
        session_token,
        pid,
        engine: engine.to_string(),
        profile_root: profile_root.to_string_lossy().to_string(),
        workspace_dir: workspace_dir.to_string_lossy().to_string(),
        workspace_fingerprint: workspace_fingerprint(
            profile_id,
            engine,
            profile_root,
            workspace_dir,
        ),
        started_at_epoch_ms: now_epoch_ms(),
        last_verified_at_epoch_ms: now_epoch_ms(),
    };
    write_workspace_marker(profile_root, &record)?;
    let mut store = state
        .launch_session_store
        .lock()
        .map_err(|_| "launch session store lock poisoned".to_string())?;
    store
        .sessions
        .insert(profile_id.to_string(), record.clone());
    persist_store_locked(state, &store)?;
    Ok(record)
}

pub fn revoke_launch_session(
    state: &AppState,
    profile_id: Uuid,
    expected_pid: Option<u32>,
) -> Result<(), String> {
    let mut store = state
        .launch_session_store
        .lock()
        .map_err(|_| "launch session store lock poisoned".to_string())?;
    let removed = match store.sessions.get(&profile_id.to_string()) {
        Some(record) if expected_pid.map(|pid| pid == record.pid).unwrap_or(true) => {
            store.sessions.remove(&profile_id.to_string())
        }
        _ => None,
    };
    persist_store_locked(state, &store)?;
    drop(store);
    if let Some(record) = removed {
        remove_workspace_marker(Path::new(&record.profile_root));
    }
    Ok(())
}

pub fn trusted_session_pid(state: &AppState, profile_id: Uuid) -> Result<Option<u32>, String> {
    let store = state
        .launch_session_store
        .lock()
        .map_err(|_| "launch session store lock poisoned".to_string())?;
    Ok(store
        .sessions
        .get(&profile_id.to_string())
        .map(|record| record.pid))
}

pub fn prune_inactive_sessions(state: &AppState) -> Result<(), String> {
    let stale = {
        let store = state
            .launch_session_store
            .lock()
            .map_err(|_| "launch session store lock poisoned".to_string())?;
        store
            .sessions
            .values()
            .filter(|record| !is_process_running(record.pid))
            .map(|record| {
                (
                    Uuid::parse_str(&record.profile_id).unwrap_or(Uuid::nil()),
                    record.profile_root.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    for (profile_id, profile_root) in stale {
        if profile_id != Uuid::nil() {
            revoke_launch_session(state, profile_id, None)?;
        } else {
            remove_workspace_marker(Path::new(&profile_root));
        }
    }
    Ok(())
}


#[path = "launch_sessions_core_support.rs"]
mod support;
pub(crate) use support::*;


