use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::protocol::{MergePolicy, SyncPayload};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRecord {
    pub profile_id: Uuid,
    pub object_key: String,
    pub revision: u64,
    pub payload_b64: String,
    pub idempotency_key: String,
}

#[derive(Debug, Error)]
pub enum SyncServerError {
    #[error("conflict on object: {0}")]
    Conflict(String),
}

#[derive(Debug, Default)]
pub struct InMemorySyncServer {
    records: BTreeMap<(Uuid, String), SyncRecord>,
    seen_idempotency: BTreeSet<String>,
    audit: Vec<String>,
}

impl InMemorySyncServer {
    pub fn apply_payload(
        &mut self,
        payload: &SyncPayload,
    ) -> Result<Vec<SyncRecord>, SyncServerError> {
        let mut applied = Vec::new();
        for m in &payload.mutations {
            if self.seen_idempotency.contains(&m.idempotency_key) {
                continue;
            }
            let key = (payload.profile_id, m.object_key.clone());
            if let Some(current) = self.records.get(&key) {
                let conflict = m.revision <= current.revision;
                if conflict && matches!(payload.resolution.policy, MergePolicy::RejectOnConflict) {
                    return Err(SyncServerError::Conflict(m.object_key.clone()));
                }
            }
            let record = SyncRecord {
                profile_id: payload.profile_id,
                object_key: m.object_key.clone(),
                revision: m.revision,
                payload_b64: m.payload_b64.clone(),
                idempotency_key: m.idempotency_key.clone(),
            };
            self.records.insert(key, record.clone());
            self.seen_idempotency.insert(m.idempotency_key.clone());
            self.audit.push(format!(
                "sync.apply profile={} key={} rev={}",
                payload.profile_id, m.object_key, m.revision
            ));
            applied.push(record);
        }
        Ok(applied)
    }

    pub fn records_for_profile(&self, profile_id: Uuid) -> Vec<SyncRecord> {
        self.records
            .values()
            .filter(|v| v.profile_id == profile_id)
            .cloned()
            .collect()
    }

    pub fn audit_entries(&self) -> &[String] {
        &self.audit
    }
}
