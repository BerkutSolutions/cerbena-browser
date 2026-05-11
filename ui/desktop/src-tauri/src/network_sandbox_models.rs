use super::*;

pub const MODE_AUTO: &str = "auto";
pub const MODE_ISOLATED: &str = "isolated";
pub const MODE_COMPAT_NATIVE: &str = "compatibility-native";
pub const MODE_CONTAINER: &str = "container";
pub const MODE_BLOCKED: &str = "blocked";

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxStore {
    #[serde(default)]
    pub global: NetworkSandboxGlobalSettings,
    #[serde(default)]
    pub profiles: BTreeMap<String, NetworkSandboxProfileSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxGlobalSettings {
    #[serde(default)]
    pub enabled: bool,
    pub default_mode: String,
    pub allow_native_compatibility_fallback: bool,
    pub target_runtime: String,
    pub max_active_sandboxes: u8,
}

impl Default for NetworkSandboxGlobalSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            default_mode: MODE_AUTO.to_string(),
            allow_native_compatibility_fallback: false,
            target_runtime: "launcher-managed".to_string(),
            max_active_sandboxes: 2,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxProfileSettings {
    pub preferred_mode: Option<String>,
    #[serde(default)]
    pub migrated_legacy_native: bool,
    pub last_resolved_mode: Option<String>,
    pub last_resolution_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxProfileView {
    pub effective_mode: String,
    pub preferred_mode: Option<String>,
    pub global_policy_enabled: bool,
    pub migrated_legacy_native: bool,
    pub last_resolved_mode: Option<String>,
    pub last_resolution_reason: Option<String>,
    pub resolution_available: bool,
    pub requires_native_backend: bool,
    pub requested_mode: String,
    pub target_runtime: String,
    pub adapter: NetworkSandboxAdapterPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedNetworkSandboxMode {
    IsolatedUserspace,
    CompatibilityNative,
    Container,
    Blocked,
}

impl ResolvedNetworkSandboxMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IsolatedUserspace => MODE_ISOLATED,
            Self::CompatibilityNative => MODE_COMPAT_NATIVE,
            Self::Container => MODE_CONTAINER,
            Self::Blocked => MODE_BLOCKED,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedNetworkSandboxStrategy {
    pub mode: ResolvedNetworkSandboxMode,
    pub requested_mode: String,
    pub requires_native_backend: bool,
    pub available: bool,
    pub reason: String,
}

impl ResolvedNetworkSandboxStrategy {
    pub fn effective_mode(&self) -> &'static str {
        self.mode.as_str()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNetworkSandboxProfileRequest {
    pub profile_id: String,
    pub preferred_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNetworkSandboxGlobalRequest {
    pub enabled: bool,
    pub default_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewNetworkSandboxRequest {
    pub profile_id: Option<String>,
    pub route_mode: Option<String>,
    pub template_id: Option<String>,
    pub preferred_mode: Option<String>,
    #[serde(default)]
    pub global_scope: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSandboxPreviewView {
    pub sandbox: NetworkSandboxProfileView,
    pub compatible_modes: Vec<String>,
    pub active_template_id: Option<String>,
    pub route_mode: String,
}
