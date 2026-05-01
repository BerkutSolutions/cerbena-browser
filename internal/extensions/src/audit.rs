use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionAuditEntry {
    pub profile_id: Uuid,
    pub extension_id: String,
    pub action: String,
    pub outcome: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ExtensionAuditLog {
    entries: Vec<ExtensionAuditEntry>,
}

impl ExtensionAuditLog {
    pub fn push(&mut self, entry: ExtensionAuditEntry) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[ExtensionAuditEntry] {
        &self.entries
    }
}
