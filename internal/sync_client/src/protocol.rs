use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl Default for SyncProtocolVersion {
    fn default() -> Self {
        Self { major: 1, minor: 0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMutation {
    pub object_key: String,
    pub revision: u64,
    pub payload_b64: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MergePolicy {
    LastWriteWins,
    RejectOnConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflictResolution {
    pub policy: MergePolicy,
    pub max_retry: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPayload {
    pub protocol: SyncProtocolVersion,
    pub profile_id: Uuid,
    pub mutations: Vec<SyncMutation>,
    pub resolution: SyncConflictResolution,
    pub sequence: u64,
}
