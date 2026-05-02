use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State};

use crate::{
    envelope::{ok, UiEnvelope},
    state::{app_local_data_root, persist_app_update_store, AppState},
};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPOSITORY_URL: &str = "https://github.com/BerkutSolutions/cerbena-browser";
const GITHUB_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/BerkutSolutions/cerbena-browser/releases/latest";
const RELEASE_CHECKSUMS_ASSET: &str = "checksums.txt";
const RELEASE_CHECKSUMS_SIGNATURE_ASSET: &str = "checksums.sig";
const RELEASE_CHECKSUMS_B64_ENV: &str = "CERBENA_RELEASE_CHECKSUMS_B64";
const RELEASE_CHECKSUMS_SIGNATURE_B64_ENV: &str = "CERBENA_RELEASE_CHECKSUMS_SIGNATURE_B64";
const UPDATE_CHECK_INTERVAL_MS: u128 = 6 * 60 * 60 * 1000;
const SCHEDULER_TICK: Duration = Duration::from_secs(15 * 60);
const USER_AGENT: &str = concat!("Cerbena-Updater/", env!("CARGO_PKG_VERSION"));
const UPDATER_EVENT_NAME: &str = "updater-progress";
const RELEASE_SIGNING_PUBLIC_KEY_XML: &str = r#"<RSAKeyValue><Modulus>sQ/dGNzpHEHiSUvpp8+h4axIghjUrkY9hHX3GNPwS9kGK6FCoc6+DuKSK/u5JwEKk/sjTks2m8ANgCm1ajaEPFE/BQjP1VsqQE3/MGbpRwWXIYUP6qKX2EhMQa5Fg0fywHV5uk7v3x6Q/Yfc4cWVLKNClqpq2hk8CX0NfUjqN1s5CNnNH1zgZPZ45ExXZQBlM5UUhdY/N4LKTFiYjpDMvoW4KSM4j9maUBmoNGVTnnRgfyWm6wM7LCoqSPpYhSb4yE+/HtaBGpePVy21B5Xi1nzPSYfShEdVkmeCJTcTj8gr1o8OcqKEs5V3yQa6MmUhNgYM/uC/lGeqiR+lwiLG4Q==</Modulus><Exponent>AQAB</Exponent></RSAKeyValue>"#;

const UPDATER_STEP_DISCOVER: &str = "discover";
const UPDATER_STEP_COMPARE: &str = "compare";
const UPDATER_STEP_SECURITY: &str = "security";
const UPDATER_STEP_DOWNLOAD: &str = "download";
const UPDATER_STEP_CHECKSUM: &str = "checksum";
const UPDATER_STEP_INSTALL: &str = "install";
const UPDATER_STEP_RELAUNCH: &str = "relaunch";

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
    checksums_url: Option<String>,
    checksums_signature_url: Option<String>,
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
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x00000010);
    }
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

pub fn launch_pending_update_on_exit(app: &AppHandle) {
    let state = app.state::<AppState>();

    let snapshot = match state.app_update_store.lock() {
        Ok(store) => store.clone(),
        Err(_) => return,
    };

    if !snapshot.pending_apply_on_exit {
        return;
    }

    let Some(path) = snapshot.staged_asset_path.as_ref() else {
        return;
    };
    let asset_path = PathBuf::from(path);
    if !asset_path.is_file() {
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
        Err(_) => return,
    };
    let install_root = match current_exe.parent() {
        Some(value) => value.to_path_buf(),
        None => return,
    };
    let relaunch_executable = resolve_relaunch_executable_path(&install_root);

    let launched = match extension.as_str() {
        "zip" => launch_zip_apply_helper(current_pid, &asset_path, &install_root, relaunch_executable.as_deref()).is_ok(),
        "msi" => launch_msi_installer(&asset_path).is_ok(),
        _ => false,
    };

    if !launched {
        return;
    }

    if let Ok(mut store) = state.app_update_store.lock() {
        store.pending_apply_on_exit = false;
        store.status = "applying".to_string();
        let _ = persist_update_store_from_state(&state, &store);
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
    progress_updater_step(
        state,
        UPDATER_STEP_DISCOVER,
        "running",
        "i18n:updater.detail.live_discover_running",
    )?;
    let client = build_release_http_client(Duration::from_secs(30), false)
        .map_err(|e| format!("build updater client: {e}"))?;
    let candidate = fetch_latest_release(&client)?;
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
    let _staged_path = stage_verified_release_asset(state, &candidate, &asset_bytes)?;
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
        return finalize_updater_success(
            state,
            "ready_to_restart",
            "updater.summary.ready_to_restart",
            "i18n:updater.detail.live_ready_to_restart",
            "updater.action.reboot",
        );
    }
    finalize_updater_success(
        state,
        "completed",
        "updater.summary.complete",
        "i18n:updater.detail.live_complete",
        "action.close",
    )
}

