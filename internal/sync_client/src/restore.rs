use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::snapshots::BackupSnapshot;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RestoreScope {
    Full,
    Selective,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreRequest {
    pub profile_id: Uuid,
    pub snapshot_id: String,
    pub scope: RestoreScope,
    pub include_prefixes: Vec<String>,
    pub expected_schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub restored_snapshot_id: String,
    pub restored_profile_id: Uuid,
    pub restored_items: usize,
    pub skipped_items: usize,
}

#[derive(Debug, Error)]
pub enum RestoreError {
    #[error("snapshot not found")]
    SnapshotNotFound,
    #[error("profile mismatch")]
    ProfileMismatch,
    #[error("schema mismatch")]
    SchemaMismatch,
    #[error("integrity verification failed")]
    IntegrityFailed,
}

#[derive(Debug, Default, Clone)]
pub struct RestorePlanner;

impl RestorePlanner {
    pub fn restore(
        &self,
        request: &RestoreRequest,
        snapshot: &BackupSnapshot,
        is_integrity_ok: bool,
        payload_paths: &[String],
    ) -> Result<RestoreResult, RestoreError> {
        if snapshot.snapshot_id != request.snapshot_id {
            return Err(RestoreError::SnapshotNotFound);
        }
        if snapshot.profile_id != request.profile_id {
            return Err(RestoreError::ProfileMismatch);
        }
        if request.expected_schema_version != 1 {
            return Err(RestoreError::SchemaMismatch);
        }
        if !is_integrity_ok {
            return Err(RestoreError::IntegrityFailed);
        }

        let (restored_items, skipped_items) = match request.scope {
            RestoreScope::Full => (payload_paths.len(), 0),
            RestoreScope::Selective => {
                let restored = payload_paths
                    .iter()
                    .filter(|p| {
                        request
                            .include_prefixes
                            .iter()
                            .any(|pref| p.starts_with(pref))
                    })
                    .count();
                (restored, payload_paths.len().saturating_sub(restored))
            }
        };

        Ok(RestoreResult {
            restored_snapshot_id: snapshot.snapshot_id.clone(),
            restored_profile_id: snapshot.profile_id,
            restored_items,
            skipped_items,
        })
    }
}
