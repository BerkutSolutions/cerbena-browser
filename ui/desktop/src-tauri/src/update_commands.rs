use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::{
    envelope::{ok, UiEnvelope},
    state::{persist_app_update_store, AppState},
};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPOSITORY_URL: &str = "https://github.com/BerkutSolutions/cerbena-browser";
const GITHUB_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/BerkutSolutions/cerbena-browser/releases/latest";
const UPDATE_CHECK_INTERVAL_MS: u128 = 6 * 60 * 60 * 1000;
const SCHEDULER_TICK: Duration = Duration::from_secs(15 * 60);
const USER_AGENT: &str = concat!("Cerbena-Updater/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateStore {
    #[serde(default)]
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
}

#[tauri::command]
pub fn get_launcher_update_state(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    let store = state
        .app_update_store
        .lock()
        .map_err(|e| format!("lock app update store: {e}"))?
        .clone();
    Ok(ok(correlation_id, to_view(&store)))
}

#[tauri::command]
pub fn set_launcher_auto_update(
    state: State<AppState>,
    enabled: bool,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    let mut store = state
        .app_update_store
        .lock()
        .map_err(|e| format!("lock app update store: {e}"))?;
    store.auto_update_enabled = enabled;
    if store.status.trim().is_empty() {
        store.status = "idle".to_string();
    }
    persist_update_store_from_state(&state, &store)?;
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
        let state = app.state::<AppState>();
        let should_run = match state.app_update_store.lock() {
            Ok(store) => {
                store.auto_update_enabled
                    && store
                        .last_checked_epoch_ms
                        .map(|value| now_epoch_ms().saturating_sub(value) >= UPDATE_CHECK_INTERVAL_MS)
                        .unwrap_or(true)
            }
            Err(_) => false,
        };
        if should_run {
            let _ = run_update_cycle(&state, false);
        }
        thread::sleep(SCHEDULER_TICK);
    });
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

    let launched = match extension.as_str() {
        "zip" => launch_zip_apply_helper(current_pid, &asset_path, &install_root).is_ok(),
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

fn run_update_cycle(state: &AppState, _manual: bool) -> Result<AppUpdateView, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("build update http client: {e}"))?;

    let result = fetch_latest_release(&client);
    let mut store = state
        .app_update_store
        .lock()
        .map_err(|e| format!("lock app update store: {e}"))?
        .clone();

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
                if store.auto_update_enabled {
                    stage_release_if_needed(state, &mut store, &candidate)?;
                }
            } else {
                clear_staged_update(&mut store);
                store.status = "up_to_date".to_string();
            }
        }
        Err(error) => {
            store.status = "error".to_string();
            store.last_error = Some(error);
        }
    }

    persist_update_store_from_state(state, &store)?;
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
        && store.staged_asset_path.as_deref().map(Path::new).is_some_and(Path::is_file)
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

    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("build update download client: {e}"))?;
    let response = client
        .get(asset_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| format!("download update asset: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "download update asset failed with HTTP {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .map_err(|e| format!("read update asset body: {e}"))?;
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

fn fetch_latest_release(client: &Client) -> Result<ReleaseCandidate, String> {
    let response = client
        .get(GITHUB_LATEST_RELEASE_API)
        .header("User-Agent", USER_AGENT)
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
    Ok(ReleaseCandidate {
        version,
        release_url: release.html_url,
        asset_name: asset.map(|item| item.name.clone()),
        asset_url: asset.map(|item| item.browser_download_url.clone()),
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
) -> Result<(), String> {
    let helper = format!(
        "$pidValue={pid};\
        $archive={archive};\
        $installRoot={install};\
        while (Get-Process -Id $pidValue -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 500 }};\
        $temp=Join-Path ([System.IO.Path]::GetTempPath()) ('cerbena-update-' + [guid]::NewGuid().ToString('N'));\
        New-Item -ItemType Directory -Path $temp -Force | Out-Null;\
        Expand-Archive -LiteralPath $archive -DestinationPath $temp -Force;\
        $source=$temp;\
        $entries=Get-ChildItem -LiteralPath $temp;\
        if ($entries.Count -eq 1 -and $entries[0].PSIsContainer) {{ $source=$entries[0].FullName }};\
        Get-ChildItem -LiteralPath $source | ForEach-Object {{ Copy-Item -LiteralPath $_.FullName -Destination $installRoot -Recurse -Force }}",
        pid = pid,
        archive = powershell_quote(archive_path),
        install = powershell_quote(install_root)
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

fn launch_msi_installer(msi_path: &Path) -> Result<(), String> {
    Command::new("msiexec.exe")
        .args(["/i", &msi_path.to_string_lossy(), "/qn", "/norestart"])
        .spawn()
        .map_err(|e| format!("spawn msi installer: {e}"))?;
    Ok(())
}

fn persist_update_store_from_state(state: &AppState, store: &AppUpdateStore) -> Result<(), String> {
    let path = state
        .app_update_store_path(&state.app_handle)
        .map_err(|e| format!("resolve app update store path: {e}"))?;
    persist_app_update_store(&path, store)
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
    normalize_version(value)
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        })
        .collect()
}

fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::{
        asset_rank, can_auto_apply_asset, is_version_newer, normalize_version, pick_release_asset,
        GithubReleaseAsset,
    };

    #[test]
    fn version_normalization_drops_leading_v() {
        assert_eq!(normalize_version("v1.2.3"), "1.2.3");
        assert_eq!(normalize_version("1.2.3"), "1.2.3");
    }

    #[test]
    fn newer_version_detection_uses_semver_like_order() {
        assert!(is_version_newer("1.2.4", "1.2.3"));
        assert!(is_version_newer("2.0.0", "1.9.9"));
        assert!(!is_version_newer("1.2.3", "1.2.3"));
        assert!(!is_version_newer("1.2.2", "1.2.3"));
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
}
