use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheCleanupResult {
    pub removed_entries: usize,
    pub errors: usize,
}
