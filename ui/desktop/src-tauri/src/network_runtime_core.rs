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
#[path = "network_runtime_resolver.rs"]
mod resolver;
#[path = "network_runtime_install_helpers.rs"]
mod install_helpers;

fn hidden_command(program: &str) -> Command {
    #[cfg(target_os = "windows")]
    let mut command = Command::new(program);
    #[cfg(not(target_os = "windows"))]
    let command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

#[cfg(target_os = "windows")]
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


#[path = "network_runtime_core_ops.rs"]
mod ops;


pub fn resolve_sing_box_binary_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    resolver::resolve_sing_box_binary_path_impl(app_handle)
}

pub fn resolve_openvpn_binary_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    resolver::resolve_openvpn_binary_path_impl(app_handle)
}

pub fn resolve_amneziawg_binary_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    resolver::resolve_amneziawg_binary_path_impl(app_handle)
}

pub fn resolve_tor_binary_path(app_handle: &AppHandle) -> Option<PathBuf> {
    resolver::resolve_tor_binary_path_impl(app_handle)
}

pub fn resolve_tor_pt_binary_path(app_handle: &AppHandle, protocol: &str) -> Option<PathBuf> {
    resolver::resolve_tor_pt_binary_path_impl(app_handle, protocol)
}

pub fn ensure_network_runtime_tools(
    app_handle: &AppHandle,
    required: &BTreeSet<NetworkTool>,
) -> Result<(), String> {
    resolver::ensure_network_runtime_tools_impl(app_handle, required)
}

fn http_client() -> Result<Client, String> {
    install_helpers::http_client()
}

fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), String> {
    install_helpers::verify_sha256(path, expected_hex)
}

fn verify_sha512(path: &Path, expected_hex: &str) -> Result<(), String> {
    install_helpers::verify_sha512(path, expected_hex)
}

fn unzip_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    install_helpers::unzip_archive(archive_path, target_dir)
}

fn untar_gz_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    install_helpers::untar_gz_archive(archive_path, target_dir)
}

fn extract_msi(msi_path: &Path, target_dir: &Path) -> Result<(), String> {
    install_helpers::extract_msi(msi_path, target_dir)
}

fn extract_checksum_value(checksum_file: &str, file_name: &str) -> Option<String> {
    install_helpers::extract_checksum_value(checksum_file, file_name)
}

fn find_file_recursive(root: &Path, file_name: &str) -> Option<PathBuf> {
    install_helpers::find_file_recursive(root, file_name)
}

fn percent(downloaded: u64, total: Option<u64>) -> f64 {
    install_helpers::percent(downloaded, total)
}

fn now_epoch_ms() -> u128 {
    install_helpers::now_epoch_ms()
}


