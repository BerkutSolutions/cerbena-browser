use super::*;

pub(crate) fn validate_record(
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

pub(crate) fn touch_session(state: &AppState, record: &LaunchSessionRecord) -> Result<(), String> {
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

pub(crate) fn persist_store_locked(state: &AppState, store: &LaunchSessionStore) -> Result<(), String> {
    let path = state.launch_session_store_path(&state.app_handle)?;
    persist_launch_session_store(&path, &state.sensitive_store_secret, store)
}

pub(crate) fn write_workspace_marker(profile_root: &Path, record: &LaunchSessionRecord) -> Result<(), String> {
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

pub(super) fn read_workspace_marker(profile_root: &Path) -> Result<Option<WorkspaceSessionMarker>, String> {
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

pub(crate) fn remove_workspace_marker(profile_root: &Path) {
    let marker_path = workspace_marker_path(profile_root);
    let _ = fs::remove_file(marker_path);
}

pub(crate) fn workspace_marker_path(profile_root: &Path) -> PathBuf {
    profile_root.join("tmp").join("cerbena-launch-session.json")
}

pub(crate) fn workspace_fingerprint(
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

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    sha256_hex(&bytes)
}

pub(crate) fn now_epoch_ms() -> u128 {
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

    pub(crate) fn temp_profile_root() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("cerbena-launch-session-{unique}"))
    }

    #[test]
    pub(crate) fn workspace_fingerprint_changes_when_workspace_changes() {
        let profile_id = Uuid::new_v4();
        let a = workspace_fingerprint(
            profile_id,
            "chromium",
            PathBuf::from("C:/profiles/a").as_path(),
            PathBuf::from("C:/profiles/a/engine-profile").as_path(),
        );
        let b = workspace_fingerprint(
            profile_id,
            "chromium",
            PathBuf::from("C:/profiles/a").as_path(),
            PathBuf::from("C:/profiles/b/engine-profile").as_path(),
        );
        assert_ne!(a, b);
    }

    #[test]
    pub(crate) fn workspace_marker_roundtrip_shape_is_stable() {
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
    pub(crate) fn validate_record_rejects_tampered_workspace_marker() {
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
            engine: "chromium".to_string(),
            profile_root: root.to_string_lossy().to_string(),
            workspace_dir: workspace.to_string_lossy().to_string(),
            workspace_fingerprint: workspace_fingerprint(profile_id, "chromium", &root, &workspace),
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
            validate_record(&record, record.pid, "chromium", &root, &workspace).expect("validate");
        assert!(!trusted);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    pub(crate) fn validate_record_rejects_engine_mismatch() {
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
            engine: "chromium".to_string(),
            profile_root: root.to_string_lossy().to_string(),
            workspace_dir: workspace.to_string_lossy().to_string(),
            workspace_fingerprint: workspace_fingerprint(profile_id, "chromium", &root, &workspace),
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
            validate_record(&record, record.pid, "librewolf", &root, &workspace).expect("validate");
        assert!(!trusted);
        let _ = fs::remove_dir_all(root);
    }
}

