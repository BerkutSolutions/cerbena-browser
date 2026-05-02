use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use tar::Archive;
use tauri::{AppHandle, Emitter, Manager};
use zip::ZipArchive;

use crate::state::AppState;

fn hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Cerbena/",
    env!("CARGO_PKG_VERSION"),
    " NetworkRuntime"
);

const SING_BOX_VERSION: &str = "1.12.0";
const SING_BOX_FILE: &str = "sing-box-1.12.0-windows-amd64.zip";
const SING_BOX_SHA256: &str = "49a5b90b390974a87b4660308446dfd9630f60ac655f76383abbd5f0994b09b3";
const SING_BOX_URL: &str =
    "https://github.com/SagerNet/sing-box/releases/download/v1.12.0/sing-box-1.12.0-windows-amd64.zip";

const OPENVPN_VERSION: &str = "2.6.16-I001";
const OPENVPN_FILE: &str = "OpenVPN-2.6.16-I001-amd64.msi";
const OPENVPN_SHA512: &str = "efc47168f37347b3de01869c71f58b3945dfae456332175f84297dffb5840f855d8cfdc4257c7bd921523e95cc5706d15d1662e547a947504ec12420eb6f2656";
const OPENVPN_URL: &str =
    "https://swupdate.openvpn.org/community/releases/OpenVPN-2.6.16-I001-amd64.msi";

const AMNEZIAWG_VERSION: &str = "2.0.0";
const AMNEZIAWG_FILE: &str = "amneziawg-amd64-2.0.0.msi";
const AMNEZIAWG_SHA256: &str = "8a6b4eb62a0bb8663ee50ba4253f5221da87f5b750640ddcf42f414dbef79933";
const AMNEZIAWG_URL: &str =
    "https://github.com/amnezia-vpn/amneziawg-windows-client/releases/download/2.0.0/amneziawg-amd64-2.0.0.msi";

const TOR_BUNDLE_VERSION: &str = "15.0.9";
const TOR_BUNDLE_FILE: &str = "tor-expert-bundle-windows-x86_64-15.0.9.tar.gz";
const TOR_BUNDLE_URL: &str = "https://archive.torproject.org/tor-package-archive/torbrowser/15.0.9/tor-expert-bundle-windows-x86_64-15.0.9.tar.gz";
const TOR_BUNDLE_SUMS_URL: &str = "https://archive.torproject.org/tor-package-archive/torbrowser/15.0.9/sha256sums-signed-build.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NetworkTool {
    SingBox,
    OpenVpn,
    AmneziaWg,
    TorBundle,
}

