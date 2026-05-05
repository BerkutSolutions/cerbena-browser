use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State};

use crate::{
    envelope::{ok, UiEnvelope},
    launcher_commands::push_runtime_log,
    state::{app_local_data_root, persist_app_update_store, AppState},
};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPOSITORY_URL: &str = "https://github.com/BerkutSolutions/cerbena-browser";
const GITHUB_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/BerkutSolutions/cerbena-browser/releases/latest";
const RELEASE_LATEST_API_URL_ENV: &str = "CERBENA_RELEASE_LATEST_API_URL";
const RELEASE_CHECKSUMS_ASSET: &str = "checksums.txt";
const RELEASE_CHECKSUMS_SIGNATURE_ASSET: &str = "checksums.sig";
const RELEASE_CHECKSUMS_B64_ENV: &str = "CERBENA_RELEASE_CHECKSUMS_B64";
const RELEASE_CHECKSUMS_SIGNATURE_B64_ENV: &str = "CERBENA_RELEASE_CHECKSUMS_SIGNATURE_B64";
const UPDATE_CHECK_INTERVAL_MS: u128 = 6 * 60 * 60 * 1000;
const SCHEDULER_TICK: Duration = Duration::from_secs(15 * 60);
const USER_AGENT: &str = concat!("Cerbena-Updater/", env!("CARGO_PKG_VERSION"));
const UPDATER_EVENT_NAME: &str = "updater-progress";
pub const UPDATER_RELAUNCH_AUTO_EXIT_ENV: &str = "CERBENA_UPDATER_AUTO_EXIT_AFTER_SECONDS";
pub const UPDATER_MSI_INSTALL_DIR_ENV: &str = "CERBENA_UPDATER_MSI_INSTALL_DIR";
pub const UPDATER_MSI_TIMEOUT_MS_ENV: &str = "CERBENA_UPDATER_MSI_TIMEOUT_MS";
const RELEASE_SIGNING_PUBLIC_KEY_XML: &str =
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../config/release/release-signing-public-key.xml"
    ));
const RELEASE_SIGNING_LEGACY_PUBLIC_KEYS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../config/release/release-signing-legacy-public-keys.json"
));

const UPDATER_STEP_DISCOVER: &str = "discover";
const UPDATER_STEP_COMPARE: &str = "compare";
const UPDATER_STEP_SECURITY: &str = "security";
const UPDATER_STEP_DOWNLOAD: &str = "download";
const UPDATER_STEP_CHECKSUM: &str = "checksum";
const UPDATER_STEP_INSTALL: &str = "install";
const UPDATER_STEP_RELAUNCH: &str = "relaunch";
const UPDATER_HELPER_LOG_ENV: &str = "CERBENA_UPDATER_RUNTIME_LOG";

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

fn default_auto_update_enabled() -> bool {
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
    let overview = state
        .updater_runtime
        .lock()
        .map_err(|e| format!("lock updater runtime: {e}"))?
        .overview
        .clone();
    Ok(ok(correlation_id, overview))
}

#[tauri::command]
pub fn start_updater_flow(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<UpdaterOverview>, String> {
    ensure_updater_flow_started(&state)?;
    let overview = state
        .updater_runtime
        .lock()
        .map_err(|e| format!("lock updater runtime: {e}"))?
        .overview
        .clone();
    Ok(ok(correlation_id, overview))
}

#[tauri::command]
pub fn launch_updater_preview(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    spawn_updater_process(&state.app_handle, UpdaterLaunchMode::Preview)?;
    Ok(ok(correlation_id, true))
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone)]
struct ReleaseCandidate {
    version: String,
    release_url: String,
    asset_name: Option<String>,
    asset_url: Option<String>,
    asset_type: Option<String>,
    asset_selection_reason: Option<String>,
    install_handoff_mode: Option<String>,
    checksums_url: Option<String>,
    checksums_signature_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedAssetKind {
    WindowsMsi,
    WindowsZip,
    WindowsExe,
    LinuxTarGz,
    LinuxZip,
    MacZip,
}

impl SelectedAssetKind {
    fn label(self) -> &'static str {
        match self {
            Self::WindowsMsi => "msi",
            Self::WindowsZip => "portable_zip",
            Self::WindowsExe => "manual_installer",
            Self::LinuxTarGz => "linux_tar_gz",
            Self::LinuxZip => "linux_zip",
            Self::MacZip => "mac_zip",
        }
    }

    fn handoff_mode(self) -> &'static str {
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
struct SelectedReleaseAsset<'a> {
    asset: &'a GithubReleaseAsset,
    kind: SelectedAssetKind,
    reason: &'static str,
}

fn updater_overview_template(mode: UpdaterLaunchMode) -> UpdaterOverview {
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

fn updater_step(id: &str, title_key: &str, status: &str) -> UpdaterStepView {
    UpdaterStepView {
        id: id.to_string(),
        title_key: title_key.to_string(),
        detail: String::new(),
        status: status.to_string(),
    }
}

#[tauri::command]
pub fn get_launcher_update_state(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    let store = refresh_update_store_snapshot(&state)?;
    Ok(ok(correlation_id, to_view(&store)))
}

#[tauri::command]
pub fn set_launcher_auto_update(
    state: State<AppState>,
    enabled: bool,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    let mut store = refresh_update_store_snapshot(&state)?;
    store.auto_update_enabled = enabled;
    if store.status.trim().is_empty() {
        store.status = "idle".to_string();
    }
    write_update_store_snapshot(&state, &store)?;
    Ok(ok(correlation_id, to_view(&store)))
}

#[tauri::command]
pub fn check_launcher_updates(
    state: State<AppState>,
    manual: bool,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    let view = run_update_cycle(&state, manual)?;
    Ok(ok(correlation_id, view))
}

pub fn start_update_scheduler(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(SCHEDULER_TICK);
        let state = app.state::<AppState>();
        let updater_active = state
            .updater_runtime
            .lock()
            .map(|runtime| runtime.launch_mode.is_active())
            .unwrap_or(false);
        if updater_active {
            continue;
        }
        let should_run = match state.app_update_store.lock() {
            Ok(store) => should_run_auto_update_check(&store),
            Err(_) => false,
        };
        if should_run {
            let _ = run_update_cycle(&state, false);
        }
    });
}

pub fn active_updater_launch_mode() -> UpdaterLaunchMode {
    UpdaterLaunchMode::from_args(std::env::args().skip(1))
}

pub fn configure_window_for_launch_mode(
    window: &tauri::WebviewWindow,
    mode: UpdaterLaunchMode,
) -> Result<(), String> {
    if !mode.is_active() {
        return Ok(());
    }
    window
        .set_title(if mode.is_preview() {
            "Cerbena Updater Preview"
        } else {
            "Cerbena Updater"
        })
        .map_err(|e| format!("set updater window title: {e}"))?;
    window
        .set_size(LogicalSize::new(760.0, 720.0))
        .map_err(|e| format!("set updater window size: {e}"))?;
    window
        .set_min_size(Some(LogicalSize::new(680.0, 640.0)))
        .map_err(|e| format!("set updater window min size: {e}"))?;
    let _ = window.center();
    window
        .eval(&format!(
            "window.location.replace('./updater.html?mode={}');",
            if mode.is_preview() { "preview" } else { "auto" }
        ))
        .map_err(|e| format!("redirect updater window: {e}"))?;
    Ok(())
}

pub fn ensure_updater_flow_started(state: &AppState) -> Result<(), String> {
    let runtime = state.updater_runtime.clone();
    let launch_mode = {
        let mut guard = runtime
            .lock()
            .map_err(|e| format!("lock updater runtime: {e}"))?;
        if guard.flow_started {
            return Ok(());
        }
        guard.flow_started = true;
        guard.running = true;
        guard.overview.status = "running".to_string();
        guard.overview.started_at = Some(now_iso());
        guard.overview.finished_at = None;
        guard.overview.can_close = false;
        guard.overview.close_label_key = "updater.running".to_string();
        guard.launch_mode
    };

    let app_handle = state.app_handle.clone();
    thread::spawn(move || {
        let app_state = app_handle.state::<AppState>();
        let result = run_updater_flow(&app_state, launch_mode);
        if let Err(error) = result {
            let _ = finalize_updater_failure(&app_state, &error);
        }
    });
    Ok(())
}

fn updater_launch_mode_from_state(state: &AppState) -> Result<UpdaterLaunchMode, String> {
    state
        .updater_runtime
        .lock()
        .map(|runtime| runtime.launch_mode)
        .map_err(|e| format!("lock updater runtime: {e}"))
}

fn schedule_updater_window_close_for_apply(state: &AppState) {
    push_runtime_log(
        state,
        "[updater] scheduling updater window close to trigger pending apply",
    );
    let app_handle = state.app_handle.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(750));
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.close();
        }
    });
}

fn should_auto_close_updater_after_ready_to_restart(launch_mode: UpdaterLaunchMode) -> bool {
    matches!(launch_mode, UpdaterLaunchMode::Auto)
}

fn run_updater_flow(state: &AppState, launch_mode: UpdaterLaunchMode) -> Result<(), String> {
    if launch_mode.is_preview() {
        return run_preview_updater_flow(state);
    }
    run_live_updater_flow(state)
}

fn should_launch_external_updater(store: &AppUpdateStore, candidate: &ReleaseCandidate) -> bool {
    store.updater_handoff_version.as_deref() != Some(candidate.version.as_str())
}

fn should_run_auto_update_check(store: &AppUpdateStore) -> bool {
    store.auto_update_enabled
        && store
            .last_checked_epoch_ms
            .map(|value| now_epoch_ms().saturating_sub(value) >= UPDATE_CHECK_INTERVAL_MS)
            .unwrap_or(true)
}

fn spawn_updater_process(app: &AppHandle, mode: UpdaterLaunchMode) -> Result<(), String> {
    let state = app.state::<AppState>();
    let exe = resolve_updater_executable_path(app)?;
    let mut command = Command::new(exe);
    if mode.is_preview() {
        command.arg("--updater-preview");
    } else {
        command.arg("--updater");
    }
    if let Some(dir) = app_local_data_root(app)
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
    {
        command.current_dir(dir);
    }
    if let Ok(log_path) = state.runtime_log_path(app) {
        command.env(UPDATER_HELPER_LOG_ENV, log_path);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x00000010);
    }
    push_runtime_log(
        &state,
        format!(
            "[updater] spawning standalone updater mode={} exe={}",
            if mode.is_preview() { "preview" } else { "auto" },
            command.get_program().to_string_lossy()
        ),
    );
    command
        .spawn()
        .map_err(|e| format!("spawn standalone updater: {e}"))?;
    Ok(())
}

fn resolve_updater_executable_path(app: &AppHandle) -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("resolve current exe: {e}"))?;
    let adjacent = current_exe
        .parent()
        .map(|parent| parent.join("cerbena-updater.exe"))
        .ok_or_else(|| "resolve current exe parent for updater".to_string())?;
    if adjacent.is_file() {
        return Ok(adjacent);
    }
    let _ = app;
    Ok(current_exe)
}

fn current_windows_install_mode() -> String {
    if !cfg!(target_os = "windows") {
        return "non_windows".to_string();
    }
    let Ok(current_exe) = std::env::current_exe() else {
        return "portable_zip".to_string();
    };
    let Some(install_root) = current_exe.parent() else {
        return "portable_zip".to_string();
    };
    let marker = install_root.join("cerbena-install-mode.txt");
    if let Ok(value) = fs::read_to_string(marker) {
        let normalized = value.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            return normalized;
        }
    }
    "portable_zip".to_string()
}

