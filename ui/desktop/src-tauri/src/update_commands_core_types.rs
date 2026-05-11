use super::*;

pub(crate) const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const REPOSITORY_URL: &str = "https://github.com/BerkutSolutions/cerbena-browser";
pub(crate) const GITHUB_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/BerkutSolutions/cerbena-browser/releases/latest";
pub(crate) const RELEASE_LATEST_API_URL_ENV: &str = "CERBENA_RELEASE_LATEST_API_URL";
pub(crate) const RELEASE_CHECKSUMS_ASSET: &str = "checksums.txt";
pub(crate) const RELEASE_CHECKSUMS_SIGNATURE_ASSET: &str = "checksums.sig";
pub(crate) const RELEASE_CHECKSUMS_B64_ENV: &str = "CERBENA_RELEASE_CHECKSUMS_B64";
pub(crate) const RELEASE_CHECKSUMS_SIGNATURE_B64_ENV: &str = "CERBENA_RELEASE_CHECKSUMS_SIGNATURE_B64";
pub(crate) const UPDATE_CHECK_INTERVAL_MS: u128 = 6 * 60 * 60 * 1000;
pub(crate) const SCHEDULER_TICK: Duration = Duration::from_secs(15 * 60);
pub(crate) const USER_AGENT: &str = concat!("Cerbena-Updater/", env!("CARGO_PKG_VERSION"));
pub(crate) const UPDATER_EVENT_NAME: &str = "updater-progress";
pub const UPDATER_RELAUNCH_AUTO_EXIT_ENV: &str = "CERBENA_UPDATER_AUTO_EXIT_AFTER_SECONDS";
pub const UPDATER_MSI_INSTALL_DIR_ENV: &str = "CERBENA_UPDATER_MSI_INSTALL_DIR";
pub const UPDATER_MSI_TIMEOUT_MS_ENV: &str = "CERBENA_UPDATER_MSI_TIMEOUT_MS";
pub(crate) const RELEASE_SIGNING_PUBLIC_KEY_XML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../config/release/release-signing-public-key.xml"
));
pub(crate) const RELEASE_SIGNING_LEGACY_PUBLIC_KEYS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../config/release/release-signing-legacy-public-keys.json"
));

pub(crate) const UPDATER_STEP_DISCOVER: &str = "discover";
pub(crate) const UPDATER_STEP_COMPARE: &str = "compare";
pub(crate) const UPDATER_STEP_SECURITY: &str = "security";
pub(crate) const UPDATER_STEP_DOWNLOAD: &str = "download";
pub(crate) const UPDATER_STEP_CHECKSUM: &str = "checksum";
pub(crate) const UPDATER_STEP_INSTALL: &str = "install";
pub(crate) const UPDATER_STEP_RELAUNCH: &str = "relaunch";
pub(crate) const UPDATER_HELPER_LOG_ENV: &str = "CERBENA_UPDATER_RUNTIME_LOG";
pub(crate) const UPDATER_AUTO_CLOSE_AFTER_READY_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateStore {
    #[serde(default = "default_auto_update_enabled")]
    pub auto_update_enabled: bool,
    #[serde(default)]
    pub last_checked_at: Option<String>,
    #[serde(default)]
    pub last_checked_epoch_ms: Option<u128>,
    #[serde(default)]
    pub latest_version: Option<String>,
    #[serde(default)]
    pub release_url: Option<String>,
    #[serde(default)]
    pub has_update: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub staged_version: Option<String>,
    #[serde(default)]
    pub staged_asset_name: Option<String>,
    #[serde(default)]
    pub staged_asset_path: Option<String>,
    #[serde(default)]
    pub selected_asset_type: Option<String>,
    #[serde(default)]
    pub selected_asset_reason: Option<String>,
    #[serde(default)]
    pub install_handoff_mode: Option<String>,
    #[serde(default)]
    pub pending_apply_on_exit: bool,
    #[serde(default)]
    pub updater_handoff_version: Option<String>,
}

pub(crate) fn default_auto_update_enabled() -> bool {
    true
}