impl NetworkTool {
    pub fn as_key(self) -> &'static str {
        match self {
            NetworkTool::SingBox => "sing-box",
            NetworkTool::OpenVpn => "openvpn",
            NetworkTool::AmneziaWg => "amneziawg",
            NetworkTool::TorBundle => "tor-bundle",
        }
    }

    fn version(self) -> &'static str {
        match self {
            NetworkTool::SingBox => SING_BOX_VERSION,
            NetworkTool::OpenVpn => OPENVPN_VERSION,
            NetworkTool::AmneziaWg => AMNEZIAWG_VERSION,
            NetworkTool::TorBundle => TOR_BUNDLE_VERSION,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRuntimeProgress {
    pub tool: String,
    pub version: String,
    pub stage: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percentage: f64,
    pub speed_bytes_per_sec: f64,
    pub message: Option<String>,
}

impl NetworkRuntimeProgress {
    fn stage(tool: NetworkTool, stage: &str, message: Option<String>) -> Self {
        Self {
            tool: tool.as_key().to_string(),
            version: tool.version().to_string(),
            stage: stage.to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 0.0,
            speed_bytes_per_sec: 0.0,
            message,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkInstallPaths {
    pub primary: PathBuf,
    pub extras: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkInstallation {
    tool: String,
    version: String,
    primary_path: PathBuf,
    extras: BTreeMap<String, PathBuf>,
    installed_at_epoch_ms: u128,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct NetworkRegistryPayload {
    tools: BTreeMap<String, NetworkInstallation>,
}

#[derive(Debug, Clone)]
struct NetworkRegistry {
    path: PathBuf,
}

impl NetworkRegistry {
    fn new(path: PathBuf) -> Result<Self, String> {
        if !path.exists() {
            let payload = NetworkRegistryPayload::default();
            let text = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
            fs::write(&path, text).map_err(|e| format!("write network registry: {e}"))?;
        }
        Ok(Self { path })
    }

    fn read(&self) -> Result<NetworkRegistryPayload, String> {
        let text =
            fs::read_to_string(&self.path).map_err(|e| format!("read network registry: {e}"))?;
        serde_json::from_str::<NetworkRegistryPayload>(&text)
            .map_err(|e| format!("parse network registry: {e}"))
    }

    fn write(&self, payload: &NetworkRegistryPayload) -> Result<(), String> {
        let text = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
        fs::write(&self.path, text).map_err(|e| format!("write network registry: {e}"))
    }

    fn get(&self, tool: NetworkTool) -> Result<Option<NetworkInstallation>, String> {
        let payload = self.read()?;
        Ok(payload.tools.get(tool.as_key()).cloned())
    }

    fn put(&self, installation: NetworkInstallation) -> Result<(), String> {
        let mut payload = self.read()?;
        payload
            .tools
            .insert(installation.tool.clone(), installation);
        self.write(&payload)
    }
}

#[derive(Debug, Clone)]
pub struct NetworkRuntime {
    install_root: PathBuf,
    cache_dir: PathBuf,
    registry: NetworkRegistry,
}

impl NetworkRuntime {
    pub fn new(base_dir: PathBuf) -> Result<Self, String> {
        let install_root = base_dir.join("tools");
        let cache_dir = base_dir.join("cache");
        fs::create_dir_all(&install_root)
            .map_err(|e| format!("create network install root: {e}"))?;
        fs::create_dir_all(&cache_dir).map_err(|e| format!("create network cache dir: {e}"))?;
        let registry = NetworkRegistry::new(base_dir.join("installed-network-tools.json"))?;
        Ok(Self {
            install_root,
            cache_dir,
            registry,
        })
    }

    pub fn installed(&self, tool: NetworkTool) -> Result<Option<NetworkInstallPaths>, String> {
        let Some(existing) = self.registry.get(tool)? else {
            return Ok(None);
        };
        if existing.version != tool.version() {
            return Ok(None);
        }
        if !existing.primary_path.exists() {
            return Ok(None);
        }
        if existing.extras.values().any(|path| !path.exists()) {
            return Ok(None);
        }
        Ok(Some(NetworkInstallPaths {
            primary: existing.primary_path,
            extras: existing.extras,
        }))
    }

    pub fn ensure_ready<F>(
        &self,
        tool: NetworkTool,
        mut emit: F,
    ) -> Result<NetworkInstallPaths, String>
    where
        F: FnMut(NetworkRuntimeProgress),
    {
        if let Some(installed) = self.installed(tool)? {
            emit(NetworkRuntimeProgress::stage(
                tool,
                "completed",
                Some("Network runtime already ready".to_string()),
            ));
            return Ok(installed);
        }

        emit(NetworkRuntimeProgress::stage(
            tool,
            "pending",
            Some("Preparing network runtime artifact".to_string()),
        ));

        let installed = match tool {
            NetworkTool::SingBox => self.install_sing_box(&mut emit)?,
            NetworkTool::OpenVpn => self.install_openvpn(&mut emit)?,
            NetworkTool::AmneziaWg => self.install_amneziawg(&mut emit)?,
            NetworkTool::TorBundle => self.install_tor_bundle(&mut emit)?,
        };

        let record = NetworkInstallation {
            tool: tool.as_key().to_string(),
            version: tool.version().to_string(),
            primary_path: installed.primary.clone(),
            extras: installed.extras.clone(),
            installed_at_epoch_ms: now_epoch_ms(),
        };
        self.registry.put(record)?;
        emit(NetworkRuntimeProgress::stage(
            tool,
            "completed",
            Some("Network runtime is ready".to_string()),
        ));
        Ok(installed)
    }

    fn install_sing_box<F>(&self, emit: &mut F) -> Result<NetworkInstallPaths, String>
    where
        F: FnMut(NetworkRuntimeProgress),
    {
        emit(NetworkRuntimeProgress::stage(
            NetworkTool::SingBox,
            "downloading",
            Some("Downloading sing-box".to_string()),
        ));
        let archive = self.download_file(
            NetworkTool::SingBox,
            SING_BOX_URL,
            SING_BOX_FILE,
            "downloading",
            emit,
        )?;
        verify_sha256(&archive, SING_BOX_SHA256)?;

        let target_dir = self
            .install_root
            .join(NetworkTool::SingBox.as_key())
            .join(SING_BOX_VERSION);
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir).map_err(|e| format!("cleanup sing-box target: {e}"))?;
        }
        fs::create_dir_all(&target_dir).map_err(|e| format!("create sing-box target: {e}"))?;

        emit(NetworkRuntimeProgress::stage(
            NetworkTool::SingBox,
            "extracting",
            Some("Extracting sing-box".to_string()),
        ));
        unzip_archive(&archive, &target_dir)?;
        let binary = find_file_recursive(&target_dir, "sing-box.exe")
            .ok_or_else(|| "sing-box.exe was not found after extraction".to_string())?;
        Ok(NetworkInstallPaths {
            primary: binary,
            extras: BTreeMap::new(),
        })
    }

    fn install_openvpn<F>(&self, emit: &mut F) -> Result<NetworkInstallPaths, String>
    where
        F: FnMut(NetworkRuntimeProgress),
    {
        emit(NetworkRuntimeProgress::stage(
            NetworkTool::OpenVpn,
            "downloading",
            Some("Downloading OpenVPN".to_string()),
        ));
        let msi = self.download_file(
            NetworkTool::OpenVpn,
            OPENVPN_URL,
            OPENVPN_FILE,
            "downloading",
            emit,
        )?;
        verify_sha512(&msi, OPENVPN_SHA512)?;

        let target_dir = self
            .install_root
            .join(NetworkTool::OpenVpn.as_key())
            .join(OPENVPN_VERSION);
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir).map_err(|e| format!("cleanup openvpn target: {e}"))?;
        }
        fs::create_dir_all(&target_dir).map_err(|e| format!("create openvpn target: {e}"))?;

        emit(NetworkRuntimeProgress::stage(
            NetworkTool::OpenVpn,
            "extracting",
            Some("Extracting OpenVPN from MSI".to_string()),
        ));
        extract_msi(&msi, &target_dir)?;
        let binary = find_file_recursive(&target_dir, "openvpn.exe")
            .ok_or_else(|| "openvpn.exe was not found after MSI extraction".to_string())?;
        Ok(NetworkInstallPaths {
            primary: binary,
            extras: BTreeMap::new(),
        })
    }

    fn install_tor_bundle<F>(&self, emit: &mut F) -> Result<NetworkInstallPaths, String>
    where
        F: FnMut(NetworkRuntimeProgress),
    {
        emit(NetworkRuntimeProgress::stage(
            NetworkTool::TorBundle,
            "downloading",
            Some("Downloading Tor expert bundle".to_string()),
        ));
        let archive = self.download_file(
            NetworkTool::TorBundle,
            TOR_BUNDLE_URL,
            TOR_BUNDLE_FILE,
            "downloading",
            emit,
        )?;
        let expected_sha = self.fetch_tor_bundle_sha256(TOR_BUNDLE_FILE)?;
        verify_sha256(&archive, &expected_sha)?;

        let target_dir = self
            .install_root
            .join(NetworkTool::TorBundle.as_key())
            .join(TOR_BUNDLE_VERSION);
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir).map_err(|e| format!("cleanup tor target: {e}"))?;
        }
        fs::create_dir_all(&target_dir).map_err(|e| format!("create tor target: {e}"))?;

        emit(NetworkRuntimeProgress::stage(
            NetworkTool::TorBundle,
            "extracting",
            Some("Extracting Tor expert bundle".to_string()),
        ));
        untar_gz_archive(&archive, &target_dir)?;

        let tor_binary = find_file_recursive(&target_dir, "tor.exe")
            .ok_or_else(|| "tor.exe was not found in Tor bundle".to_string())?;
        let lyrebird = find_file_recursive(&target_dir, "lyrebird.exe");
        let snowflake = find_file_recursive(&target_dir, "snowflake-client.exe");

        let mut extras = BTreeMap::new();
        if let Some(path) = lyrebird {
            extras.insert("lyrebird".to_string(), path);
        }
        if let Some(path) = snowflake {
            extras.insert("snowflake-client".to_string(), path);
        }
        Ok(NetworkInstallPaths {
            primary: tor_binary,
            extras,
        })
    }

    fn install_amneziawg<F>(&self, emit: &mut F) -> Result<NetworkInstallPaths, String>
    where
        F: FnMut(NetworkRuntimeProgress),
    {
        emit(NetworkRuntimeProgress::stage(
            NetworkTool::AmneziaWg,
            "downloading",
            Some("Downloading AmneziaWG".to_string()),
        ));
        let msi = self.download_file(
            NetworkTool::AmneziaWg,
            AMNEZIAWG_URL,
            AMNEZIAWG_FILE,
            "downloading",
            emit,
        )?;
        verify_sha256(&msi, AMNEZIAWG_SHA256)?;

        let target_dir = self
            .install_root
            .join(NetworkTool::AmneziaWg.as_key())
            .join(AMNEZIAWG_VERSION);
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)
                .map_err(|e| format!("cleanup amneziawg target: {e}"))?;
        }
        fs::create_dir_all(&target_dir).map_err(|e| format!("create amneziawg target: {e}"))?;

        emit(NetworkRuntimeProgress::stage(
            NetworkTool::AmneziaWg,
            "extracting",
            Some("Extracting AmneziaWG from MSI".to_string()),
        ));
        extract_msi(&msi, &target_dir)?;
        let binary = find_file_recursive(&target_dir, "amneziawg.exe")
            .or_else(|| find_file_recursive(&target_dir, "wireguard.exe"))
            .ok_or_else(|| "amneziawg.exe was not found after MSI extraction".to_string())?;
        Ok(NetworkInstallPaths {
            primary: binary,
            extras: BTreeMap::new(),
        })
    }

    fn download_file<F>(
        &self,
        tool: NetworkTool,
        url: &str,
        file_name: &str,
        stage: &str,
        emit: &mut F,
    ) -> Result<PathBuf, String>
    where
        F: FnMut(NetworkRuntimeProgress),
    {
        let client = http_client()?;
        let mut response = client
            .get(url)
            .send()
            .map_err(|e| format!("download {}: {e}", tool.as_key()))?;
        if !response.status().is_success() {
            return Err(format!(
                "download {} failed with status {}",
                tool.as_key(),
                response.status()
            ));
        }
        let total = response.content_length();
        let target = self.cache_dir.join(file_name);
        let mut file = fs::File::create(&target).map_err(|e| format!("create cache file: {e}"))?;
        let mut downloaded: u64 = 0;
        let mut last_emit = Instant::now();
        let started = Instant::now();
        let mut chunk = [0u8; 64 * 1024];
        loop {
            let read = response
                .read(&mut chunk)
                .map_err(|e| format!("read download stream: {e}"))?;
            if read == 0 {
                break;
            }
            file.write_all(&chunk[..read])
                .map_err(|e| format!("write cache file: {e}"))?;
            downloaded += read as u64;
            if last_emit.elapsed() >= Duration::from_millis(250) {
                let elapsed = started.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 {
                    downloaded as f64 / elapsed
                } else {
                    0.0
                };
                emit(NetworkRuntimeProgress {
                    tool: tool.as_key().to_string(),
                    version: tool.version().to_string(),
                    stage: stage.to_string(),
                    downloaded_bytes: downloaded,
                    total_bytes: total,
                    percentage: percent(downloaded, total),
                    speed_bytes_per_sec: speed,
                    message: None,
                });
                last_emit = Instant::now();
            }
        }
        let elapsed = started.elapsed().as_secs_f64();
        emit(NetworkRuntimeProgress {
            tool: tool.as_key().to_string(),
            version: tool.version().to_string(),
            stage: stage.to_string(),
            downloaded_bytes: downloaded,
            total_bytes: total,
            percentage: percent(downloaded, total),
            speed_bytes_per_sec: if elapsed > 0.0 {
                downloaded as f64 / elapsed
            } else {
                0.0
            },
            message: None,
        });
        Ok(target)
    }

    fn fetch_tor_bundle_sha256(&self, archive_file: &str) -> Result<String, String> {
        let cache_file = self
            .cache_dir
            .join(format!("tor-sha256sums-{}.txt", TOR_BUNDLE_VERSION));
        let text = if cache_file.exists() {
            fs::read_to_string(&cache_file).map_err(|e| format!("read tor checksums cache: {e}"))?
        } else {
            let client = http_client()?;
            let body = client
                .get(TOR_BUNDLE_SUMS_URL)
                .send()
                .and_then(|response| response.error_for_status())
                .map_err(|e| format!("download tor checksums: {e}"))?
                .text()
                .map_err(|e| format!("read tor checksums body: {e}"))?;
            fs::write(&cache_file, body.as_bytes())
                .map_err(|e| format!("cache tor checksums: {e}"))?;
            body
        };
        extract_checksum_value(&text, archive_file)
            .ok_or_else(|| format!("tor checksum for {archive_file} not found"))
    }
}