pub fn launch_pending_update_on_exit(app: &AppHandle) {
    let state = app.state::<AppState>();

    let snapshot = match state.app_update_store.lock() {
        Ok(store) => store.clone(),
        Err(error) => {
            push_runtime_log(
                &state,
                format!("[updater] pending apply skipped: failed to lock store: {error}"),
            );
            return;
        }
    };

    if !snapshot.pending_apply_on_exit {
        push_runtime_log(
            &state,
            "[updater] pending apply skipped: pending_apply_on_exit=false",
        );
        return;
    }

    let Some(path) = snapshot.staged_asset_path.as_ref() else {
        push_runtime_log(
            &state,
            "[updater] pending apply skipped: staged_asset_path missing",
        );
        return;
    };
    let asset_path = PathBuf::from(path);
    if !asset_path.is_file() {
        push_runtime_log(
            &state,
            format!(
                "[updater] pending apply skipped: staged asset missing path={}",
                asset_path.display()
            ),
        );
        return;
    }

    let extension = asset_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let current_pid = std::process::id();
    let current_exe = match std::env::current_exe() {
        Ok(value) => value,
        Err(error) => {
            push_runtime_log(
                &state,
                format!("[updater] pending apply skipped: resolve current exe failed: {error}"),
            );
            return;
        }
    };
    let install_root = match current_exe.parent() {
        Some(value) => value.to_path_buf(),
        None => {
            push_runtime_log(&state, "[updater] pending apply skipped: install root missing");
            return;
        }
    };
    let relaunch_executable = resolve_relaunch_executable_path(&install_root);
    let msi_target_install_root = app_local_data_root(&state.app_handle).ok();
    let runtime_log_path = state
        .runtime_log_path(&state.app_handle)
        .ok()
        .map(|value| value.to_string_lossy().to_string());
    push_runtime_log(
        &state,
        format!(
            "[updater] pending apply start asset={} extension={} install_root={} msi_target_root={} relaunch_exe={} target_version={}",
            asset_path.display(),
            extension,
            install_root.display(),
            msi_target_install_root
                .as_deref()
                .map(|value| value.display().to_string())
                .unwrap_or_else(|| "missing".to_string()),
            relaunch_executable
                .as_deref()
                .map(|value| value.display().to_string())
                .unwrap_or_else(|| "missing".to_string()),
            snapshot
                .staged_version
                .clone()
                .unwrap_or_else(|| "unknown".to_string())
        ),
    );

    let launched = match extension.as_str() {
        "zip" => launch_zip_apply_helper(
            current_pid,
            &asset_path,
            &install_root,
            relaunch_executable.as_deref(),
            runtime_log_path.as_deref(),
        )
        .is_ok(),
        "msi" => {
            let store_path = state
                .app_update_store_path(&state.app_handle)
                .ok()
                .map(|value| value.to_string_lossy().to_string());
            launch_msi_apply_helper(
                current_pid,
                &asset_path,
                msi_target_install_root.as_deref(),
                store_path.as_deref(),
                snapshot.staged_version.as_deref(),
                runtime_log_path.as_deref(),
            )
            .is_ok()
        }
        _ => false,
    };

    if !launched {
        push_runtime_log(
            &state,
            format!("[updater] pending apply failed: helper launch failed for {extension}"),
        );
        return;
    }

    if let Ok(mut store) = state.app_update_store.lock() {
        store.pending_apply_on_exit = false;
        store.status = "applying".to_string();
        let _ = persist_update_store_from_state(&state, &store);
        push_runtime_log(
            &state,
            format!(
                "[updater] pending apply helper launched asset={} status=applying",
                asset_path.display()
            ),
        );
    };
}

fn run_preview_updater_flow(state: &AppState) -> Result<(), String> {
    update_updater_overview(state, |overview| {
        overview.target_version = Some(format!("{CURRENT_VERSION}-preview"));
        overview.release_url = Some(REPOSITORY_URL.to_string());
        overview.summary_key = "updater.summary.preview_running".to_string();
        overview.summary_detail = "i18n:updater.detail.preview_running".to_string();
    })?;
    progress_updater_step(
        state,
        UPDATER_STEP_DISCOVER,
        "running",
        "i18n:updater.detail.preview_discover_running",
    )?;
    thread::sleep(Duration::from_millis(260));
    progress_updater_step(
        state,
        UPDATER_STEP_DISCOVER,
        "done",
        "i18n:updater.detail.preview_discover_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_COMPARE,
        "running",
        "i18n:updater.detail.preview_compare_running",
    )?;
    thread::sleep(Duration::from_millis(260));
    progress_updater_step(
        state,
        UPDATER_STEP_COMPARE,
        "done",
        "i18n:updater.detail.preview_compare_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_SECURITY,
        "running",
        "i18n:updater.detail.preview_security_running",
    )?;
    thread::sleep(Duration::from_millis(260));
    progress_updater_step(
        state,
        UPDATER_STEP_SECURITY,
        "done",
        "i18n:updater.detail.preview_security_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_DOWNLOAD,
        "running",
        "i18n:updater.detail.preview_download_running",
    )?;
    thread::sleep(Duration::from_millis(260));
    progress_updater_step(
        state,
        UPDATER_STEP_DOWNLOAD,
        "done",
        "i18n:updater.detail.preview_download_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_CHECKSUM,
        "running",
        "i18n:updater.detail.preview_checksum_running",
    )?;
    thread::sleep(Duration::from_millis(260));
    progress_updater_step(
        state,
        UPDATER_STEP_CHECKSUM,
        "done",
        "i18n:updater.detail.preview_checksum_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_INSTALL,
        "done",
        "i18n:updater.detail.preview_install_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_RELAUNCH,
        "done",
        "i18n:updater.detail.preview_relaunch_done",
    )?;
    finalize_updater_success(
        state,
        "completed",
        "updater.summary.preview_complete",
        "i18n:updater.detail.preview_complete",
        "action.close",
    )
}

fn run_live_updater_flow(state: &AppState) -> Result<(), String> {
    let launch_mode = updater_launch_mode_from_state(state)?;
    push_runtime_log(
        state,
        format!(
            "[updater] live flow start current_version={} launch_mode={:?}",
            CURRENT_VERSION, launch_mode
        ),
    );
    progress_updater_step(
        state,
        UPDATER_STEP_DISCOVER,
        "running",
        "i18n:updater.detail.live_discover_running",
    )?;
    let client = build_release_http_client(Duration::from_secs(30), false)
        .map_err(|e| format!("build updater client: {e}"))?;
    let candidate = fetch_latest_release(&client)?;
    push_runtime_log(
        state,
        format!(
            "[updater] live release discovered version={} asset={} asset_type={} handoff={} reason={} release_url={}",
            candidate.version,
            candidate.asset_name.as_deref().unwrap_or("missing"),
            candidate.asset_type.as_deref().unwrap_or("unknown"),
            candidate.install_handoff_mode.as_deref().unwrap_or("unknown"),
            candidate
                .asset_selection_reason
                .as_deref()
                .unwrap_or("unspecified"),
            candidate.release_url
        ),
    );
    update_updater_overview(state, |overview| {
        overview.target_version = Some(candidate.version.clone());
        overview.release_url = Some(candidate.release_url.clone());
        overview.summary_key = "updater.summary.running".to_string();
        overview.summary_detail = "i18n:updater.detail.live_running".to_string();
    })?;
    progress_updater_step(
        state,
        UPDATER_STEP_DISCOVER,
        "done",
        "i18n:updater.detail.live_discover_done",
    )?;

    progress_updater_step(
        state,
        UPDATER_STEP_COMPARE,
        "running",
        "i18n:updater.detail.live_compare_running",
    )?;
    if !is_version_newer(&candidate.version, CURRENT_VERSION) {
        push_runtime_log(
            state,
            format!(
                "[updater] live compare up_to_date current={} latest={}",
                CURRENT_VERSION, candidate.version
            ),
        );
        progress_updater_step(
            state,
            UPDATER_STEP_COMPARE,
            "done",
            "i18n:updater.detail.live_compare_uptodate",
        )?;
        mark_remaining_updater_steps_skipped(state, UPDATER_STEP_SECURITY)?;
        return finalize_updater_success(
            state,
            "up_to_date",
            "updater.summary.up_to_date",
            "i18n:updater.detail.live_up_to_date",
            "action.close",
        );
    }
    progress_updater_step(
        state,
        UPDATER_STEP_COMPARE,
        "done",
        "i18n:updater.detail.live_compare_done",
    )?;

    progress_updater_step(
        state,
        UPDATER_STEP_SECURITY,
        "running",
        "i18n:updater.detail.live_security_running",
    )?;
    let security_bundle = verify_release_security_bundle(&candidate)?;
    push_runtime_log(
        state,
        format!(
            "[updater] release security verified checksums_bytes={}",
            security_bundle.checksums_text.len()
        ),
    );
    progress_updater_step(
        state,
        UPDATER_STEP_SECURITY,
        "done",
        "i18n:updater.detail.live_security_done",
    )?;

    let asset_name = candidate
        .asset_name
        .clone()
        .ok_or_else(|| "release asset name is missing".to_string())?;
    let asset_url = candidate
        .asset_url
        .clone()
        .ok_or_else(|| "release asset url is missing".to_string())?;
    progress_updater_step(
        state,
        UPDATER_STEP_DOWNLOAD,
        "running",
        "i18n:updater.detail.live_download_running",
    )?;
    let asset_bytes = download_release_bytes(&client, &asset_url, "release asset")?;
    push_runtime_log(
        state,
        format!(
            "[updater] asset downloaded name={} bytes={} url={}",
            asset_name,
            asset_bytes.len(),
            asset_url
        ),
    );
    progress_updater_step(
        state,
        UPDATER_STEP_DOWNLOAD,
        "done",
        "i18n:updater.detail.live_download_done",
    )?;

    progress_updater_step(
        state,
        UPDATER_STEP_CHECKSUM,
        "running",
        "i18n:updater.detail.live_checksum_running",
    )?;
    ensure_asset_matches_verified_checksum(&security_bundle, &asset_name, &asset_bytes)?;
    push_runtime_log(
        state,
        format!(
            "[updater] checksum verified asset={} sha256={}",
            asset_name,
            sha256_hex(&asset_bytes)
        ),
    );
    progress_updater_step(
        state,
        UPDATER_STEP_CHECKSUM,
        "done",
        "i18n:updater.detail.live_checksum_done",
    )?;

    progress_updater_step(
        state,
        UPDATER_STEP_INSTALL,
        "running",
        "i18n:updater.detail.live_install_running",
    )?;
    let staged_path = stage_verified_release_asset(state, &candidate, &asset_bytes)?;
    push_runtime_log(
        state,
        format!(
            "[updater] asset staged version={} asset={} path={} auto_apply={}",
            candidate.version,
            asset_name,
            staged_path.display(),
            can_auto_apply_asset(&asset_name)
        ),
    );
    progress_updater_step(
        state,
        UPDATER_STEP_INSTALL,
        "done",
        "i18n:updater.detail.live_install_done",
    )?;

    progress_updater_step(
        state,
        UPDATER_STEP_RELAUNCH,
        "done",
        "i18n:updater.detail.live_relaunch_done",
    )?;
    if can_auto_apply_asset(&asset_name) {
        push_runtime_log(
            state,
            format!(
                "[updater] flow ready_to_restart version={} asset={} awaiting close handoff",
                candidate.version, asset_name
            ),
        );
        let result = finalize_updater_success(
            state,
            "ready_to_restart",
            "updater.summary.ready_to_restart",
            "i18n:updater.detail.live_ready_to_restart",
            "updater.action.reboot",
        );
        if result.is_ok() && should_auto_close_updater_after_ready_to_restart(launch_mode) {
            schedule_updater_window_close_for_apply(state);
        }
        return result;
    }
    push_runtime_log(
        state,
        format!(
            "[updater] flow completed without auto-apply version={} asset={}",
            candidate.version, asset_name
        ),
    );
    finalize_updater_success(
        state,
        "completed",
        "updater.summary.complete",
        "i18n:updater.detail.live_complete",
        "action.close",
    )
}