impl Default for AppUpdateStore {
    fn default() -> Self {
        Self {
            auto_update_enabled: default_auto_update_enabled(),
            last_checked_at: None,
            last_checked_epoch_ms: None,
            latest_version: None,
            release_url: None,
            has_update: false,
            status: String::new(),
            last_error: None,
            staged_version: None,
            staged_asset_name: None,
            staged_asset_path: None,
            selected_asset_type: None,
            selected_asset_reason: None,
            install_handoff_mode: None,
            pending_apply_on_exit: false,
            updater_handoff_version: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdaterLaunchMode {
    Disabled,
    Preview,
    Auto,
}

impl UpdaterLaunchMode {
    pub fn from_args<I>(args: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut preview = false;
        let mut updater = false;
        for value in args {
            match value.as_ref().trim() {
                "--updater" => updater = true,
                "--updater-preview" => {
                    updater = true;
                    preview = true;
                }
                _ => {}
            }
        }
        let launched_from_updater_binary = std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_name()
                    .and_then(|value| value.to_str().map(|value| value.to_ascii_lowercase()))
            })
            .map(|value| value.contains("updater"))
            .unwrap_or(false);
        if !updater && !launched_from_updater_binary {
            return Self::Disabled;
        }
        if preview {
            return Self::Preview;
        }
        Self::Auto
    }

    pub fn is_active(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    pub fn is_preview(self) -> bool {
        matches!(self, Self::Preview)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterStepView {
    pub id: String,
    pub title_key: String,
    pub detail: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterOverview {
    pub mode: String,
    pub dry_run: bool,
    pub status: String,
    pub current_version: String,
    pub target_version: Option<String>,
    pub release_url: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub summary_key: String,
    pub summary_detail: String,
    pub can_close: bool,
    pub close_label_key: String,
    pub steps: Vec<UpdaterStepView>,
}

#[derive(Debug)]
pub struct UpdaterRuntimeState {
    pub launch_mode: UpdaterLaunchMode,
    pub flow_started: bool,
    pub running: bool,
    pub overview: UpdaterOverview,
}

impl UpdaterRuntimeState {
    pub fn new(launch_mode: UpdaterLaunchMode) -> Self {
        Self {
            launch_mode,
            flow_started: false,
            running: false,
            overview: updater_overview_template(launch_mode),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateView {
    pub current_version: String,
    pub repository_url: String,
    pub auto_update_enabled: bool,
    pub last_checked_at: Option<String>,
    pub latest_version: Option<String>,
    pub release_url: Option<String>,
    pub has_update: bool,
    pub status: String,
    pub last_error: Option<String>,
    pub staged_version: Option<String>,
    pub staged_asset_name: Option<String>,
    pub selected_asset_type: Option<String>,
    pub selected_asset_reason: Option<String>,
    pub install_handoff_mode: Option<String>,
    pub can_auto_apply: bool,
}

#[tauri::command]
pub fn get_updater_overview(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<UpdaterOverview>, String> {
    updater_commands::get_updater_overview_impl(state, correlation_id)
}

#[tauri::command]
pub fn start_updater_flow(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<UpdaterOverview>, String> {
    updater_commands::start_updater_flow_impl(state, correlation_id)
}

#[tauri::command]
pub fn launch_updater_preview(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    updater_commands::launch_updater_preview_impl(state, correlation_id)
}

#[derive(Debug, Deserialize)]
pub(crate) struct GithubRelease {
    pub(crate) tag_name: String,
    pub(crate) html_url: String,
    pub(crate) assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GithubReleaseAsset {
    pub(crate) name: String,
    pub(crate) browser_download_url: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ReleaseCandidate {
    pub(crate) version: String,
    pub(crate) release_url: String,
    pub(crate) asset_name: Option<String>,
    pub(crate) asset_url: Option<String>,
    pub(crate) asset_type: Option<String>,
    pub(crate) asset_selection_reason: Option<String>,
    pub(crate) install_handoff_mode: Option<String>,
    pub(crate) checksums_url: Option<String>,
    pub(crate) checksums_signature_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectedAssetKind {
    WindowsMsi,
    WindowsZip,
    WindowsExe,
    LinuxTarGz,
    LinuxZip,
    MacZip,
}

impl SelectedAssetKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::WindowsMsi => "msi",
            Self::WindowsZip => "portable_zip",
            Self::WindowsExe => "manual_installer",
            Self::LinuxTarGz => "linux_tar_gz",
            Self::LinuxZip => "linux_zip",
            Self::MacZip => "mac_zip",
        }
    }

    pub(crate) fn handoff_mode(self) -> &'static str {
        match self {
            Self::WindowsMsi => "direct_msi",
            Self::WindowsZip => "portable_zip",
            Self::WindowsExe => "manual_installer",
            Self::LinuxTarGz => "manual_installer",
            Self::LinuxZip => "manual_installer",
            Self::MacZip => "manual_installer",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectedReleaseAsset<'a> {
    pub(crate) asset: &'a GithubReleaseAsset,
    pub(crate) kind: SelectedAssetKind,
    pub(crate) reason: &'static str,
}

pub(crate) fn updater_overview_template(mode: UpdaterLaunchMode) -> UpdaterOverview {
    let dry_run = mode.is_preview();
    UpdaterOverview {
        mode: if dry_run {
            "preview".to_string()
        } else {
            "auto".to_string()
        },
        dry_run,
        status: "idle".to_string(),
        current_version: CURRENT_VERSION.to_string(),
        target_version: None,
        release_url: None,
        started_at: None,
        finished_at: None,
        summary_key: if dry_run {
            "updater.summary.preview_ready".to_string()
        } else {
            "updater.summary.ready".to_string()
        },
        summary_detail: String::new(),
        can_close: false,
        close_label_key: "updater.running".to_string(),
        steps: vec![
            updater_step(UPDATER_STEP_DISCOVER, "updater.steps.discover", "idle"),
            updater_step(UPDATER_STEP_COMPARE, "updater.steps.compare", "idle"),
            updater_step(UPDATER_STEP_SECURITY, "updater.steps.security", "idle"),
            updater_step(UPDATER_STEP_DOWNLOAD, "updater.steps.download", "idle"),
            updater_step(UPDATER_STEP_CHECKSUM, "updater.steps.checksum", "idle"),
            updater_step(UPDATER_STEP_INSTALL, "updater.steps.install", "idle"),
            updater_step(UPDATER_STEP_RELAUNCH, "updater.steps.relaunch", "idle"),
        ],
    }
}

pub(crate) fn updater_step(id: &str, title_key: &str, status: &str) -> UpdaterStepView {
    UpdaterStepView {
        id: id.to_string(),
        title_key: title_key.to_string(),
        detail: String::new(),
        status: status.to_string(),
    }
}