pub fn resolve_sing_box_binary_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("BROWSER_SINGBOX_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone())?;
    if let Some(installed) = runtime.installed(NetworkTool::SingBox)? {
        return Ok(installed.primary);
    }
    if let Some(found) = find_path_binary(if cfg!(target_os = "windows") {
        &["sing-box.exe", "sing-box"]
    } else {
        &["sing-box"]
    }) {
        return Ok(found);
    }
    Err("sing-box binary is unavailable".to_string())
}

pub fn resolve_openvpn_binary_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("BROWSER_OPENVPN_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone())?;
    if let Some(installed) = runtime.installed(NetworkTool::OpenVpn)? {
        return Ok(installed.primary);
    }
    if let Some(found) = find_path_binary(if cfg!(target_os = "windows") {
        &["openvpn.exe", "openvpn"]
    } else {
        &["openvpn"]
    }) {
        return Ok(found);
    }
    Err("openvpn binary is unavailable".to_string())
}

pub fn resolve_amneziawg_binary_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("BROWSER_AMNEZIAWG_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone())?;
    if let Some(installed) = runtime.installed(NetworkTool::AmneziaWg)? {
        return Ok(installed.primary);
    }
    if let Some(found) = find_path_binary(if cfg!(target_os = "windows") {
        &["amneziawg.exe", "amneziawg", "wireguard.exe", "wireguard"]
    } else {
        &["amneziawg", "wireguard"]
    }) {
        return Ok(found);
    }
    Err("amneziawg binary is unavailable".to_string())
}

