use serde::{Deserialize, Serialize};

fn default_check_default_browser_on_startup() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellPreferenceStore {
    #[serde(default = "default_check_default_browser_on_startup")]
    pub check_default_browser_on_startup: bool,
    #[serde(default)]
    pub default_browser_prompt_decided: bool,
    #[serde(default)]
    pub minimize_to_tray_enabled: bool,
    #[serde(default)]
    pub close_to_tray_prompt_declined: bool,
    #[serde(default)]
    pub launch_on_system_startup: bool,
    #[serde(default)]
    pub startup_profile_id: Option<String>,
}

impl Default for ShellPreferenceStore {
    fn default() -> Self {
        Self {
            check_default_browser_on_startup: true,
            default_browser_prompt_decided: false,
            minimize_to_tray_enabled: false,
            close_to_tray_prompt_declined: false,
            launch_on_system_startup: false,
            startup_profile_id: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellPreferenceUpdateRequest {
    pub check_default_browser_on_startup: Option<bool>,
    pub default_browser_prompt_decided: Option<bool>,
    pub minimize_to_tray_enabled: Option<bool>,
    pub close_to_tray_prompt_declined: Option<bool>,
    pub launch_on_system_startup: Option<bool>,
    pub startup_profile_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUrlRequest {
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellPreferencesState {
    pub check_default_browser_on_startup: bool,
    pub default_browser_prompt_decided: bool,
    pub minimize_to_tray_enabled: bool,
    pub close_to_tray_prompt_declined: bool,
    pub launch_on_system_startup: bool,
    pub startup_profile_id: Option<String>,
    pub launched_from_system_startup: bool,
    pub is_default_browser: bool,
    pub should_prompt_default_browser_preference: bool,
    pub should_prompt_default_link_profile: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseRequestAction {
    AllowExit,
    HideToTray,
    PromptToEnableTray,
}

