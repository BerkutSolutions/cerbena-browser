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
struct WorkspaceSessionMarker {
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

fn validate_record(
    record: &LaunchSessionRecord,
    expected_pid: u32,
    engine: &str,
    profile_root: &Path,
    workspace_dir: &Path,
) -> Result<bool, String> {
    if record.pid != expected_pid || record.engine != engine {
        return Ok(false);
    }
    let record_profile_id =
        Uuid::parse_str(&record.profile_id).map_err(|e| format!("session profile id: {e}"))?;
    let expected_fingerprint =
        workspace_fingerprint(record_profile_id, engine, profile_root, workspace_dir);
    if record.workspace_fingerprint != expected_fingerprint {
        return Ok(false);
    }
    let marker = read_workspace_marker(profile_root)?;
    Ok(marker
        .map(|value| {
            value.session_id == record.session_id
                && value.session_token_hash == record.session_token_hash
                && value.workspace_fingerprint == record.workspace_fingerprint
                && value.pid == record.pid
        })
        .unwrap_or(false))
}

fn touch_session(state: &AppState, record: &LaunchSessionRecord) -> Result<(), String> {
    let mut store = state
        .launch_session_store
        .lock()
        .map_err(|_| "launch session store lock poisoned".to_string())?;
    if let Some(current) = store.sessions.get_mut(&record.profile_id) {
        current.last_verified_at_epoch_ms = now_epoch_ms();
        persist_store_locked(state, &store)?;
    }
    Ok(())
}

fn persist_store_locked(state: &AppState, store: &LaunchSessionStore) -> Result<(), String> {
    let path = state.launch_session_store_path(&state.app_handle)?;
    persist_launch_session_store(&path, &state.sensitive_store_secret, store)
}

fn write_workspace_marker(profile_root: &Path, record: &LaunchSessionRecord) -> Result<(), String> {
    let marker_path = workspace_marker_path(profile_root);
    if let Some(parent) = marker_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create session marker dir {}: {e}", parent.display()))?;
    }
    let marker = WorkspaceSessionMarker {
        session_id: record.session_id.clone(),
        session_token_hash: record.session_token_hash.clone(),
        workspace_fingerprint: record.workspace_fingerprint.clone(),
        pid: record.pid,
    };
    let bytes = serde_json::to_vec_pretty(&marker)
        .map_err(|e| format!("serialize workspace session marker: {e}"))?;
    fs::write(&marker_path, bytes).map_err(|e| {
        format!(
            "write workspace session marker {}: {e}",
            marker_path.display()
        )
    })
}

fn read_workspace_marker(profile_root: &Path) -> Result<Option<WorkspaceSessionMarker>, String> {
    let marker_path = workspace_marker_path(profile_root);
    if !marker_path.exists() {
        return Ok(None);
    }
    let raw = fs::read(&marker_path).map_err(|e| {
        format!(
            "read workspace session marker {}: {e}",
            marker_path.display()
        )
    })?;
    let marker = serde_json::from_slice::<WorkspaceSessionMarker>(&raw).map_err(|e| {
        format!(
            "parse workspace session marker {}: {e}",
            marker_path.display()
        )
    })?;
    Ok(Some(marker))
}

fn remove_workspace_marker(profile_root: &Path) {
    let marker_path = workspace_marker_path(profile_root);
    let _ = fs::remove_file(marker_path);
}

fn workspace_marker_path(profile_root: &Path) -> PathBuf {
    profile_root.join("tmp").join("cerbena-launch-session.json")
}