pub fn resolve_tor_binary_path(app_handle: &AppHandle) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("BROWSER_TOR_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone()).ok()?;
    if let Some(installed) = runtime.installed(NetworkTool::TorBundle).ok().flatten() {
        return Some(installed.primary);
    }
    find_path_binary(if cfg!(target_os = "windows") {
        &["tor.exe", "tor"]
    } else {
        &["tor"]
    })
}

pub fn resolve_tor_pt_binary_path(app_handle: &AppHandle, protocol: &str) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("BROWSER_TOR_PT_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone()).ok()?;
    if let Some(installed) = runtime.installed(NetworkTool::TorBundle).ok().flatten() {
        match protocol {
            "snowflake" => {
                if let Some(path) = installed.extras.get("snowflake-client").cloned() {
                    return Some(path);
                }
            }
            _ => {
                if let Some(path) = installed.extras.get("lyrebird").cloned() {
                    return Some(path);
                }
            }
        }
        if let Some(path) = installed.extras.values().next().cloned() {
            return Some(path);
        }
    }
    find_path_binary(if cfg!(target_os = "windows") {
        &[
            "lyrebird.exe",
            "snowflake-client.exe",
            "lyrebird",
            "snowflake-client",
        ]
    } else {
        &["lyrebird", "snowflake-client"]
    })
}