fn run_update_cycle(state: &AppState, manual: bool) -> Result<AppUpdateView, String> {
    let client = build_release_http_client(Duration::from_secs(20), false)
        .map_err(|e| format!("build update http client: {e}"))?;

    let result = fetch_latest_release(&client);
    let mut store = refresh_update_store_snapshot(state)?;

    store.last_checked_at = Some(now_iso());
    store.last_checked_epoch_ms = Some(now_epoch_ms());

    match result {
        Ok(candidate) => {
            store.latest_version = Some(candidate.version.clone());
            store.release_url = Some(candidate.release_url.clone());
            store.has_update = is_version_newer(&candidate.version, CURRENT_VERSION);
            store.last_error = None;
            if store.has_update {
                store.status = "available".to_string();
                if manual {
                    match spawn_updater_process(&state.app_handle, UpdaterLaunchMode::Auto) {
                        Ok(()) => {
                            store.updater_handoff_version = Some(candidate.version.clone());
                            store.status = "handoff".to_string();
                        }
                        Err(error) => {
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
                            }
                            Err(error) => {
                                store.status = "error".to_string();
                                store.last_error = Some(error);
                            }
                        }
                    } else {
                        if let Err(error) = stage_release_if_needed(state, &mut store, &candidate)
                        {
                            store.status = "error".to_string();
                            store.last_error = Some(error);
                        }
                    }
                }
            } else {
                clear_staged_update(&mut store);
                store.updater_handoff_version = None;
                store.status = "up_to_date".to_string();
            }
        }
        Err(error) => {
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
        store.status = if can_auto_apply_asset(asset_name) {
            "downloaded".to_string()
        } else {
            "available".to_string()
        };
        store.pending_apply_on_exit = can_auto_apply_asset(asset_name);
        return Ok(());
    }

    let root = state
        .app_update_root_path(&state.app_handle)
        .map_err(|e| format!("resolve update root path: {e}"))?;
    let target_dir = root.join(&candidate.version);
    fs::create_dir_all(&target_dir).map_err(|e| format!("create update dir: {e}"))?;
    let asset_path = target_dir.join(asset_name);

    let client = build_release_http_client(Duration::from_secs(60), true)
        .map_err(|e| format!("build update download client: {e}"))?;
    let bytes = download_release_bytes(&client, asset_url, "update asset")?;
    verify_release_candidate(candidate, &bytes)?;
    fs::write(&asset_path, &bytes).map_err(|e| format!("write update asset: {e}"))?;

    store.staged_version = Some(candidate.version.clone());
    store.staged_asset_name = Some(asset_name.clone());
    store.staged_asset_path = Some(asset_path.to_string_lossy().to_string());
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
    let root = state
        .app_update_root_path(&state.app_handle)
        .map_err(|e| format!("resolve update root path: {e}"))?;
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
    fetch_latest_release_from_url(client, GITHUB_LATEST_RELEASE_API)
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
        return Err(format!(
            "latest release request failed with HTTP {}",
            response.status()
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
        asset_name: asset.map(|item| item.name.clone()),
        asset_url: asset.map(|item| item.browser_download_url.clone()),
        checksums_url: checksums.map(|item| item.browser_download_url.clone()),
        checksums_signature_url: checksums_signature.map(|item| item.browser_download_url.clone()),
    })
}

