use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipMode {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipSetting {
    pub mode: PipMode,
    pub prefer_always_on_top: bool,
    pub fallback_message_key: String,
}

#[derive(Debug, Default, Clone)]
pub struct PipPolicyService;

impl PipPolicyService {
    pub fn resolve(&self, requested: PipMode, platform_supported: bool) -> PipSetting {
        if platform_supported {
            PipSetting {
                mode: requested,
                prefer_always_on_top: matches!(requested, PipMode::Enabled),
                fallback_message_key: "pip.supported".to_string(),
            }
        } else {
            PipSetting {
                mode: PipMode::Disabled,
                prefer_always_on_top: false,
                fallback_message_key: "pip.unsupported_platform".to_string(),
            }
        }
    }
}