fn run_update_cycle(state: &AppState, manual: bool) -> Result<AppUpdateView, String> {
    let latest_release_url = resolve_latest_release_api_url();
    push_runtime_log(
        state,
        format!(
            "[updater] check start manual={} version={} url={} user_agent={}",
            manual, CURRENT_VERSION, latest_release_url, USER_AGENT
        ),
    );
    let client = build_release_http_client(Duration::from_secs(20), false)
        .map_err(|e| format!("build update http client: {e}"))?;

    let result = fetch_latest_release_from_url(&client, &latest_release_url);
    let mut store = refresh_update_store_snapshot(state)?;

    store.last_checked_at = Some(now_iso());
    store.last_checked_epoch_ms = Some(now_epoch_ms());

    match result {
        Ok(candidate) => {
            push_runtime_log(
                state,
                format!(
                "[updater] latest release discovered version={} release_url={}",
                candidate.version, candidate.release_url
            ),
        );
            push_runtime_log(
                state,
                format!(
                    "[updater] selected asset type={} handoff={} reason={}",
                    candidate.asset_type.as_deref().unwrap_or("unknown"),
                    candidate.install_handoff_mode.as_deref().unwrap_or("unknown"),
                    candidate
                        .asset_selection_reason
                        .as_deref()
                        .unwrap_or("unspecified")
                ),
            );
            store.latest_version = Some(candidate.version.clone());
            store.release_url = Some(candidate.release_url.clone());
            store.has_update = is_version_newer(&candidate.version, CURRENT_VERSION);
            store.selected_asset_type = candidate.asset_type.clone();
            store.selected_asset_reason = candidate.asset_selection_reason.clone();
            store.install_handoff_mode = candidate.install_handoff_mode.clone();
            store.last_error = None;
            if store.has_update {
                store.status = "available".to_string();
                if manual {
                    match spawn_updater_process(&state.app_handle, UpdaterLaunchMode::Auto) {
                        Ok(()) => {
                            store.updater_handoff_version = Some(candidate.version.clone());
                            store.status = "handoff".to_string();
                            push_runtime_log(
                                state,
                                format!(
                                    "[updater] handoff started version={} mode=manual",
                                    candidate.version
                                ),
                            );
                        }
                        Err(error) => {
                            push_runtime_log(state, format!("[updater] handoff failed: {error}"));
                            store.status = "error".to_string();
                            store.last_error = Some(error);
                        }
                    }
                } else if store.auto_update_enabled {
                    if should_launch_external_updater(&store, &candidate) {
                        match spawn_updater_process(&state.app_handle, UpdaterLaunchMode::Auto) {
                            Ok(()) => {
                                store.updater_handoff_version = Some(candidate.version.clone());
                                store.status = "handoff".to_string();
                                push_runtime_log(
                                    state,
                                    format!(
                                        "[updater] handoff started version={} mode=auto",
                                        candidate.version
                                    ),
                                );
                            }
                            Err(error) => {
                                push_runtime_log(state, format!("[updater] handoff failed: {error}"));
                                store.status = "error".to_string();
                                store.last_error = Some(error);
                            }
                        }
                    } else {
                        if let Err(error) = stage_release_if_needed(state, &mut store, &candidate) {
                            push_runtime_log(state, format!("[updater] stage release failed: {error}"));
                            store.status = "error".to_string();
                            store.last_error = Some(error);
                        }
                    }
                }
            } else {
                push_runtime_log(
                    state,
                    format!(
                        "[updater] no update available current={} latest={}",
                        CURRENT_VERSION, candidate.version
                    ),
                );
                clear_staged_update(&mut store);
                store.updater_handoff_version = None;
                store.status = "up_to_date".to_string();
            }
        }
        Err(error) => {
            push_runtime_log(state, format!("[updater] check failed: {error}"));
            store.status = "error".to_string();
            store.last_error = Some(error);
        }
    }

    write_update_store_snapshot(state, &store)?;
    Ok(to_view(&store))
}

fn stage_release_if_needed(
    state: &AppState,
    store: &mut AppUpdateStore,
    candidate: &ReleaseCandidate,
) -> Result<(), String> {
    let Some(asset_url) = candidate.asset_url.as_ref() else {
        store.status = "available".to_string();
        return Ok(());
    };
    let Some(asset_name) = candidate.asset_name.as_ref() else {
        store.status = "available".to_string();
        return Ok(());
    };

    if store.staged_version.as_deref() == Some(candidate.version.as_str())
        && store
            .staged_asset_path
            .as_deref()
            .map(Path::new)
            .is_some_and(Path::is_file)
    {
        push_runtime_log(
            state,
            format!(
                "[updater] reuse staged asset version={} path={}",
                candidate.version,
                store.staged_asset_path.as_deref().unwrap_or_default()
            ),
        );
        store.status = if can_auto_apply_asset(asset_name) {
            "downloaded".to_string()
        } else {
            "available".to_string()
        };
        store.selected_asset_type = candidate.asset_type.clone();
        store.selected_asset_reason = candidate.asset_selection_reason.clone();
        store.install_handoff_mode = candidate.install_handoff_mode.clone();
        store.pending_apply_on_exit = can_auto_apply_asset(asset_name);
        return Ok(());
    }

    let root = resolve_update_asset_root(state, asset_name)?;
    let target_dir = root.join(&candidate.version);
    fs::create_dir_all(&target_dir).map_err(|e| format!("create update dir: {e}"))?;
    let asset_path = target_dir.join(asset_name);

    let client = build_release_http_client(Duration::from_secs(60), true)
        .map_err(|e| format!("build update download client: {e}"))?;
    push_runtime_log(
        state,
        format!(
            "[updater] staging download start version={} asset={} target_dir={}",
            candidate.version,
            asset_name,
            target_dir.display()
        ),
    );
    let bytes = download_release_bytes(&client, asset_url, "update asset")?;
    verify_release_candidate(candidate, &bytes)?;
    fs::write(&asset_path, &bytes).map_err(|e| format!("write update asset: {e}"))?;
    push_runtime_log(
        state,
        format!(
            "[updater] staging complete asset={} bytes={} path={}",
            asset_name,
            bytes.len(),
            asset_path.display()
        ),
    );

    store.staged_version = Some(candidate.version.clone());
    store.staged_asset_name = Some(asset_name.clone());
    store.staged_asset_path = Some(asset_path.to_string_lossy().to_string());
    store.selected_asset_type = candidate.asset_type.clone();
    store.selected_asset_reason = candidate.asset_selection_reason.clone();
    store.install_handoff_mode = candidate.install_handoff_mode.clone();
    store.pending_apply_on_exit = can_auto_apply_asset(asset_name);
    store.status = if store.pending_apply_on_exit {
        "downloaded".to_string()
    } else {
        "available".to_string()
    };
    Ok(())
}

fn clear_staged_update(store: &mut AppUpdateStore) {
    store.staged_version = None;
    store.staged_asset_name = None;
    store.staged_asset_path = None;
    store.selected_asset_type = None;
    store.selected_asset_reason = None;
    store.install_handoff_mode = None;
    store.pending_apply_on_exit = false;
}

fn stage_verified_release_asset(
    state: &AppState,
    candidate: &ReleaseCandidate,
    asset_bytes: &[u8],
) -> Result<PathBuf, String> {
    let asset_name = candidate
        .asset_name
        .as_deref()
        .ok_or_else(|| "release asset name is missing".to_string())?;
    let root = resolve_update_asset_root(state, asset_name)?;
    let target_dir = root.join(&candidate.version);
    fs::create_dir_all(&target_dir).map_err(|e| format!("create update dir: {e}"))?;
    let asset_path = target_dir.join(asset_name);
    fs::write(&asset_path, asset_bytes).map_err(|e| format!("write update asset: {e}"))?;

    let mut store = state
        .app_update_store
        .lock()
        .map_err(|e| format!("lock app update store: {e}"))?;
    store.latest_version = Some(candidate.version.clone());
    store.release_url = Some(candidate.release_url.clone());
    store.has_update = true;
    store.staged_version = Some(candidate.version.clone());
    store.staged_asset_name = Some(asset_name.to_string());
    store.staged_asset_path = Some(asset_path.to_string_lossy().to_string());
    store.selected_asset_type = candidate.asset_type.clone();
    store.selected_asset_reason = candidate.asset_selection_reason.clone();
    store.install_handoff_mode = candidate.install_handoff_mode.clone();
    store.pending_apply_on_exit = can_auto_apply_asset(asset_name);
    store.status = if store.pending_apply_on_exit {
        "downloaded".to_string()
    } else {
        "available".to_string()
    };
    store.last_error = None;
    persist_update_store_from_state(state, &store)?;
    Ok(asset_path)
}

fn update_updater_overview<F>(state: &AppState, mut change: F) -> Result<(), String>
where
    F: FnMut(&mut UpdaterOverview),
{
    let overview = {
        let mut runtime = state
            .updater_runtime
            .lock()
            .map_err(|e| format!("lock updater runtime: {e}"))?;
        change(&mut runtime.overview);
        runtime.overview.clone()
    };
    let _ = state.app_handle.emit(UPDATER_EVENT_NAME, &overview);
    Ok(())
}

fn progress_updater_step(
    state: &AppState,
    step_id: &str,
    status: &str,
    detail: &str,
) -> Result<(), String> {
    update_updater_overview(state, |overview| {
        if let Some(step) = overview.steps.iter_mut().find(|item| item.id == step_id) {
            step.status = status.to_string();
            step.detail = detail.to_string();
        }
    })
}

fn mark_remaining_updater_steps_skipped(
    state: &AppState,
    from_step_id: &str,
) -> Result<(), String> {
    let mut should_mark = false;
    update_updater_overview(state, |overview| {
        for step in &mut overview.steps {
            if step.id == from_step_id {
                should_mark = true;
            }
            if should_mark && step.status == "idle" {
                step.status = "skipped".to_string();
                step.detail = "i18n:updater.detail.skipped".to_string();
            }
        }
    })
}

fn finalize_updater_success(
    state: &AppState,
    status: &str,
    summary_key: &str,
    summary_detail: &str,
    close_label_key: &str,
) -> Result<(), String> {
    update_updater_overview(state, |overview| {
        overview.status = status.to_string();
        overview.summary_key = summary_key.to_string();
        overview.summary_detail = summary_detail.to_string();
        overview.finished_at = Some(now_iso());
        overview.can_close = true;
        overview.close_label_key = close_label_key.to_string();
    })?;
    let mut runtime = state
        .updater_runtime
        .lock()
        .map_err(|e| format!("lock updater runtime: {e}"))?;
    runtime.running = false;
    Ok(())
}

fn finalize_updater_failure(state: &AppState, error: &str) -> Result<(), String> {
    update_updater_overview(state, |overview| {
        overview.status = "error".to_string();
        overview.summary_key = "updater.summary.failed".to_string();
        overview.summary_detail = error.to_string();
        overview.finished_at = Some(now_iso());
        overview.can_close = true;
        overview.close_label_key = "action.close".to_string();
    })?;
    let mut runtime = state
        .updater_runtime
        .lock()
        .map_err(|e| format!("lock updater runtime: {e}"))?;
    runtime.running = false;
    Ok(())
}

fn fetch_latest_release(client: &Client) -> Result<ReleaseCandidate, String> {
    let latest_release_url = resolve_latest_release_api_url();
    fetch_latest_release_from_url(client, &latest_release_url)
}

