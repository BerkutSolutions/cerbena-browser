use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionStatus {
    Imported,
    PendingFirstLaunchInstall,
    Installed,
    Enabled,
    Disabled,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionUpdatePolicy {
    ManualOnly,
    FollowSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionImportState {
    Pending,
    Installed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRecord {
    pub profile_id: Uuid,
    pub extension_id: String,
    pub display_name: String,
    pub version: String,
    pub source: String,
    pub package_path: String,
    pub status: ExtensionStatus,
    pub import_state: ExtensionImportState,
    pub update_policy: ExtensionUpdatePolicy,
    pub first_launch_attempts: u8,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileExtensionState {
    pub profile_id: Uuid,
    pub extensions: Vec<ExtensionRecord>,
}