fn workspace_fingerprint(
    profile_id: Uuid,
    engine: &str,
    profile_root: &Path,
    workspace_dir: &Path,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(profile_id.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(engine.trim().as_bytes());
    hasher.update(b"|");
    hasher.update(profile_root.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(workspace_dir.to_string_lossy().as_bytes());
    sha256_hex(&hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    sha256_hex(&bytes)
}

fn now_epoch_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        sha256_hex, validate_record, workspace_fingerprint, LaunchSessionRecord,
        WorkspaceSessionMarker,
    };
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    use uuid::Uuid;

    fn temp_profile_root() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("cerbena-launch-session-{unique}"))
    }

    #[test]
    fn workspace_fingerprint_changes_when_workspace_changes() {
        let profile_id = Uuid::new_v4();
        let a = workspace_fingerprint(
            profile_id,
            "wayfern",
            PathBuf::from("C:/profiles/a").as_path(),
            PathBuf::from("C:/profiles/a/engine-profile").as_path(),
        );
        let b = workspace_fingerprint(
            profile_id,
            "wayfern",
            PathBuf::from("C:/profiles/a").as_path(),
            PathBuf::from("C:/profiles/b/engine-profile").as_path(),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn workspace_marker_roundtrip_shape_is_stable() {
        let root = temp_profile_root();
        let marker_path = root.join("tmp").join("cerbena-launch-session.json");
        fs::create_dir_all(marker_path.parent().expect("parent")).expect("mkdir");
        let marker = WorkspaceSessionMarker {
            session_id: Uuid::new_v4().to_string(),
            session_token_hash: sha256_hex(b"token"),
            workspace_fingerprint: sha256_hex(b"workspace"),
            pid: 4242,
        };
        let encoded = serde_json::to_vec_pretty(&marker).expect("encode");
        fs::write(&marker_path, encoded).expect("write");
        let decoded: WorkspaceSessionMarker =
            serde_json::from_slice(&fs::read(&marker_path).expect("read")).expect("decode");
        assert_eq!(decoded.pid, 4242);
        assert_eq!(decoded.session_token_hash, marker.session_token_hash);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_record_rejects_tampered_workspace_marker() {
        let root = temp_profile_root();
        let workspace = root.join("engine-profile");
        let marker_path = root.join("tmp").join("cerbena-launch-session.json");
        fs::create_dir_all(marker_path.parent().expect("parent")).expect("mkdir");
        fs::create_dir_all(&workspace).expect("workspace");
        let profile_id = Uuid::new_v4();
        let record = LaunchSessionRecord {
            profile_id: profile_id.to_string(),
            session_id: Uuid::new_v4().to_string(),
            session_token: "token".to_string(),
            session_token_hash: sha256_hex(b"token"),
            pid: 4242,
            engine: "wayfern".to_string(),
            profile_root: root.to_string_lossy().to_string(),
            workspace_dir: workspace.to_string_lossy().to_string(),
            workspace_fingerprint: workspace_fingerprint(profile_id, "wayfern", &root, &workspace),
            started_at_epoch_ms: 1,
            last_verified_at_epoch_ms: 1,
        };
        let marker = WorkspaceSessionMarker {
            session_id: record.session_id.clone(),
            session_token_hash: sha256_hex(b"tampered"),
            workspace_fingerprint: record.workspace_fingerprint.clone(),
            pid: record.pid,
        };
        fs::write(
            &marker_path,
            serde_json::to_vec_pretty(&marker).expect("encode marker"),
        )
        .expect("write marker");

        let trusted =
            validate_record(&record, record.pid, "wayfern", &root, &workspace).expect("validate");
        assert!(!trusted);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_record_rejects_engine_mismatch() {
        let root = temp_profile_root();
        let workspace = root.join("engine-profile");
        let marker_path = root.join("tmp").join("cerbena-launch-session.json");
        fs::create_dir_all(marker_path.parent().expect("parent")).expect("mkdir");
        fs::create_dir_all(&workspace).expect("workspace");
        let profile_id = Uuid::new_v4();
        let record = LaunchSessionRecord {
            profile_id: profile_id.to_string(),
            session_id: Uuid::new_v4().to_string(),
            session_token: "token".to_string(),
            session_token_hash: sha256_hex(b"token"),
            pid: 4242,
            engine: "wayfern".to_string(),
            profile_root: root.to_string_lossy().to_string(),
            workspace_dir: workspace.to_string_lossy().to_string(),
            workspace_fingerprint: workspace_fingerprint(profile_id, "wayfern", &root, &workspace),
            started_at_epoch_ms: 1,
            last_verified_at_epoch_ms: 1,
        };
        let marker = WorkspaceSessionMarker {
            session_id: record.session_id.clone(),
            session_token_hash: record.session_token_hash.clone(),
            workspace_fingerprint: record.workspace_fingerprint.clone(),
            pid: record.pid,
        };
        fs::write(
            &marker_path,
            serde_json::to_vec_pretty(&marker).expect("encode marker"),
        )
        .expect("write marker");

        let trusted =
            validate_record(&record, record.pid, "camoufox", &root, &workspace).expect("validate");
        assert!(!trusted);
        let _ = fs::remove_dir_all(root);
    }
}