fn fetch_latest_release_from_url(
    client: &Client,
    latest_release_url: &str,
) -> Result<ReleaseCandidate, String> {
    let response = client
        .get(latest_release_url)
        .send()
        .map_err(|e| format!("request latest release: {e}"))?;
    if !response.status().is_success() {
        return Err(describe_http_failure(
            response,
            "latest release request",
            Some(latest_release_url),
        ));
    }
    let release: GithubRelease = response
        .json()
        .map_err(|e| format!("parse latest release payload: {e}"))?;
    let version = normalize_version(&release.tag_name);
    let asset = pick_release_asset(&release.assets);
    let checksums = release
        .assets
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case(RELEASE_CHECKSUMS_ASSET));
    let checksums_signature = release.assets.iter().find(|item| {
        item.name
            .eq_ignore_ascii_case(RELEASE_CHECKSUMS_SIGNATURE_ASSET)
    });
    Ok(ReleaseCandidate {
        version,
        release_url: release.html_url,
        asset_name: asset.map(|item| item.asset.name.clone()),
        asset_url: asset.map(|item| item.asset.browser_download_url.clone()),
        asset_type: asset.map(|item| item.kind.label().to_string()),
        asset_selection_reason: asset.map(|item| item.reason.to_string()),
        install_handoff_mode: asset.map(|item| item.kind.handoff_mode().to_string()),
        checksums_url: checksums.map(|item| item.browser_download_url.clone()),
        checksums_signature_url: checksums_signature.map(|item| item.browser_download_url.clone()),
    })
}

fn pick_release_asset(assets: &[GithubReleaseAsset]) -> Option<SelectedReleaseAsset<'_>> {
    let os = std::env::consts::OS;
    let install_mode = if os == "windows" {
        Some(current_windows_install_mode())
    } else {
        None
    };
    pick_release_asset_for_context(assets, os, install_mode.as_deref())
}

fn pick_release_asset_for_context<'a>(
    assets: &'a [GithubReleaseAsset],
    os: &str,
    windows_install_mode: Option<&str>,
) -> Option<SelectedReleaseAsset<'a>> {
    let candidates = assets
        .iter()
        .filter_map(|asset| classify_release_asset(os, asset))
        .collect::<Vec<_>>();

    if os == "windows" {
        let prefer_msi = windows_install_mode.unwrap_or("portable_zip") != "portable_zip";
        if prefer_msi {
            if let Some(asset) = candidates
                .iter()
                .copied()
                .find(|item| item.kind == SelectedAssetKind::WindowsMsi)
            {
                return Some(SelectedReleaseAsset {
                    reason: "windows_installed_context_prefers_msi",
                    ..asset
                });
            }
        }
        if let Some(asset) = candidates
            .iter()
            .copied()
            .find(|item| item.kind == SelectedAssetKind::WindowsZip)
        {
            return Some(SelectedReleaseAsset {
                reason: if prefer_msi {
                    "windows_portable_zip_fallback"
                } else {
                    "windows_portable_zip_primary"
                },
                ..asset
            });
        }
        if let Some(asset) = candidates
            .iter()
            .copied()
            .find(|item| item.kind == SelectedAssetKind::WindowsMsi)
        {
            return Some(SelectedReleaseAsset {
                reason: "windows_msi_fallback_when_zip_missing",
                ..asset
            });
        }
        if let Some(asset) = candidates
            .iter()
            .copied()
            .find(|item| item.kind == SelectedAssetKind::WindowsExe)
        {
            return Some(SelectedReleaseAsset {
                reason: "windows_manual_installer_fallback",
                ..asset
            });
        }
        return None;
    }

    candidates
        .into_iter()
        .min_by_key(|item| asset_rank(item.kind))
        .map(|item| SelectedReleaseAsset {
            reason: match item.kind {
                SelectedAssetKind::LinuxTarGz => "linux_tar_gz_primary",
                SelectedAssetKind::LinuxZip => "linux_zip_fallback",
                SelectedAssetKind::MacZip => "mac_zip_primary",
                _ => "generic_asset_selection",
            },
            ..item
        })
}

fn classify_release_asset<'a>(
    os: &str,
    asset: &'a GithubReleaseAsset,
) -> Option<SelectedReleaseAsset<'a>> {
    let name = asset.name.to_ascii_lowercase();
    match os {
        "windows" if name.ends_with(".msi") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::WindowsMsi,
            reason: "windows_msi_candidate",
        }),
        "windows" if name.ends_with(".zip") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::WindowsZip,
            reason: "windows_zip_candidate",
        }),
        "windows" if name.ends_with(".exe") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::WindowsExe,
            reason: "windows_exe_candidate",
        }),
        "linux" if name.ends_with(".tar.gz") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::LinuxTarGz,
            reason: "linux_tar_gz_candidate",
        }),
        "linux" if name.ends_with(".zip") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::LinuxZip,
            reason: "linux_zip_candidate",
        }),
        "macos" if name.ends_with(".zip") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::MacZip,
            reason: "mac_zip_candidate",
        }),
        _ => None,
    }
}

fn verify_release_candidate(
    candidate: &ReleaseCandidate,
    asset_bytes: &[u8],
) -> Result<(), String> {
    let asset_name = candidate
        .asset_name
        .as_deref()
        .ok_or_else(|| "release asset name is missing".to_string())?;
    let security_bundle = verify_release_security_bundle(candidate)?;
    ensure_asset_matches_verified_checksum(&security_bundle, asset_name, asset_bytes)
}

struct VerifiedReleaseSecurityBundle {
    checksums_text: String,
}

fn verify_release_security_bundle(
    candidate: &ReleaseCandidate,
) -> Result<VerifiedReleaseSecurityBundle, String> {
    let checksums_url = candidate
        .checksums_url
        .as_deref()
        .ok_or_else(|| "release checksums asset is missing".to_string())?;
    let signature_url = candidate
        .checksums_signature_url
        .as_deref()
        .ok_or_else(|| "release checksums signature asset is missing".to_string())?;

    let client = build_release_http_client(Duration::from_secs(30), false)
        .map_err(|e| format!("build checksum verification client: {e}"))?;
    let checksums_bytes = download_release_bytes(&client, checksums_url, "release checksums")?;
    let signature_bytes =
        download_release_bytes(&client, signature_url, "release checksums signature")?;
    verify_release_checksums_signature(&checksums_bytes, &signature_bytes)?;
    let checksums_text = String::from_utf8(checksums_bytes)
        .map_err(|e| format!("decode release checksums as utf8: {e}"))?;
    Ok(VerifiedReleaseSecurityBundle { checksums_text })
}

fn ensure_asset_matches_verified_checksum(
    security_bundle: &VerifiedReleaseSecurityBundle,
    asset_name: &str,
    asset_bytes: &[u8],
) -> Result<(), String> {
    let expected_sha256 = extract_checksum_for_asset(&security_bundle.checksums_text, asset_name)
        .ok_or_else(|| format!("signed checksums do not include {asset_name}"))?;
    let actual_sha256 = sha256_hex(asset_bytes);
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256.trim()) {
        return Err(format!(
            "update asset checksum mismatch for {asset_name}: expected {}, got {}",
            expected_sha256.trim(),
            actual_sha256
        ));
    }
    Ok(())
}

fn download_release_bytes(client: &Client, url: &str, label: &str) -> Result<Vec<u8>, String> {
    let mut last_error = String::new();
    for attempt in 1..=3 {
        let response = match client.get(url).send() {
            Ok(value) => value,
            Err(error) => {
                last_error = format!("download {label}: {error}");
                if attempt < 3 {
                    thread::sleep(Duration::from_millis(250 * attempt as u64));
                    continue;
                }
                break;
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            last_error = describe_http_failure(response, &format!("download {label}"), Some(url));
            if status.is_server_error() && attempt < 3 {
                thread::sleep(Duration::from_millis(250 * attempt as u64));
                continue;
            }
            break;
        }
        match response.bytes() {
            Ok(value) => return Ok(value.to_vec()),
            Err(error) => {
                last_error = format!("read {label} body: {error}");
                if attempt < 3 {
                    thread::sleep(Duration::from_millis(250 * attempt as u64));
                    continue;
                }
                break;
            }
        }
    }
    Err(last_error)
}

fn build_release_http_client(
    timeout: Duration,
    disable_auto_decompression: bool,
) -> Result<Client, String> {
    let mut builder = Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(20))
        .user_agent(USER_AGENT);
    if disable_auto_decompression {
        builder = builder.no_gzip().no_brotli().no_deflate();
    }
    builder
        .build()
        .map_err(|e| format!("build release http client: {e}"))
}

fn describe_http_failure(response: Response, context: &str, url: Option<&str>) -> String {
    let status = response.status();
    let headers = response.headers().clone();
    let request_id = headers
        .get("x-github-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .trim()
        .to_string();
    let content_type = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .trim()
        .to_string();
    let body = response.text().unwrap_or_default();
    let body_snippet = sanitize_http_error_body(&body);
    let mut parts = vec![format!("{context} failed with HTTP {status}")];
    if let Some(target) = url.filter(|value| !value.trim().is_empty()) {
        parts.push(format!("url={target}"));
    }
    if !request_id.is_empty() {
        parts.push(format!("request_id={request_id}"));
    }
    if !content_type.is_empty() {
        parts.push(format!("content_type={content_type}"));
    }
    if !body_snippet.is_empty() {
        parts.push(format!("body={body_snippet}"));
    }
    parts.join(" | ")
}

fn sanitize_http_error_body(body: &str) -> String {
    let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut snippet = trimmed.chars().take(600).collect::<String>();
    if trimmed.chars().count() > 600 {
        snippet.push_str("...");
    }
    snippet
}

fn verify_release_checksums_signature(
    checksums_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<(), String> {
    let signature_b64 = String::from_utf8(signature_bytes.to_vec())
        .map_err(|e| format!("decode release signature as utf8: {e}"))?;
    let raw_signature = signature_b64.trim();
    let variants = signature_verification_variants(checksums_bytes);
    let mut last_error = String::new();
    for variant in variants {
        match verify_release_checksums_signature_variant(&variant, raw_signature) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

fn release_signing_public_keys() -> Vec<String> {
    let mut keys = Vec::new();
    let current = RELEASE_SIGNING_PUBLIC_KEY_XML.trim();
    if !current.is_empty() {
        keys.push(current.to_string());
    }

    if let Ok(legacy_keys) = serde_json::from_str::<Vec<String>>(RELEASE_SIGNING_LEGACY_PUBLIC_KEYS_JSON)
    {
        for key in legacy_keys {
            let trimmed = key.trim();
            if !trimmed.is_empty() && keys.iter().all(|existing| existing != trimmed) {
                keys.push(trimmed.to_string());
            }
        }
    }

    keys
}

fn verify_release_checksums_signature_variant(
    checksums_bytes: &[u8],
    signature_b64: &str,
) -> Result<(), String> {
    let mut last_error = String::new();
    for public_key_xml in release_signing_public_keys() {
        match verify_release_checksums_signature_variant_with_key(
            checksums_bytes,
            signature_b64,
            &public_key_xml,
        ) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

fn verify_release_checksums_signature_variant_with_key(
    checksums_bytes: &[u8],
    signature_b64: &str,
    public_key_xml: &str,
) -> Result<(), String> {
    let script = r#"
$publicXml = @'
__PUBLIC_KEY_XML__
'@
$checksumsB64 = [Environment]::GetEnvironmentVariable('__CHECKSUMS_ENV__')
$signatureB64 = [Environment]::GetEnvironmentVariable('__SIGNATURE_ENV__')
if ([string]::IsNullOrWhiteSpace($checksumsB64) -or [string]::IsNullOrWhiteSpace($signatureB64)) {
  exit 2
}
$checksums = [Convert]::FromBase64String($checksumsB64)
$signature = [Convert]::FromBase64String($signatureB64)
$rsa = New-Object System.Security.Cryptography.RSACryptoServiceProvider
$rsa.PersistKeyInCsp = $false
$rsa.FromXmlString($publicXml)
$sha = [System.Security.Cryptography.SHA256]::Create()
if (-not $rsa.VerifyData($checksums, $sha, $signature)) {
  exit 1
}
"#
    .replace("__PUBLIC_KEY_XML__", public_key_xml)
    .replace("__CHECKSUMS_ENV__", RELEASE_CHECKSUMS_B64_ENV)
    .replace("__SIGNATURE_ENV__", RELEASE_CHECKSUMS_SIGNATURE_B64_ENV);

    let mut command = Command::new("powershell");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &script,
    ]);
    command.env(RELEASE_CHECKSUMS_B64_ENV, B64.encode(checksums_bytes));
    command.env(RELEASE_CHECKSUMS_SIGNATURE_B64_ENV, signature_b64);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .map_err(|e| format!("run checksum signature verification: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(format!(
        "release checksum signature verification failed (code {:?}){}{}",
        output.status.code(),
        if stderr.is_empty() {
            String::new()
        } else {
            format!(" stderr: {stderr}")
        },
        if stdout.is_empty() {
            String::new()
        } else {
            format!(" stdout: {stdout}")
        }
    ))
}

fn signature_verification_variants(checksums_bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut variants = vec![checksums_bytes.to_vec()];
    let Ok(text) = String::from_utf8(checksums_bytes.to_vec()) else {
        return variants;
    };

    let normalized_lf = text.replace("\r\n", "\n").replace('\r', "\n");
    for candidate in [normalized_lf.clone(), normalized_lf.replace('\n', "\r\n")] {
        let candidate_bytes = candidate.into_bytes();
        if variants.iter().all(|existing| existing != &candidate_bytes) {
            variants.push(candidate_bytes);
        }
    }
    variants
}

fn extract_checksum_for_asset<'a>(checksums_text: &'a str, asset_name: &str) -> Option<&'a str> {
    for line in checksums_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let hash = parts.next()?;
        let entry = parts.next()?;
        let normalized = entry.replace('\\', "/");
        if normalized == asset_name || normalized.ends_with(&format!("/{asset_name}")) {
            return Some(hash);
        }
    }
    None
}

fn asset_rank(kind: SelectedAssetKind) -> u8 {
    match kind {
        SelectedAssetKind::WindowsMsi => 0,
        SelectedAssetKind::WindowsZip => 1,
        SelectedAssetKind::WindowsExe => 2,
        SelectedAssetKind::LinuxTarGz => 0,
        SelectedAssetKind::LinuxZip => 1,
        SelectedAssetKind::MacZip => 0,
    }
}

fn can_auto_apply_asset(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".msi")
}

