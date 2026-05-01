use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkDispatchResult {
    pub target_profile_id: Uuid,
    pub url: String,
    pub fallback_used: bool,
}

#[derive(Debug, Default, Clone)]
pub struct DefaultBrowserHandler {
    default_profile: Option<Uuid>,
    profile_start_pages: BTreeMap<Uuid, String>,
}

impl DefaultBrowserHandler {
    pub fn set_default_profile(&mut self, profile_id: Uuid) {
        self.default_profile = Some(profile_id);
    }

    pub fn set_profile_start_page(&mut self, profile_id: Uuid, url: &str) {
        self.profile_start_pages.insert(profile_id, url.to_string());
    }

    pub fn dispatch_external_link(&self, url: &str) -> Option<LinkDispatchResult> {
        let profile_id = self.default_profile?;
        let fallback = self
            .profile_start_pages
            .get(&profile_id)
            .cloned()
            .unwrap_or_else(|| "https://duckduckgo.com".to_string());
        let normalized = if url.trim().is_empty() {
            fallback.clone()
        } else {
            url.to_string()
        };
        Some(LinkDispatchResult {
            target_profile_id: profile_id,
            url: normalized,
            fallback_used: url.trim().is_empty(),
        })
    }
}