fn pick_release_asset(assets: &[GithubReleaseAsset]) -> Option<&GithubReleaseAsset> {
    let os = std::env::consts::OS;
    let mut candidates = assets
        .iter()
        .filter(|asset| {
            let name = asset.name.to_ascii_lowercase();
            matches!(os, "windows" if name.contains("windows") || name.contains("win") || name.ends_with(".zip") || name.ends_with(".msi"))
                || matches!(os, "linux" if name.contains("linux") || name.ends_with(".tar.gz") || name.ends_with(".zip"))
                || matches!(os, "macos" if name.contains("mac") || name.contains("darwin") || name.ends_with(".zip"))
        })
        .collect::<Vec<_>>();

    candidates.sort_by_key(|asset| asset_rank(&asset.name));
    candidates.into_iter().next()
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
            last_error = format!("download {label} failed with HTTP {}", response.status());
            if response.status().is_server_error() && attempt < 3 {
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

fn verify_release_checksums_signature_variant(
    checksums_bytes: &[u8],
    signature_b64: &str,
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
    .replace("__PUBLIC_KEY_XML__", RELEASE_SIGNING_PUBLIC_KEY_XML)
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
    for candidate in [
        normalized_lf.clone(),
        normalized_lf.replace('\n', "\r\n"),
    ] {
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

fn asset_rank(name: &str) -> u8 {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        return 0;
    }
    if lower.ends_with(".msi") {
        return 1;
    }
    if lower.ends_with(".exe") {
        return 2;
    }
    if lower.ends_with(".tar.gz") {
        return 3;
    }
    10
}

fn can_auto_apply_asset(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".msi")
}

fn launch_zip_apply_helper(
    pid: u32,
    archive_path: &Path,
    install_root: &Path,
    relaunch_executable: Option<&Path>,
) -> Result<(), String> {
    let helper = build_zip_apply_helper_script(pid, archive_path, install_root, relaunch_executable);
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
) -> String {
    let relaunch = relaunch_executable
        .map(powershell_quote)
        .unwrap_or_else(|| "$null".to_string());
    format!(
        "$pidValue={pid};\
        $archive={archive};\
        $installRoot={install};\
        $relaunchExe={relaunch};\
        $targetExecutables=@('cerbena.exe','browser-desktop-ui.exe','cerbena-updater.exe');\
        while (Get-Process -Id $pidValue -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 250 }};\
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
        foreach ($proc in $runningTargets) {{\
            try {{\
                $proc.WaitForExit(15000) | Out-Null;\
            }} catch {{}}\
        }};\
        $temp=Join-Path ([System.IO.Path]::GetTempPath()) ('cerbena-update-' + [guid]::NewGuid().ToString('N'));\
        New-Item -ItemType Directory -Path $temp -Force | Out-Null;\
        try {{\
            Expand-Archive -LiteralPath $archive -DestinationPath $temp -Force;\
            $source=$temp;\
            $entries=Get-ChildItem -LiteralPath $temp;\
            if ($entries.Count -eq 1 -and $entries[0].PSIsContainer) {{ $source=$entries[0].FullName }};\
            $copySucceeded=$false;\
            for ($attempt=0; $attempt -lt 10 -and -not $copySucceeded; $attempt++) {{\
                try {{\
                    Get-ChildItem -LiteralPath $source | ForEach-Object {{ Copy-Item -LiteralPath $_.FullName -Destination $installRoot -Recurse -Force }};\
                    $copySucceeded=$true;\
                }} catch {{\
                    if ($attempt -ge 9) {{ throw }};\
                    Start-Sleep -Milliseconds 500;\
                }}\
            }};\
            if ($relaunchExe -and (Test-Path -LiteralPath $relaunchExe)) {{ Start-Process -FilePath $relaunchExe }}\
        }} finally {{\
            if (Test-Path -LiteralPath $temp) {{ Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue }}\
        }}",
        pid = pid,
        archive = powershell_quote(archive_path),
        install = powershell_quote(install_root),
        relaunch = relaunch
    )
}

fn launch_msi_installer(msi_path: &Path) -> Result<(), String> {
    Command::new("msiexec.exe")
        .args(["/i", &msi_path.to_string_lossy(), "/qn", "/norestart"])
        .spawn()
        .map_err(|e| format!("spawn msi installer: {e}"))?;
    Ok(())
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
    if staged_is_current_or_older {
        clear_staged_update(store);
        store.updater_handoff_version = None;
        if store.status == "applying" || store.status == "downloaded" {
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
        parsed.extend(suffix.split('.').map(|part| part.parse::<u64>().unwrap_or(0)));
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
        asset_rank, build_release_http_client, build_zip_apply_helper_script,
        can_auto_apply_asset, default_auto_update_enabled, download_release_bytes,
        ensure_asset_matches_verified_checksum, extract_checksum_for_asset,
        fetch_latest_release_from_url, is_version_newer, normalize_version, pick_release_asset,
        reconcile_update_store_with_current_version, resolve_relaunch_executable_path,
        sha256_hex, should_run_auto_update_check, signature_verification_variants,
        AppUpdateStore, GithubReleaseAsset, VerifiedReleaseSecurityBundle, CURRENT_VERSION,
        RELEASE_CHECKSUMS_B64_ENV, RELEASE_CHECKSUMS_SIGNATURE_B64_ENV, UpdaterLaunchMode,
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
        let mut parts = normalized
            .split('.')
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let last = parts
            .pop()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        parts.push((last + 1).to_string());
        parts.join(".")
    }

    fn spawn_http_server(routes: Vec<(String, Vec<u8>, &'static str, Vec<(&'static str, &'static str)>)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test http server");
        let addr = listener.local_addr().expect("local addr");
        thread::spawn(move || {
            for _ in 0..routes.len() {
                let (mut stream, _) = listener.accept().expect("accept test connection");
                let mut buffer = [0u8; 8192];
                let read = stream.read(&mut buffer).expect("read request");
                let request = String::from_utf8_lossy(&buffer[..read]);
                let first_line = request.lines().next().unwrap_or_default();
                let path = first_line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/");
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
    fn auto_apply_support_is_limited_to_safe_asset_types() {
        assert!(can_auto_apply_asset("cerbena-windows.zip"));
        assert!(can_auto_apply_asset("cerbena-windows.msi"));
        assert!(!can_auto_apply_asset("cerbena-windows.exe"));
    }

    #[test]
    fn preferred_asset_order_keeps_zip_before_other_formats() {
        assert!(asset_rank("a.zip") < asset_rank("a.msi"));
        assert!(asset_rank("a.msi") < asset_rank("a.exe"));
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
        let selected = pick_release_asset(&assets).expect("selected asset");
        if cfg!(target_os = "windows") {
            assert_eq!(selected.name, "cerbena-windows.zip");
        }
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
        );
        assert!(script.contains("Stop-Process -Id $proc.Id -Force"));
        assert!(script.contains("WaitForExit(15000)"));
        assert!(script.contains("@('cerbena.exe','browser-desktop-ui.exe','cerbena-updater.exe')"));
        assert!(script.contains("for ($attempt=0; $attempt -lt 10 -and -not $copySucceeded; $attempt++)"));
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
    fn trusted_updater_downloads_mocked_newer_release_asset() {
        let asset_name = "cerbena-windows-x64.zip";
        let asset_bytes = b"trusted-update-asset".to_vec();
        let checksum = sha256_hex(&asset_bytes);
        let next_version = next_release_version(CURRENT_VERSION);
        let checksums_text = format!("{checksum}  {asset_name}\n");
        let base = spawn_http_server(vec![
            (
                format!("/{asset_name}"),
                asset_bytes.clone(),
                "application/octet-stream",
                Vec::new(),
            ),
        ]);
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
        let api_base = spawn_http_server(vec![
            (
                "/latest".to_string(),
                release_payload.into_bytes(),
                "application/json",
                Vec::new(),
            ),
        ]);
        let client = build_release_http_client(Duration::from_secs(5), false)
            .expect("build discovery client");
        let candidate = fetch_latest_release_from_url(&client, &format!("{api_base}/latest"))
            .expect("discover mocked release");
        assert!(is_version_newer(&candidate.version, CURRENT_VERSION));
        assert_eq!(candidate.asset_name.as_deref(), Some(asset_name));
        let download_client = build_release_http_client(Duration::from_secs(5), true)
            .expect("build download client");
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
        let downloaded = download_release_bytes(&client, &format!("{base}/asset.zip"), "release asset")
            .expect("download payload with broken content encoding header");
        assert_eq!(downloaded, payload);
    }
}