fn uses_msi_installer(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".msi")
}

fn resolve_latest_release_api_url() -> String {
    std::env::var(RELEASE_LATEST_API_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| GITHUB_LATEST_RELEASE_API.to_string())
}

fn resolve_update_asset_root(state: &AppState, asset_name: &str) -> Result<PathBuf, String> {
    if uses_msi_installer(asset_name) {
        let app_root = app_local_data_root(&state.app_handle)
            .map_err(|e| format!("resolve app local data root for msi staging: {e}"))?;
        let namespace_hash = sha256_hex(app_root.to_string_lossy().as_bytes());
        let namespace = &namespace_hash[..16];
        return Ok(std::env::temp_dir()
            .join("cerbena-browser-updates")
            .join(namespace));
    }
    state
        .app_update_root_path(&state.app_handle)
        .map_err(|e| format!("resolve update root path: {e}"))
}

fn launch_zip_apply_helper(
    pid: u32,
    archive_path: &Path,
    install_root: &Path,
    relaunch_executable: Option<&Path>,
    runtime_log_path: Option<&str>,
) -> Result<(), String> {
    let helper = build_zip_apply_helper_script(
        pid,
        archive_path,
        install_root,
        relaunch_executable,
        runtime_log_path,
    );
    Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &helper,
        ])
        .spawn()
        .map_err(|e| format!("spawn zip update helper: {e}"))?;
    Ok(())
}

fn build_zip_apply_helper_script(
    pid: u32,
    archive_path: &Path,
    install_root: &Path,
    relaunch_executable: Option<&Path>,
    runtime_log_path: Option<&str>,
) -> String {
    let relaunch = relaunch_executable
        .map(powershell_quote)
        .unwrap_or_else(|| "$null".to_string());
    let runtime_log = runtime_log_path
        .map(|value| powershell_quote(Path::new(value)))
        .unwrap_or_else(|| "$null".to_string());
    format!(
        "$pidValue={pid};\
        $archive={archive};\
        $installRoot={install};\
        $relaunchExe={relaunch};\
        $runtimeLogPath={runtime_log};\
        $versionProbe=$env:CERBENA_SELFTEST_REPORT_VERSION_FILE;\
        $autoExitAfter='20';\
        $targetExecutables=@('cerbena.exe','browser-desktop-ui.exe','cerbena-updater.exe');\
        function Write-Log([string]$message) {{\
            if (-not $runtimeLogPath -or [string]::IsNullOrWhiteSpace($runtimeLogPath)) {{ return }};\
            try {{\
                $directory = Split-Path -Parent $runtimeLogPath;\
                if ($directory) {{ [System.IO.Directory]::CreateDirectory($directory) | Out-Null }};\
                [System.IO.File]::AppendAllText($runtimeLogPath, ('[' + [DateTime]::UtcNow.ToString('o') + '] [updater-helper][zip] ' + $message + [Environment]::NewLine), (New-Object System.Text.UTF8Encoding($false)));\
            }} catch {{}}\
        }};\
        Write-Log ('helper started pid=' + $pidValue + ' archive=' + $archive + ' installRoot=' + $installRoot);\
        while (Get-Process -Id $pidValue -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 250 }};\
        Write-Log 'launcher process exited; starting zip apply';\
        $targetPaths=@();\
        foreach ($exeName in $targetExecutables) {{\
            $candidate=Join-Path $installRoot $exeName;\
            if (Test-Path -LiteralPath $candidate) {{\
                $targetPaths += [System.IO.Path]::GetFullPath($candidate);\
            }}\
        }};\
        $runningTargets=@(Get-Process -ErrorAction SilentlyContinue | Where-Object {{\
            $_.Id -ne $PID -and $_.Id -ne $pidValue\
        }} | Where-Object {{\
            try {{\
                $processPath=$_.Path;\
                $processPath -and ($targetPaths -contains [System.IO.Path]::GetFullPath($processPath))\
            }} catch {{\
                $false\
            }}\
        }});\
        foreach ($proc in $runningTargets) {{\
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue;\
        }};\
        Write-Log ('stopped running targets count=' + $runningTargets.Count);\
        foreach ($proc in $runningTargets) {{\
            try {{\
                $proc.WaitForExit(15000) | Out-Null;\
            }} catch {{}}\
        }};\
        $temp=Join-Path ([System.IO.Path]::GetTempPath()) ('cerbena-update-' + [guid]::NewGuid().ToString('N'));\
        New-Item -ItemType Directory -Path $temp -Force | Out-Null;\
        try {{\
            Write-Log ('expanding archive to temp=' + $temp);\
            Expand-Archive -LiteralPath $archive -DestinationPath $temp -Force;\
            $source=$temp;\
            $entries=Get-ChildItem -LiteralPath $temp;\
            if ($entries.Count -eq 1 -and $entries[0].PSIsContainer) {{ $source=$entries[0].FullName }};\
            $copySucceeded=$false;\
            for ($attempt=0; $attempt -lt 10 -and -not $copySucceeded; $attempt++) {{\
                try {{\
                    Get-ChildItem -LiteralPath $source | ForEach-Object {{ Copy-Item -LiteralPath $_.FullName -Destination $installRoot -Recurse -Force }};\
                    $copySucceeded=$true;\
                    Write-Log ('copy succeeded attempt=' + ($attempt + 1));\
                }} catch {{\
                    Write-Log ('copy failed attempt=' + ($attempt + 1) + ' error=' + $_.Exception.Message);\
                    if ($attempt -ge 9) {{ throw }};\
                    Start-Sleep -Milliseconds 500;\
                }}\
            }};\
            if ($relaunchExe -and (Test-Path -LiteralPath $relaunchExe)) {{\
                $relaunchInfo = New-Object System.Diagnostics.ProcessStartInfo;\
                $relaunchInfo.FileName = $relaunchExe;\
                $relaunchInfo.WorkingDirectory = Split-Path -Parent $relaunchExe;\
                $relaunchInfo.UseShellExecute = $false;\
                if ($versionProbe) {{\
                    $relaunchInfo.EnvironmentVariables['CERBENA_SELFTEST_REPORT_VERSION_FILE'] = $versionProbe;\
                    $relaunchInfo.EnvironmentVariables['{auto_exit_env}'] = $autoExitAfter;\
                }};\
                if ($runtimeLogPath) {{\
                    $relaunchInfo.EnvironmentVariables['{helper_log_env}'] = $runtimeLogPath;\
                }};\
                [System.Diagnostics.Process]::Start($relaunchInfo) | Out-Null;\
                Write-Log ('relaunch started exe=' + $relaunchExe);\
            }} else {{\
                Write-Log 'relaunch skipped because executable is missing';\
            }}\
        }} catch {{\
            Write-Log ('helper failed: ' + $_.Exception.Message);\
            throw;\
        }} finally {{\
            Write-Log 'cleaning temporary extraction directory';\
            if (Test-Path -LiteralPath $temp) {{ Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue }}\
        }}",
        pid = pid,
        archive = powershell_quote(archive_path),
        install = powershell_quote(install_root),
        relaunch = relaunch,
        runtime_log = runtime_log,
        helper_log_env = UPDATER_HELPER_LOG_ENV,
        auto_exit_env = UPDATER_RELAUNCH_AUTO_EXIT_ENV
    )
}

fn launch_msi_apply_helper(
    pid: u32,
    msi_path: &Path,
    target_install_root: Option<&Path>,
    update_store_path: Option<&str>,
    target_version: Option<&str>,
    runtime_log_path: Option<&str>,
) -> Result<(), String> {
    let helper = build_msi_apply_helper_script(
        pid,
        msi_path,
        target_install_root,
        update_store_path,
        target_version,
        runtime_log_path,
    );
    Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &helper,
        ])
        .spawn()
        .map_err(|e| format!("spawn msi update helper: {e}"))?;
    Ok(())
}