pub fn ensure_network_runtime_tools(
    app_handle: &AppHandle,
    required: &BTreeSet<NetworkTool>,
) -> Result<(), String> {
    if required.is_empty() {
        return Ok(());
    }
    let state = app_handle.state::<AppState>();
    let runtime = NetworkRuntime::new(state.network_runtime_root.clone())?;
    for tool in required {
        if tool_is_resolved_without_download(app_handle, *tool) {
            continue;
        }
        ensure_network_tool_with_lock(app_handle, &state, &runtime, *tool)?;
    }
    Ok(())
}

fn ensure_network_tool_with_lock(
    app_handle: &AppHandle,
    state: &AppState,
    runtime: &NetworkRuntime,
    tool: NetworkTool,
) -> Result<(), String> {
    let key = tool.as_key().to_string();
    loop {
        let started_here = {
            let mut active = state
                .active_network_downloads
                .lock()
                .map_err(|_| "network download lock poisoned".to_string())?;
            if active.contains(&key) {
                false
            } else {
                active.insert(key.clone());
                true
            }
        };

        if started_here {
            let handle = app_handle.clone();
            let ensure_result = runtime.ensure_ready(tool, |progress| {
                let _ = handle.emit("network-runtime-progress", progress);
            });
            if let Err(error) = &ensure_result {
                let _ = app_handle.emit(
                    "network-runtime-progress",
                    NetworkRuntimeProgress::stage(tool, "error", Some(error.to_string())),
                );
            }
            let mut active = state
                .active_network_downloads
                .lock()
                .map_err(|_| "network download lock poisoned".to_string())?;
            active.remove(&key);
            return ensure_result.map(|_| ());
        }

        if runtime.installed(tool)?.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn tool_is_resolved_without_download(app_handle: &AppHandle, tool: NetworkTool) -> bool {
    match tool {
        NetworkTool::SingBox => resolve_sing_box_binary_path(app_handle).is_ok(),
        NetworkTool::OpenVpn => resolve_openvpn_binary_path(app_handle).is_ok(),
        NetworkTool::AmneziaWg => resolve_amneziawg_binary_path(app_handle).is_ok(),
        NetworkTool::TorBundle => {
            resolve_tor_binary_path(app_handle).is_some()
                && resolve_tor_pt_binary_path(app_handle, "obfs4").is_some()
                && resolve_tor_pt_binary_path(app_handle, "snowflake").is_some()
        }
    }
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(600))
        .connect_timeout(Duration::from_secs(20))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("http client build: {e}"))
}

fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), String> {
    let actual = hash_file_sha256(path)?;
    if actual.eq_ignore_ascii_case(expected_hex.trim()) {
        return Ok(());
    }
    Err(format!(
        "sha256 mismatch for {}: expected {}, got {}",
        path.display(),
        expected_hex,
        actual
    ))
}

fn verify_sha512(path: &Path, expected_hex: &str) -> Result<(), String> {
    let actual = hash_file_sha512(path)?;
    if actual.eq_ignore_ascii_case(expected_hex.trim()) {
        return Ok(());
    }
    Err(format!(
        "sha512 mismatch for {}: expected {}, got {}",
        path.display(),
        expected_hex,
        actual
    ))
}

fn hash_file_sha256(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&chunk[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_file_sha512(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha512::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&chunk[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn unzip_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path)
        .map_err(|e| format!("open zip archive {}: {e}", archive_path.display()))?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("open zip archive: {e}"))?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|e| format!("zip entry {index}: {e}"))?;
        let Some(name) = entry.enclosed_name().map(|v| v.to_path_buf()) else {
            continue;
        };
        let out_path = target_dir.join(name);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("create zip dir {}: {e}", out_path.display()))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create zip file parent: {e}"))?;
        }
        let mut out = fs::File::create(&out_path)
            .map_err(|e| format!("create zip file {}: {e}", out_path.display()))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| format!("extract zip file {}: {e}", out_path.display()))?;
    }
    Ok(())
}

fn untar_gz_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path)
        .map_err(|e| format!("open tar.gz archive {}: {e}", archive_path.display()))?;
    let gz = GzDecoder::new(file);
    let mut archive = Archive::new(gz);
    for (index, entry_result) in archive
        .entries()
        .map_err(|e| format!("open tar.gz entries {}: {e}", archive_path.display()))?
        .enumerate()
    {
        let mut entry = entry_result.map_err(|e| {
            format!(
                "read tar.gz entry {index} in {}: {e}",
                archive_path.display()
            )
        })?;
        let relative = entry
            .path()
            .map_err(|e| format!("read tar path {index} in {}: {e}", archive_path.display()))?
            .into_owned();
        let out_path = safe_archive_join(target_dir, &relative).map_err(|e| {
            format!(
                "reject tar entry {} in {}: {e}",
                relative.display(),
                archive_path.display()
            )
        })?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(format!(
                "reject tar entry {} in {}: links are not allowed",
                relative.display(),
                archive_path.display()
            ));
        }
        if entry_type.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("create tar dir {}: {e}", out_path.display()))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create tar file parent {}: {e}", parent.display()))?;
        }
        entry
            .unpack(&out_path)
            .map_err(|e| format!("extract tar file {}: {e}", out_path.display()))?;
    }
    Ok(())
}

fn safe_archive_join(target_dir: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                return Err("path traversal is not allowed".to_string())
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err("absolute archive paths are not allowed".to_string())
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err("empty archive path".to_string());
    }
    Ok(target_dir.join(normalized))
}

