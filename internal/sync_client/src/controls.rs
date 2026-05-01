use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncServerConfig {
    pub server_url: String,
    pub key_id: String,
    pub sync_enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatusLevel {
    Healthy,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatusView {
    pub level: SyncStatusLevel,
    pub message_key: String,
    pub last_sync_unix_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictViewItem {
    pub object_key: String,
    pub local_revision: u64,
    pub remote_revision: u64,
    pub action_hint_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncControlsModel {
    pub server: SyncServerConfig,
    pub status: SyncStatusView,
    pub conflicts: Vec<ConflictViewItem>,
    pub can_backup: bool,
    pub can_restore: bool,
}

impl SyncControlsModel {
    pub fn validate(&self) -> Result<(), String> {
        if self.server.sync_enabled && self.server.server_url.trim().is_empty() {
            return Err("sync.controls.server_url.required".to_string());
        }
        if self.server.sync_enabled && self.server.key_id.trim().is_empty() {
            return Err("sync.controls.key_id.required".to_string());
        }
        Ok(())
    }
}
