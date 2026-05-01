use browser_profile::{ProfileManager, SelectiveWipeRequest, WipeDataType};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PanicMode {
    Full,
    KeepPasswordsOnly,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanicWipeSummary {
    pub profile_id: Uuid,
    pub wiped_paths: usize,
    pub mode: PanicMode,
}

#[derive(Debug, Error)]
pub enum PanicError {
    #[error("safeguard rejected panic wipe")]
    SafeguardRejected,
    #[error("profile error: {0}")]
    Profile(String),
}

#[derive(Debug, Default, Clone)]
pub struct PanicWipeService;

impl PanicWipeService {
    pub fn execute(
        &self,
        manager: &ProfileManager,
        profile_id: Uuid,
        mode: PanicMode,
        site_scopes: Vec<String>,
        retain_paths: Vec<String>,
        confirm_phrase: &str,
        actor: &str,
    ) -> Result<PanicWipeSummary, PanicError> {
        if confirm_phrase != "ERASE_NOW" {
            return Err(PanicError::SafeguardRejected);
        }
        let data_types = match mode {
            PanicMode::Full => vec![
                WipeDataType::Cookies,
                WipeDataType::History,
                WipeDataType::Passwords,
                WipeDataType::Cache,
                WipeDataType::ExtensionsStorage,
            ],
            PanicMode::KeepPasswordsOnly => vec![
                WipeDataType::Cookies,
                WipeDataType::History,
                WipeDataType::Cache,
                WipeDataType::ExtensionsStorage,
            ],
            PanicMode::Custom => vec![
                WipeDataType::Cookies,
                WipeDataType::History,
                WipeDataType::Cache,
            ],
        };
        let wiped = manager
            .selective_wipe_profile_data(
                profile_id,
                &SelectiveWipeRequest {
                    data_types,
                    site_scopes,
                    retain_paths,
                },
                actor,
            )
            .map_err(|e| PanicError::Profile(e.to_string()))?;
        manager
            .close_profile(profile_id)
            .map_err(|e| PanicError::Profile(e.to_string()))?;
        Ok(PanicWipeSummary {
            profile_id,
            wiped_paths: wiped.len(),
            mode,
        })
    }
}