fn extract_msi(msi_path: &Path, target_dir: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let msi = escape_powershell_single_quoted(&msi_path.to_string_lossy());
        let target = escape_powershell_single_quoted(&target_dir.to_string_lossy());
        let script = format!(
            "$p = Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/a', '{msi}', 'TARGETDIR={target}', '/quiet', '/norestart') -WindowStyle Hidden -PassThru -Wait; exit $p.ExitCode"
        );
        let mut command = hidden_command("powershell.exe");
        command
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(script);
        let output = command
            .output()
            .map_err(|e| format!("start hidden msiexec administrative extract: {e}"))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(format!(
            "msiexec administrative extract failed (code {:?}){}{}",
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
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = msi_path;
        let _ = target_dir;
        Err("MSI extraction is supported only on Windows".to_string())
    }
}

fn extract_checksum_value(checksum_file: &str, file_name: &str) -> Option<String> {
    for line in checksum_file.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !trimmed.contains(file_name) {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let hash = parts.next()?;
        let entry = parts.last()?;
        let normalized = entry.trim_start_matches('*');
        if normalized == file_name {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

fn find_file_recursive(root: &Path, file_name: &str) -> Option<PathBuf> {
    let mut queue = vec![root.to_path_buf()];
    while let Some(dir) = queue.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                queue.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if name.eq_ignore_ascii_case(file_name) {
                return Some(path);
            }
        }
    }
    None
}

fn can_spawn(binary: &str, probe_arg: &str) -> bool {
    hidden_command(binary)
        .arg(probe_arg)
        .output()
        .map(|output| {
            output.status.success() || !output.stdout.is_empty() || !output.stderr.is_empty()
        })
        .unwrap_or(false)
}

fn find_path_binary(candidates: &[&str]) -> Option<PathBuf> {
    for candidate in candidates {
        if can_spawn(candidate, "--help") || can_spawn(candidate, "version") {
            return Some(PathBuf::from(candidate));
        }
    }
    None
}

fn percent(downloaded: u64, total: Option<u64>) -> f64 {
    let Some(total) = total else {
        return 0.0;
    };
    if total == 0 {
        return 0.0;
    }
    ((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use tar::{Builder, Header};
    use tempfile::tempdir;

    #[test]
    fn parse_checksum_from_signed_file_line() {
        let sums = "\
abcdef0123456789  tor-expert-bundle-windows-x86_64-15.0.9.tar.gz\n\
1111111111111111  tor-browser-linux-x86_64-15.0.9.tar.xz\n";
        let actual = extract_checksum_value(sums, "tor-expert-bundle-windows-x86_64-15.0.9.tar.gz");
        assert_eq!(actual.as_deref(), Some("abcdef0123456789"));
    }

    #[test]
    fn parse_checksum_with_star_prefix() {
        let sums = "0123 *tor-expert-bundle-windows-x86_64-15.0.9.tar.gz\n";
        let actual = extract_checksum_value(sums, "tor-expert-bundle-windows-x86_64-15.0.9.tar.gz");
        assert_eq!(actual.as_deref(), Some("0123"));
    }

    #[test]
    fn safe_archive_join_rejects_parent_traversal_entries() {
        let target_dir = Path::new("C:/tmp/cerbena-test");
        let error = safe_archive_join(target_dir, Path::new("../escape.txt"))
            .expect_err("must reject traversal");
        assert!(error.contains("path traversal"));
    }

    #[test]
    fn tar_extraction_accepts_safe_entries() {
        let temp = tempdir().expect("tempdir");
        let archive_path = temp.path().join("safe.tar.gz");
        let target_dir = temp.path().join("target");
        fs::create_dir_all(&target_dir).expect("create target");

        let tar_file = fs::File::create(&archive_path).expect("create archive");
        let encoder = GzEncoder::new(tar_file, Compression::default());
        let mut builder = Builder::new(encoder);
        let payload = b"ok";
        let mut header = Header::new_gnu();
        header.set_size(payload.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "tor/tor.exe", &payload[..])
            .expect("append safe entry");
        let encoder = builder.into_inner().expect("finish tar builder");
        encoder.finish().expect("finish gzip encoder");

        untar_gz_archive(&archive_path, &target_dir).expect("safe archive extracts");
        let extracted = target_dir.join("tor").join("tor.exe");
        assert!(extracted.is_file());
        let bytes = fs::read(extracted).expect("read extracted file");
        assert_eq!(bytes, b"ok");
    }
}