fn build_msi_apply_helper_script(
    pid: u32,
    msi_path: &Path,
    target_install_root: Option<&Path>,
    update_store_path: Option<&str>,
    target_version: Option<&str>,
    runtime_log_path: Option<&str>,
) -> String {
    let store = update_store_path
        .map(|value| powershell_quote(Path::new(value)))
        .unwrap_or_else(|| "$null".to_string());
    let version = target_version
        .map(|value| format!("'{}'", value.replace('\'', "''")))
        .unwrap_or_else(|| "$null".to_string());
    let install_root = target_install_root
        .map(powershell_quote)
        .unwrap_or_else(|| "$null".to_string());
    let runtime_log = runtime_log_path
        .map(|value| powershell_quote(Path::new(value)))
        .unwrap_or_else(|| "$null".to_string());
    format!(
        "$ErrorActionPreference='Stop';\
        $pidValue={pid};\
        $msiPath={msi};\
        $installRoot={install};\
        $storePath={store};\
        $targetVersion={version};\
        $runtimeLogPath={runtime_log};\
        $msiLogPath=([System.IO.Path]::ChangeExtension($msiPath, '.msiexec.log'));\
        $msiInstallDirOverride=$env:{install_dir_env};\
        $msiWaitTimeoutMs = 120000;\
        if ($env:{msi_timeout_env}) {{\
            try {{\
                $parsedTimeout = [int]$env:{msi_timeout_env};\
                if ($parsedTimeout -ge 15000) {{ $msiWaitTimeoutMs = $parsedTimeout }};\
            }} catch {{}}\
        }};\
        $versionProbe=$env:CERBENA_SELFTEST_REPORT_VERSION_FILE;\
        $autoExitAfter='20';\
        $targetExecutables=@('cerbena.exe','browser-desktop-ui.exe','cerbena-updater.exe','cerbena-launcher.exe');\
        function Write-Log([string]$message) {{\
            if (-not $runtimeLogPath -or [string]::IsNullOrWhiteSpace($runtimeLogPath)) {{ return }};\
            try {{\
                $directory = Split-Path -Parent $runtimeLogPath;\
                if ($directory) {{ [System.IO.Directory]::CreateDirectory($directory) | Out-Null }};\
                [System.IO.File]::AppendAllText($runtimeLogPath, ('[' + [DateTime]::UtcNow.ToString('o') + '] [updater-helper][msi] ' + $message + [Environment]::NewLine), (New-Object System.Text.UTF8Encoding($false)));\
            }} catch {{}}\
        }};\
        function Describe-MsiExit([int]$code) {{\
            switch ($code) {{\
                1602 {{ return 'msi install canceled before completion' }}\
                1618 {{ return 'another Windows Installer transaction is already running (1618)' }}\
                3010 {{ return 'msi install completed and requested a relaunch (3010)' }}\
                default {{ return ('msi install failed with exit code ' + $code) }}\
            }}\
        }};\
        function Resolve-RelaunchExecutable() {{\
            if (-not $installRoot -or [string]::IsNullOrWhiteSpace($installRoot) -or -not (Test-Path -LiteralPath $installRoot)) {{ return $null }};\
            foreach ($exeName in @('cerbena.exe','browser-desktop-ui.exe')) {{\
                $candidate = Join-Path $installRoot $exeName;\
                if (Test-Path -LiteralPath $candidate) {{ return $candidate }};\
            }};\
            return $null;\
        }};\
        function Update-Store([string]$status, [string]$lastError, [bool]$pendingApply) {{\
            if (-not $storePath -or [string]::IsNullOrWhiteSpace($storePath) -or -not (Test-Path -LiteralPath $storePath)) {{ return }};\
            try {{\
                $json = Get-Content -LiteralPath $storePath -Raw | ConvertFrom-Json;\
                $json.status = $status;\
                $json.lastError = if ([string]::IsNullOrWhiteSpace($lastError)) {{ $null }} else {{ $lastError }};\
                $json.pendingApplyOnExit = $pendingApply;\
                if ($targetVersion) {{ $json.stagedVersion = $targetVersion }};\
                $updated = $json | ConvertTo-Json -Depth 8;\
                [System.IO.File]::WriteAllText($storePath, $updated, (New-Object System.Text.UTF8Encoding($false)));\
                Write-Log ('store updated status=' + $status + ' pending=' + $pendingApply + ' error=' + $lastError);\
            }} catch {{}}\
        }};\
        try {{\
        Write-Log ('helper started pid=' + $pidValue + ' msi=' + $msiPath + ' installRoot=' + $installRoot + ' installDirOverride=' + $msiInstallDirOverride + ' store=' + $storePath + ' targetVersion=' + $targetVersion);\
        while (Get-Process -Id $pidValue -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 250 }};\
        Write-Log 'launcher process exited; starting msi apply';\
        Update-Store 'applying' $null $false;\
        $targetPaths=@();\
        if ($installRoot -and (Test-Path -LiteralPath $installRoot)) {{\
            foreach ($exeName in $targetExecutables) {{\
                $candidate=Join-Path $installRoot $exeName;\
                if (Test-Path -LiteralPath $candidate) {{\
                    $targetPaths += [System.IO.Path]::GetFullPath($candidate);\
                }}\
            }};\
        }};\
        $runningTargets=@(Get-Process -ErrorAction SilentlyContinue | Where-Object {{\
            $_.Id -ne $PID -and $_.Id -ne $pidValue\
        }} | Where-Object {{\
            try {{\
                $processPath=$_.Path;\
                $processPath -and ($targetPaths -contains [System.IO.Path]::GetFullPath($processPath))\
            }} catch {{\
                $false\
            }}\
        }});\
        foreach ($proc in $runningTargets) {{\
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue;\
        }};\
        Write-Log ('stopped running targets count=' + $runningTargets.Count);\
        foreach ($proc in $runningTargets) {{\
            try {{\
                $proc.WaitForExit(15000) | Out-Null;\
            }} catch {{}}\
        }};\
        $msiArgs=@('/i', $msiPath, '/qn', '/norestart', '/l*v', $msiLogPath);\
        if ($msiInstallDirOverride -and -not [string]::IsNullOrWhiteSpace($msiInstallDirOverride)) {{\
            $msiArgs += ('INSTALLDIR=\"' + $msiInstallDirOverride + '\"');\
        }};\
        $attempt = 0;\
        $maxAttempts = 3;\
        $exitCode = -1;\
        $completed = $false;\
        while (-not $completed -and $attempt -lt $maxAttempts) {{\
            $attempt++;\
            Write-Log ('invoking msiexec attempt=' + $attempt + ' path=' + $msiPath + ' log=' + $msiLogPath + ' installRoot=' + $installRoot + ' installDirOverride=' + $msiInstallDirOverride);\
            $existingMsiexec = @(Get-CimInstance Win32_Process -Filter \"Name = 'msiexec.exe'\" -ErrorAction SilentlyContinue);\
            Write-Log ('msiexec pre-spawn processCount=' + $existingMsiexec.Count + ' attempt=' + $attempt);\
            $proc = $null;\
            try {{\
                $proc = Start-Process -FilePath 'msiexec.exe' -ArgumentList $msiArgs -WindowStyle Hidden -PassThru -ErrorAction Stop;\
            }} catch {{\
                $spawnError = $_.Exception.Message;\
                Write-Log ('msiexec spawn failed attempt=' + $attempt + ' error=' + $spawnError);\
                Update-Store 'error' ('msiexec spawn failed: ' + $spawnError + '; verbose log: ' + $msiLogPath) $false;\
                exit 125;\
            }};\
            if ($null -eq $proc) {{\
                Write-Log ('msiexec spawn returned null attempt=' + $attempt);\
                Update-Store 'error' ('msiexec spawn returned null process; verbose log: ' + $msiLogPath) $false;\
                exit 126;\
            }};\
            $null = $proc.Handle;\
            $timedOut = $false;\
            Write-Log ('msiexec spawned pid=' + $proc.Id + ' attempt=' + $attempt);\
            $waitStartedAt = [DateTime]::UtcNow;\
            $lastWaitHeartbeatAt = $waitStartedAt.AddSeconds(-2);\
            while (-not $proc.WaitForExit(1000)) {{\
                $elapsedMs = [int]([DateTime]::UtcNow - $waitStartedAt).TotalMilliseconds;\
                if ($elapsedMs -ge $msiWaitTimeoutMs) {{\
                    $timedOut = $true;\
                    break;\
                }};\
                if ([DateTime]::UtcNow -ge $lastWaitHeartbeatAt.AddSeconds(2)) {{\
                    Write-Log ('msiexec wait heartbeat attempt=' + $attempt + ' pid=' + $proc.Id + ' elapsedMs=' + $elapsedMs + ' timeoutMs=' + $msiWaitTimeoutMs);\
                    $lastWaitHeartbeatAt = [DateTime]::UtcNow;\
                }};\
            }};\
            if ($timedOut) {{\
                Write-Log ('msiexec timed out after ' + $msiWaitTimeoutMs + 'ms attempt=' + $attempt + ' pid=' + $proc.Id + ' log=' + $msiLogPath);\
                try {{ Start-Process -FilePath 'taskkill.exe' -ArgumentList @('/PID', [string]$proc.Id, '/T', '/F') -WindowStyle Hidden -Wait | Out-Null }} catch {{}};\
                try {{ $proc.Kill() }} catch {{}};\
                try {{ $proc.WaitForExit(10000) | Out-Null }} catch {{}};\
            }};\
            if ($timedOut) {{\
                Update-Store 'error' ('msiexec timed out; verbose log: ' + $msiLogPath) $false;\
                exit 124;\
            }};\
            $exitCode = $proc.ExitCode;\
            Write-Log ('msiexec completed attempt=' + $attempt + ' exitCode=' + $exitCode + ' log=' + $msiLogPath);\
            if (Test-Path -LiteralPath $msiLogPath) {{\
                try {{\
                    $msiLogSize = (Get-Item -LiteralPath $msiLogPath -ErrorAction Stop).Length;\
                    Write-Log ('msiexec log detected sizeBytes=' + $msiLogSize + ' attempt=' + $attempt + ' path=' + $msiLogPath);\
                }} catch {{\
                    Write-Log ('msiexec log stat failed attempt=' + $attempt + ' path=' + $msiLogPath + ' error=' + $_.Exception.Message);\
                }};\
            }} else {{\
                Write-Log ('msiexec log missing after completion attempt=' + $attempt + ' path=' + $msiLogPath);\
            }};\
            if ($exitCode -eq 1618 -and $attempt -lt $maxAttempts) {{\
                Write-Log ('msiexec returned 1618; retrying attempt=' + ($attempt + 1));\
                Start-Sleep -Seconds 5;\
                continue;\
            }};\
            $completed = $true;\
        }};\
        if (-not $completed) {{\
            Update-Store 'error' ('msiexec did not complete after retries; verbose log: ' + $msiLogPath) $false;\
            exit 1618;\
        }};\
        if ($exitCode -eq 1602) {{\
            Update-Store 'canceled' ((Describe-MsiExit $exitCode) + '; verbose log: ' + $msiLogPath) $false;\
            exit $exitCode;\
        }};\
        if ($exitCode -ne 0 -and $exitCode -ne 3010) {{\
            Update-Store 'error' ((Describe-MsiExit $exitCode) + '; verbose log: ' + $msiLogPath) $false;\
            exit $exitCode;\
        }};\
        Update-Store 'applied_pending_relaunch' $null $false;\
        $relaunchExe = Resolve-RelaunchExecutable;\
        if ($relaunchExe -and (Test-Path -LiteralPath $relaunchExe)) {{\
            $relaunchInfo = New-Object System.Diagnostics.ProcessStartInfo;\
            $relaunchInfo.FileName = $relaunchExe;\
            $relaunchInfo.WorkingDirectory = Split-Path -Parent $relaunchExe;\
            $relaunchInfo.UseShellExecute = $false;\
            if ($versionProbe) {{\
                $relaunchInfo.EnvironmentVariables['CERBENA_SELFTEST_REPORT_VERSION_FILE'] = $versionProbe;\
                $relaunchInfo.EnvironmentVariables['{auto_exit_env}'] = $autoExitAfter;\
            }};\
            if ($runtimeLogPath) {{\
                $relaunchInfo.EnvironmentVariables['{helper_log_env}'] = $runtimeLogPath;\
            }};\
            [System.Diagnostics.Process]::Start($relaunchInfo) | Out-Null;\
            Write-Log ('relaunch started exe=' + $relaunchExe);\
        }} else {{\
            Write-Log ('relaunch skipped because executable is missing installRoot=' + $installRoot);\
        }};\
        }} catch {{\
            $message = $_.Exception.Message;\
            Write-Log ('helper exception: ' + $message);\
            Update-Store 'error' ('helper exception: ' + $message + '; verbose log: ' + $msiLogPath) $false;\
            exit 1;\
        }}",
        pid = pid,
        msi = powershell_quote(msi_path),
        install = install_root,
        store = store,
        version = version,
        runtime_log = runtime_log,
        install_dir_env = UPDATER_MSI_INSTALL_DIR_ENV,
        msi_timeout_env = UPDATER_MSI_TIMEOUT_MS_ENV,
        helper_log_env = UPDATER_HELPER_LOG_ENV,
        auto_exit_env = UPDATER_RELAUNCH_AUTO_EXIT_ENV
    )
}

