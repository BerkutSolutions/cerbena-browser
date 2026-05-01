use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::SyncRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSnapshot {
    pub snapshot_id: String,
    pub profile_id: Uuid,
    pub created_at_unix_ms: u128,
    pub encrypted_blob_b64: String,
    pub integrity_sha256_hex: String,
}

#[derive(Debug, Clone)]
pub struct SnapshotManager {
    pub retention_limit: usize,
    snapshots: Vec<BackupSnapshot>,
    quarantined: Vec<BackupSnapshot>,
}

impl Default for SnapshotManager {
    fn default() -> Self {
        Self {
            retention_limit: 10,
            snapshots: Vec::new(),
            quarantined: Vec::new(),
        }
    }
}

impl SnapshotManager {
    pub fn with_retention_limit(retention_limit: usize) -> Self {
        Self {
            retention_limit,
            ..Self::default()
        }
    }

    pub fn create_snapshot(
        &mut self,
        profile_id: Uuid,
        encrypted_blob_b64: String,
        sha256_hex: String,
    ) -> BackupSnapshot {
        let snapshot = BackupSnapshot {
            snapshot_id: format!("snap-{}-{}", profile_id, now_unix_ms()),
            profile_id,
            created_at_unix_ms: now_unix_ms(),
            encrypted_blob_b64,
            integrity_sha256_hex: sha256_hex,
        };
        self.snapshots.push(snapshot.clone());
        self.prune();
        snapshot
    }

    pub fn verify_or_quarantine(&mut self, snapshot_id: &str, computed_sha256_hex: &str) -> bool {
        let Some(index) = self
            .snapshots
            .iter()
            .position(|v| v.snapshot_id == snapshot_id)
        else {
            return false;
        };
        if self.snapshots[index].integrity_sha256_hex == computed_sha256_hex {
            return true;
        }
        let broken = self.snapshots.remove(index);
        self.quarantined.push(broken);
        false
    }

    pub fn snapshots_for_profile(&self, profile_id: Uuid) -> Vec<BackupSnapshot> {
        self.snapshots
            .iter()
            .filter(|v| v.profile_id == profile_id)
            .cloned()
            .collect()
    }

    pub fn quarantined(&self) -> &[BackupSnapshot] {
        &self.quarantined
    }

    pub fn from_records_payload(records: &[SyncRecord]) -> String {
        serde_json::to_string(records).unwrap_or_else(|_| "[]".to_string())
    }

    fn prune(&mut self) {
        if self.snapshots.len() <= self.retention_limit {
            return;
        }
        self.snapshots
            .sort_by(|a, b| a.created_at_unix_ms.cmp(&b.created_at_unix_ms));
        while self.snapshots.len() > self.retention_limit {
            self.snapshots.remove(0);
        }
    }
}

fn now_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
