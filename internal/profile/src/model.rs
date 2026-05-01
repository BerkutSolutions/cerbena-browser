use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::ProfileError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    Wayfern,
    Camoufox,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileState {
    Created,
    Ready,
    Running,
    Stopped,
    Locked,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub engine: Engine,
    pub state: ProfileState,
    pub default_start_page: Option<String>,
    pub default_search_provider: Option<String>,
    pub ephemeral_mode: bool,
    pub password_lock_enabled: bool,
    #[serde(default)]
    pub panic_frame_enabled: bool,
    #[serde(default)]
    pub panic_frame_color: Option<String>,
    #[serde(default)]
    pub panic_protected_sites: Vec<String>,
    pub crypto_version: u32,
    pub ephemeral_retain_paths: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateProfileInput {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub engine: Engine,
    pub default_start_page: Option<String>,
    pub default_search_provider: Option<String>,
    pub ephemeral_mode: bool,
    pub password_lock_enabled: bool,
    pub panic_frame_enabled: bool,
    pub panic_frame_color: Option<String>,
    pub panic_protected_sites: Vec<String>,
    pub ephemeral_retain_paths: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PatchProfileInput {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub state: Option<ProfileState>,
    pub default_start_page: Option<Option<String>>,
    pub default_search_provider: Option<Option<String>>,
    pub ephemeral_mode: Option<bool>,
    pub password_lock_enabled: Option<bool>,
    pub panic_frame_enabled: Option<bool>,
    pub panic_frame_color: Option<Option<String>>,
    pub panic_protected_sites: Option<Vec<String>>,
    pub ephemeral_retain_paths: Option<Vec<String>>,
}

pub fn validate_name(name: &str) -> Result<(), ProfileError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ProfileError::Validation("name cannot be empty".to_string()));
    }
    if trimmed.len() > 128 {
        return Err(ProfileError::Validation(
            "name exceeds 128 characters".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_tags(tags: &[String]) -> Result<(), ProfileError> {
    if tags.len() > 32 {
        return Err(ProfileError::Validation(
            "maximum 32 tags per profile".to_string(),
        ));
    }
    for tag in tags {
        if tag.trim().is_empty() {
            return Err(ProfileError::Validation("tag cannot be empty".to_string()));
        }
        // Internal system tags can exceed the user-facing limit.
        if (tag.starts_with("policy:")
            || tag.starts_with("dns-template:")
            || tag.starts_with("cert-id:")
            || tag.starts_with("ext:")
            || tag.starts_with("ext-disabled:"))
            && tag.len() > 48
        {
            continue;
        }
        if tag.len() > 48 {
            return Err(ProfileError::Validation(
                "tag exceeds 48 characters".to_string(),
            ));
        }
    }
    Ok(())
}