fn resolve_relaunch_executable_path(install_root: &Path) -> Option<PathBuf> {
    let candidates = [
        install_root.join("cerbena.exe"),
        install_root.join("browser-desktop-ui.exe"),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

fn persist_update_store_from_state(state: &AppState, store: &AppUpdateStore) -> Result<(), String> {
    let path = state
        .app_update_store_path(&state.app_handle)
        .map_err(|e| format!("resolve app update store path: {e}"))?;
    persist_app_update_store(&path, store)
}

fn refresh_update_store_snapshot(state: &AppState) -> Result<AppUpdateStore, String> {
    let path = state
        .app_update_store_path(&state.app_handle)
        .map_err(|e| format!("resolve app update store path: {e}"))?;
    let disk_store = if path.exists() {
        let raw = fs::read(&path).map_err(|e| format!("read app update store: {e}"))?;
        serde_json::from_slice::<AppUpdateStore>(&raw)
            .map_err(|e| format!("parse app update store: {e}"))?
    } else {
        AppUpdateStore::default()
    };
    let mut disk_store = disk_store;
    reconcile_update_store_with_current_version(&mut disk_store);
    push_runtime_log(
        state,
        format!(
            "[updater] update store snapshot refreshed status={} latest={} staged={} handoff={} pending_apply={}",
            disk_store.status,
            disk_store.latest_version.as_deref().unwrap_or("none"),
            disk_store.staged_version.as_deref().unwrap_or("none"),
            disk_store
                .updater_handoff_version
                .as_deref()
                .unwrap_or("none"),
            disk_store.pending_apply_on_exit
        ),
    );
    let mut guard = state
        .app_update_store
        .lock()
        .map_err(|e| format!("lock app update store: {e}"))?;
    *guard = disk_store.clone();
    Ok(disk_store)
}

fn reconcile_update_store_with_current_version(store: &mut AppUpdateStore) {
    let staged_is_current_or_older = store
        .staged_version
        .as_deref()
        .map(|version| !is_version_newer(version, CURRENT_VERSION))
        .unwrap_or(false);
    let handoff_is_current_or_older = store
        .updater_handoff_version
        .as_deref()
        .map(|version| !is_version_newer(version, CURRENT_VERSION))
        .unwrap_or(false);
    let stale_handoff_status = matches!(
        store.status.as_str(),
        "applying" | "downloaded" | "applied_pending_relaunch"
    );
    if staged_is_current_or_older || (handoff_is_current_or_older && stale_handoff_status) {
        clear_staged_update(store);
        store.updater_handoff_version = None;
        if stale_handoff_status {
            store.status = "up_to_date".to_string();
        }
        store.last_error = None;
        store.latest_version = Some(CURRENT_VERSION.to_string());
    }
}

fn write_update_store_snapshot(state: &AppState, store: &AppUpdateStore) -> Result<(), String> {
    {
        let mut guard = state
            .app_update_store
            .lock()
            .map_err(|e| format!("lock app update store: {e}"))?;
        *guard = store.clone();
    }
    persist_update_store_from_state(state, store)
}

fn to_view(store: &AppUpdateStore) -> AppUpdateView {
    AppUpdateView {
        current_version: CURRENT_VERSION.to_string(),
        repository_url: REPOSITORY_URL.to_string(),
        auto_update_enabled: store.auto_update_enabled,
        last_checked_at: store.last_checked_at.clone(),
        latest_version: store.latest_version.clone(),
        release_url: store.release_url.clone(),
        has_update: store.has_update,
        status: if store.status.trim().is_empty() {
            "idle".to_string()
        } else {
            store.status.clone()
        },
        last_error: store.last_error.clone(),
        staged_version: store.staged_version.clone(),
        staged_asset_name: store.staged_asset_name.clone(),
        selected_asset_type: store.selected_asset_type.clone(),
        selected_asset_reason: store.selected_asset_reason.clone(),
        install_handoff_mode: store.install_handoff_mode.clone(),
        can_auto_apply: store
            .staged_asset_name
            .as_deref()
            .map(can_auto_apply_asset)
            .unwrap_or(false),
    }
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default()
}

fn now_iso() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    seconds.to_string()
}

fn normalize_version(raw: &str) -> String {
    raw.trim().trim_start_matches('v').to_string()
}

fn is_version_newer(candidate: &str, current: &str) -> bool {
    let left = parse_version_parts(candidate);
    let right = parse_version_parts(current);
    left > right
}

fn parse_version_parts(value: &str) -> Vec<u64> {
    let normalized = normalize_version(value);
    let mut parts = normalized.splitn(2, '-');
    let base = parts.next().unwrap_or_default();
    let hotfix_suffix = parts.next().filter(|suffix| {
        !suffix.is_empty()
            && suffix
                .split('.')
                .all(|segment| !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_digit()))
    });

    let mut parsed = base
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    if let Some(suffix) = hotfix_suffix {
        parsed.extend(
            suffix
                .split('.')
                .map(|part| part.parse::<u64>().unwrap_or(0)),
        );
    }
    parsed
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::{
        asset_rank, build_msi_apply_helper_script, build_release_http_client,
        build_zip_apply_helper_script, can_auto_apply_asset, default_auto_update_enabled,
        download_release_bytes, ensure_asset_matches_verified_checksum,
        extract_checksum_for_asset, fetch_latest_release_from_url, is_version_newer,
        normalize_version, pick_release_asset_for_context,
        release_signing_public_keys, should_auto_close_updater_after_ready_to_restart,
        reconcile_update_store_with_current_version, resolve_latest_release_api_url,
        resolve_relaunch_executable_path, sha256_hex, should_run_auto_update_check,
        signature_verification_variants,
        AppUpdateStore, GithubReleaseAsset, SelectedAssetKind, UpdaterLaunchMode,
        VerifiedReleaseSecurityBundle, CURRENT_VERSION, RELEASE_LATEST_API_URL_ENV,
        RELEASE_CHECKSUMS_B64_ENV, RELEASE_CHECKSUMS_SIGNATURE_B64_ENV,
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        path::{Path, PathBuf},
        process::Command,
        thread,
        time::Duration,
    };

    fn next_release_version(version: &str) -> String {
        let normalized = normalize_version(version);
        let mut base_and_suffix = normalized.splitn(2, '-');
        let base = base_and_suffix.next().unwrap_or_default();
        let suffix = base_and_suffix.next();

        if let Some(hotfix_suffix) = suffix.filter(|value| {
            !value.is_empty()
                && value
                    .split('.')
                    .all(|segment| !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_digit()))
        }) {
            let mut hotfix_parts = hotfix_suffix
                .split('.')
                .map(|value| value.parse::<u64>().unwrap_or(0))
                .collect::<Vec<_>>();
            if let Some(last) = hotfix_parts.last_mut() {
                *last += 1;
            } else {
                hotfix_parts.push(1);
            }
            return format!(
                "{base}-{}",
                hotfix_parts
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(".")
            );
        }

        let mut parts = base
            .split('.')
            .map(|value| value.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>();
        if let Some(last) = parts.last_mut() {
            *last += 1;
        } else {
            parts.push(1);
        }
        parts
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }

    fn spawn_http_server(
        routes: Vec<(
            String,
            Vec<u8>,
            &'static str,
            Vec<(&'static str, &'static str)>,
        )>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test http server");
        let addr = listener.local_addr().expect("local addr");
        thread::spawn(move || {
            for _ in 0..routes.len() {
                let (mut stream, _) = listener.accept().expect("accept test connection");
                let mut buffer = [0u8; 8192];
                let read = stream.read(&mut buffer).expect("read request");
                let request = String::from_utf8_lossy(&buffer[..read]);
                let first_line = request.lines().next().unwrap_or_default();
                let path = first_line.split_whitespace().nth(1).unwrap_or("/");
                let (status, body, content_type, extra_headers) = routes
                    .iter()
                    .find(|(route, _, _, _)| route == path)
                    .map(|(_, body, content_type, headers)| {
                        ("200 OK", body.clone(), *content_type, headers.clone())
                    })
                    .unwrap_or_else(|| {
                        (
                            "404 Not Found",
                            b"not found".to_vec(),
                            "text/plain",
                            Vec::new(),
                        )
                    });
                let mut headers = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n",
                    body.len()
                );
                for (name, value) in extra_headers {
                    headers.push_str(&format!("{name}: {value}\r\n"));
                }
                headers.push_str("\r\n");
                stream.write_all(headers.as_bytes()).expect("write headers");
                stream.write_all(&body).expect("write body");
                stream.flush().expect("flush response");
            }
        });
        format!("http://{}", addr)
    }

    #[test]
    fn version_normalization_drops_leading_v() {
        assert_eq!(normalize_version("v1.2.3"), "1.2.3");
        assert_eq!(normalize_version("1.2.3"), "1.2.3");
    }

    #[test]
    fn newer_version_detection_uses_semver_like_order() {
        assert!(is_version_newer("1.2.4", "1.2.3"));
        assert!(is_version_newer("2.0.0", "1.9.9"));
        assert!(is_version_newer("1.0.4-1", "1.0.4"));
        assert!(!is_version_newer("1.2.3", "1.2.3"));
        assert!(!is_version_newer("1.2.2", "1.2.3"));
        assert!(!is_version_newer("1.0.4-preview", "1.0.4"));
    }

    #[test]
    fn next_release_version_advances_hotfix_versions() {
        let next = next_release_version("1.0.12-1");
        assert_eq!(next, "1.0.12-2");
        assert!(is_version_newer(&next, "1.0.12-1"));
    }

    #[test]
    fn auto_apply_support_is_limited_to_safe_asset_types() {
        assert!(can_auto_apply_asset("cerbena-windows.zip"));
        assert!(can_auto_apply_asset("cerbena-windows.msi"));
        assert!(!can_auto_apply_asset("cerbena-windows.exe"));
    }

    #[test]
    fn preferred_asset_order_keeps_zip_before_other_formats() {
        assert!(asset_rank(SelectedAssetKind::WindowsMsi) < asset_rank(SelectedAssetKind::WindowsZip));
        assert!(asset_rank(SelectedAssetKind::WindowsZip) < asset_rank(SelectedAssetKind::WindowsExe));
    }

    #[test]
    fn release_asset_picker_prefers_best_match_for_platform() {
        let assets = vec![
            GithubReleaseAsset {
                name: "cerbena-windows.msi".to_string(),
                browser_download_url: "https://example.invalid/1".to_string(),
            },
            GithubReleaseAsset {
                name: "cerbena-windows.zip".to_string(),
                browser_download_url: "https://example.invalid/2".to_string(),
            },
        ];
        let selected = pick_release_asset_for_context(&assets, "windows", Some("msi"))
            .expect("selected installed asset");
        assert_eq!(selected.asset.name, "cerbena-windows.msi");
        assert_eq!(selected.kind, SelectedAssetKind::WindowsMsi);
        assert_eq!(selected.reason, "windows_installed_context_prefers_msi");
    }

    #[test]
    fn release_asset_picker_prefers_zip_for_portable_windows_context() {
        let assets = vec![
            GithubReleaseAsset {
                name: "cerbena-windows.msi".to_string(),
                browser_download_url: "https://example.invalid/1".to_string(),
            },
            GithubReleaseAsset {
                name: "cerbena-windows.zip".to_string(),
                browser_download_url: "https://example.invalid/2".to_string(),
            },
        ];
        let selected = pick_release_asset_for_context(&assets, "windows", Some("portable_zip"))
            .expect("selected portable asset");
        assert_eq!(selected.asset.name, "cerbena-windows.zip");
        assert_eq!(selected.kind, SelectedAssetKind::WindowsZip);
        assert_eq!(selected.reason, "windows_portable_zip_primary");
    }

    #[test]
    fn release_asset_picker_falls_back_to_msi_when_zip_is_missing() {
        let assets = vec![GithubReleaseAsset {
            name: "cerbena-windows.msi".to_string(),
            browser_download_url: "https://example.invalid/1".to_string(),
        }];
        let selected = pick_release_asset_for_context(&assets, "windows", Some("portable_zip"))
            .expect("selected fallback asset");
        assert_eq!(selected.asset.name, "cerbena-windows.msi");
        assert_eq!(selected.kind, SelectedAssetKind::WindowsMsi);
        assert_eq!(selected.reason, "windows_msi_fallback_when_zip_missing");
    }

    #[test]
    fn checksum_extraction_matches_plain_and_nested_asset_paths() {
        let checksums = "\
abc123  cerbena-windows-x64.zip\n\
def456  cerbena-windows-x64/cerbena.exe\n";
        assert_eq!(
            extract_checksum_for_asset(checksums, "cerbena-windows-x64.zip"),
            Some("abc123")
        );
        assert_eq!(
            extract_checksum_for_asset(checksums, "cerbena.exe"),
            Some("def456")
        );
    }

    #[test]
    fn sha256_hex_matches_known_digest() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn signature_verification_variants_add_newline_fallbacks_once() {
        let variants = signature_verification_variants(b"alpha\r\nbeta\r\n");
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0], b"alpha\r\nbeta\r\n");
        assert_eq!(variants[1], b"alpha\nbeta\n");
    }

    #[test]
    fn release_signing_public_keys_include_current_and_legacy_keys() {
        let keys = release_signing_public_keys();
        assert!(keys.len() >= 2);
        assert!(keys.iter().any(|key| key.contains("1nCCvDQ4TOZjV1t78V3T3dIz")));
        assert!(keys.iter().any(|key| key.contains("sQ/dGNzpHEHiSUvpp8+h4axI")));
    }

    #[test]
    fn auto_update_scheduler_runs_without_prior_check_when_enabled() {
        let store = AppUpdateStore {
            auto_update_enabled: true,
            ..AppUpdateStore::default()
        };
        assert!(should_run_auto_update_check(&store));
    }

    #[test]
    fn missing_auto_update_field_defaults_to_enabled() {
        let store: AppUpdateStore = serde_json::from_str("{}").expect("deserialize update store");
        assert_eq!(store.auto_update_enabled, default_auto_update_enabled());
    }

    #[test]
    fn powershell_command_reads_checksum_payloads_from_environment() {
        let script = format!(
            "$a=[Environment]::GetEnvironmentVariable('{checksums}'); \
             $b=[Environment]::GetEnvironmentVariable('{signature}'); \
             if ([string]::IsNullOrWhiteSpace($a) -or [string]::IsNullOrWhiteSpace($b)) {{ exit 2 }}; \
             if ($a -eq 'alpha' -and $b -eq 'beta') {{ exit 0 }}; \
             exit 1",
            checksums = RELEASE_CHECKSUMS_B64_ENV,
            signature = RELEASE_CHECKSUMS_SIGNATURE_B64_ENV
        );
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .env(RELEASE_CHECKSUMS_B64_ENV, "alpha")
            .env(RELEASE_CHECKSUMS_SIGNATURE_B64_ENV, "beta")
            .output()
            .expect("run powershell env transport test");
        assert!(
            output.status.success(),
            "powershell env transport must succeed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn reconcile_update_store_clears_staged_update_once_current_version_is_installed() {
        let mut store = AppUpdateStore {
            latest_version: Some("1.0.6-1".to_string()),
            staged_version: Some(CURRENT_VERSION.to_string()),
            staged_asset_name: Some("cerbena-windows-x64.zip".to_string()),
            staged_asset_path: Some("C:/tmp/update.zip".to_string()),
            pending_apply_on_exit: true,
            updater_handoff_version: Some(CURRENT_VERSION.to_string()),
            status: "applying".to_string(),
            ..AppUpdateStore::default()
        };
        reconcile_update_store_with_current_version(&mut store);
        assert_eq!(store.staged_version, None);
        assert_eq!(store.staged_asset_name, None);
        assert_eq!(store.staged_asset_path, None);
        assert!(!store.pending_apply_on_exit);
        assert_eq!(store.status, "up_to_date");
        assert_eq!(store.latest_version.as_deref(), Some(CURRENT_VERSION));
    }

    #[test]
    fn reconcile_update_store_clears_stale_handoff_state_after_successful_relaunch() {
        let mut store = AppUpdateStore {
            latest_version: Some("1.0.6-1".to_string()),
            staged_asset_name: Some("cerbena-browser-1.2.3.msi".to_string()),
            staged_asset_path: Some("C:/tmp/update.msi".to_string()),
            selected_asset_type: Some("msi".to_string()),
            selected_asset_reason: Some("windows_installed_context_prefers_msi".to_string()),
            install_handoff_mode: Some("direct_msi".to_string()),
            updater_handoff_version: Some(CURRENT_VERSION.to_string()),
            status: "applied_pending_relaunch".to_string(),
            ..AppUpdateStore::default()
        };
        reconcile_update_store_with_current_version(&mut store);
        assert_eq!(store.staged_asset_name, None);
        assert_eq!(store.staged_asset_path, None);
        assert_eq!(store.selected_asset_type, None);
        assert_eq!(store.selected_asset_reason, None);
        assert_eq!(store.install_handoff_mode, None);
        assert_eq!(store.updater_handoff_version, None);
        assert_eq!(store.status, "up_to_date");
    }

    #[test]
    fn relaunch_executable_prefers_cerbena_binary() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cerbena = temp.path().join("cerbena.exe");
        std::fs::write(&cerbena, b"stub").expect("write cerbena");
        let legacy = temp.path().join("browser-desktop-ui.exe");
        std::fs::write(&legacy, b"stub").expect("write legacy");
        assert_eq!(
            resolve_relaunch_executable_path(temp.path()),
            Some(PathBuf::from(cerbena))
        );
    }

    #[test]
    fn zip_apply_helper_stops_existing_launcher_processes_before_copy() {
        let script = build_zip_apply_helper_script(
            4242,
            Path::new("C:/tmp/update.zip"),
            Path::new("C:/Program Files/Cerbena Browser"),
            Some(Path::new("C:/Program Files/Cerbena Browser/cerbena.exe")),
            Some("C:/tmp/runtime_logs.log"),
        );
        assert!(script.contains("Stop-Process -Id $proc.Id -Force"));
        assert!(script.contains("WaitForExit(15000)"));
        assert!(script.contains("@('cerbena.exe','browser-desktop-ui.exe','cerbena-updater.exe')"));
        assert!(script
            .contains("for ($attempt=0; $attempt -lt 10 -and -not $copySucceeded; $attempt++)"));
        assert!(script.contains("CERBENA_UPDATER_AUTO_EXIT_AFTER_SECONDS"));
        assert!(script.contains("[updater-helper][zip]"));
        assert!(script.contains("CERBENA_UPDATER_RUNTIME_LOG"));
    }

    #[test]
    fn msi_apply_helper_uses_quiet_msiexec_and_updates_store() {
        let script = build_msi_apply_helper_script(
            4242,
            Path::new("C:/tmp/update.msi"),
            Some(Path::new("C:/Users/test/AppData/Local/Cerbena Browser")),
            Some("C:/tmp/app_update_store.json"),
            Some("1.2.3"),
            Some("C:/tmp/runtime_logs.log"),
        );
        assert!(script.contains("Start-Process -FilePath 'msiexec.exe'"));
        assert!(script.contains("'/qn'"));
        assert!(script.contains("'/l*v'"));
        assert!(script.contains("@('cerbena.exe','browser-desktop-ui.exe','cerbena-updater.exe','cerbena-launcher.exe')"));
        assert!(script.contains("Stop-Process -Id $proc.Id -Force"));
        assert!(script.contains("WaitForExit(15000)"));
        assert!(script.contains("while (-not $proc.WaitForExit(1000))"));
        assert!(script.contains("$elapsedMs -ge $msiWaitTimeoutMs"));
        assert!(script.contains("msiexec timed out"));
        assert!(script.contains("taskkill.exe"));
        assert!(script.contains("Resolve-RelaunchExecutable"));
        assert!(script.contains("INSTALLDIR="));
        assert!(script.contains("Update-Store 'applied_pending_relaunch'"));
        assert!(script.contains("pendingApplyOnExit"));
        assert!(script.contains("Update-Store 'canceled'"));
        assert!(script.contains("1602"));
        assert!(script.contains("1618"));
        assert!(script.contains("another Windows Installer transaction is already running (1618)"));
        assert!(script.contains("verbose log"));
        assert!(script.contains("[updater-helper][msi]"));
        assert!(script.contains("CERBENA_UPDATER_MSI_INSTALL_DIR"));
        assert!(script.contains("CERBENA_UPDATER_MSI_TIMEOUT_MS"));
        assert!(script.contains("CERBENA_UPDATER_RUNTIME_LOG"));
    }

    #[test]
    fn latest_release_api_url_prefers_env_override() {
        let key = RELEASE_LATEST_API_URL_ENV;
        let previous = std::env::var(key).ok();
        std::env::set_var(key, "http://127.0.0.1:9191/latest");
        assert_eq!(
            resolve_latest_release_api_url(),
            "http://127.0.0.1:9191/latest"
        );
        if let Some(value) = previous {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn updater_launch_mode_detects_preview_flag() {
        assert!(matches!(
            UpdaterLaunchMode::from_args(["--updater-preview"]),
            UpdaterLaunchMode::Preview
        ));
    }

    #[test]
    fn updater_launch_mode_detects_auto_flag() {
        assert!(matches!(
            UpdaterLaunchMode::from_args(["--updater"]),
            UpdaterLaunchMode::Auto
        ));
    }

    #[test]
    fn auto_launch_mode_triggers_close_after_ready_to_restart() {
        assert!(should_auto_close_updater_after_ready_to_restart(
            UpdaterLaunchMode::Auto
        ));
        assert!(!should_auto_close_updater_after_ready_to_restart(
            UpdaterLaunchMode::Preview
        ));
        assert!(!should_auto_close_updater_after_ready_to_restart(
            UpdaterLaunchMode::Disabled
        ));
    }

    #[test]
    fn trusted_updater_downloads_mocked_newer_release_asset() {
        let asset_name = "cerbena-windows-x64.zip";
        let asset_bytes = b"trusted-update-asset".to_vec();
        let checksum = sha256_hex(&asset_bytes);
        let next_version = next_release_version(CURRENT_VERSION);
        let checksums_text = format!("{checksum}  {asset_name}\n");
        let base = spawn_http_server(vec![(
            format!("/{asset_name}"),
            asset_bytes.clone(),
            "application/octet-stream",
            Vec::new(),
        )]);
        let release_payload = format!(
            r#"{{
                "tag_name":"v{version}",
                "html_url":"https://example.invalid/releases/v{version}",
                "assets":[
                    {{"name":"checksums.txt","browser_download_url":"{base}/checksums.txt"}},
                    {{"name":"checksums.sig","browser_download_url":"{base}/checksums.sig"}},
                    {{"name":"{asset_name}","browser_download_url":"{base}/{asset_name}"}}
                ]
            }}"#,
            version = next_version,
            asset_name = asset_name,
            base = base
        );
        let api_base = spawn_http_server(vec![(
            "/latest".to_string(),
            release_payload.into_bytes(),
            "application/json",
            Vec::new(),
        )]);
        let client = build_release_http_client(Duration::from_secs(5), false)
            .expect("build discovery client");
        let candidate = fetch_latest_release_from_url(&client, &format!("{api_base}/latest"))
            .expect("discover mocked release");
        assert!(is_version_newer(&candidate.version, CURRENT_VERSION));
        assert_eq!(candidate.asset_name.as_deref(), Some(asset_name));
        let download_client =
            build_release_http_client(Duration::from_secs(5), true).expect("build download client");
        let downloaded = download_release_bytes(
            &download_client,
            candidate.asset_url.as_deref().expect("asset url"),
            "release asset",
        )
        .expect("download mocked asset");
        let security_bundle = VerifiedReleaseSecurityBundle { checksums_text };
        ensure_asset_matches_verified_checksum(&security_bundle, asset_name, &downloaded)
            .expect("verify checksum");
        assert_eq!(downloaded, asset_bytes);
    }

    #[test]
    fn trusted_updater_download_tolerates_bad_content_encoding_headers() {
        let payload = b"plain-binary-payload".to_vec();
        let base = spawn_http_server(vec![(
            "/asset.zip".to_string(),
            payload.clone(),
            "application/octet-stream",
            vec![("Content-Encoding", "gzip")],
        )]);
        let client = build_release_http_client(Duration::from_secs(5), true)
            .expect("build raw download client");
        let downloaded =
            download_release_bytes(&client, &format!("{base}/asset.zip"), "release asset")
                .expect("download payload with broken content encoding header");
        assert_eq!(downloaded, payload);
    }
}
