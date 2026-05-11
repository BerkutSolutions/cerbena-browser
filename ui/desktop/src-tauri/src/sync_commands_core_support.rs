use super::{
    persist_sync_store_with_secret, AppState, Duration, SyncControlsModel, SyncServerConfig,
    SyncStatusLevel, SyncStatusView, SystemTime, TcpStream, ToSocketAddrs, UNIX_EPOCH,
};

pub(super) fn resolve_ping_profile_key(
    store: &crate::state::SyncStore,
    requested_profile_id: Option<String>,
) -> Result<String, String> {
    if let Some(profile_id) = requested_profile_id {
        let trimmed = profile_id.trim();
        if trimmed.is_empty() {
            return Err("profile id is required".to_string());
        }
        return Ok(trimmed.to_string());
    }

    if let Some((profile_id, _)) = store
        .controls
        .iter()
        .find(|(_, model)| model.server.sync_enabled)
    {
        return Ok(profile_id.clone());
    }
    Err("no enabled sync profile found".to_string())
}

pub(super) fn parse_server_endpoint(server_url: &str) -> Result<(String, u16), String> {
    let parsed = reqwest::Url::parse(server_url.trim())
        .map_err(|e| format!("sync server url is invalid: {e}"))?;
    let host = parsed
        .host_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "sync server host is required".to_string())?
        .to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| "sync server port is missing".to_string())?;
    Ok((host, port))
}

pub(super) fn endpoint_reachable(host: &str, port: u16, timeout_ms: u64) -> bool {
    let mut addrs = match (host, port).to_socket_addrs() {
        Ok(value) => value,
        Err(_) => return false,
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms.max(1))).is_ok()
}

pub(super) fn persist_store(state: &AppState, store: &crate::state::SyncStore) -> Result<(), String> {
    let path = state.sync_store_path(&state.app_handle)?;
    persist_sync_store_with_secret(&path, &state.sensitive_store_secret, store)
}

pub(super) fn default_sync_controls() -> SyncControlsModel {
    SyncControlsModel {
        server: SyncServerConfig {
            server_url: String::new(),
            key_id: String::new(),
            sync_enabled: false,
        },
        status: SyncStatusView {
            level: SyncStatusLevel::Warning,
            message_key: "sync.disabled".to_string(),
            last_sync_unix_ms: None,
        },
        conflicts: Vec::new(),
        can_backup: true,
        can_restore: true,
    }
}

pub(super) fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
