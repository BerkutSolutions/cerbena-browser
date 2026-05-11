#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Instant,
};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use tar::Archive;
use xz2::read::XzDecoder;
use zip::ZipArchive;

use crate::{
    chromium::ChromiumAdapter,
    contract::{EngineAdapter, EngineError, EngineKind},
    firefox_esr::FirefoxEsrAdapter,
    librewolf::LibrewolfAdapter,
    progress::EngineDownloadProgress,
    registry::EngineRegistry,
    ungoogled_chromium::UngoogledChromiumAdapter,
};
#[path = "runtime_artifacts.rs"]
mod artifacts;
#[path = "runtime_binary.rs"]
mod runtime_binary;
#[path = "runtime_download.rs"]
mod runtime_download;
#[path = "runtime_install.rs"]
mod runtime_install;
#[path = "runtime_injection.rs"]
mod runtime_injection;
#[path = "runtime_launch_chromium.rs"]
mod runtime_launch_chromium;
#[path = "runtime_launch_firefox.rs"]
mod runtime_launch_firefox;
#[path = "runtime_launch_policy.rs"]
mod runtime_launch_policy;
#[path = "runtime_launch_dispatch.rs"]
mod runtime_launch_dispatch;
#[path = "runtime_compat.rs"]
mod runtime_compat;
#[path = "runtime_facade_helpers.rs"]
mod runtime_facade_helpers;
#[path = "runtime_resolution.rs"]
mod runtime_resolution;
#[path = "runtime_api.rs"]
mod runtime_api;

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/136.0.0.0 Safari/537.36";
const CHROMIUM_SNAPSHOTS_BASE_URL: &str =
    "https://storage.googleapis.com/chromium-browser-snapshots";
const LIBREWOLF_RELEASES_URL: &str =
    "https://api.github.com/repos/librewolf-community/browser/releases?per_page=20";
const LIBREWOLF_LINUX_MIRROR_RELEASES_URL: &str =
    "https://api.github.com/repos/librewolf-community/browser-linux/releases?per_page=20";
const LIBREWOLF_LINUX_INSTALLATION_URL: &str = "https://librewolf.net/installation/linux/";
const LIBREWOLF_WINDOWS_INSTALLATION_URL: &str = "https://librewolf.net/installation/windows/";
const FIREFOX_VERSIONS_URL: &str = "https://product-details.mozilla.org/1.0/firefox_versions.json";
const FIREFOX_RELEASES_BASE_URL: &str = "https://releases.mozilla.org/pub/firefox/releases";
const UNGOOGLED_CHROMIUM_WINDOWS_RELEASES_URL: &str =
    "https://api.github.com/repos/ungoogled-software/ungoogled-chromium-windows/releases?per_page=20";
const UNGOOGLED_CHROMIUM_MACOS_RELEASES_URL: &str =
    "https://api.github.com/repos/ungoogled-software/ungoogled-chromium-macos/releases?per_page=20";
const UNGOOGLED_CHROMIUM_LINUX_RELEASES_URL: &str =
    "https://api.github.com/repos/ungoogled-software/ungoogled-chromium/releases?per_page=20";
const CHROMIUM_POLICY_EXTENSION_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_ZERO_BYTES_TIMEOUT_SECS: u64 = 30;
const GITHUB_ZERO_BYTES_TIMEOUT_SECS: u64 = 180;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInstallation {
    pub engine: EngineKind,
    pub version: String,
    pub binary_path: PathBuf,
    pub installed_at_epoch_ms: u128,
}

#[derive(Debug, Clone)]
pub struct EngineRuntime {
    install_root: PathBuf,
    cache_dir: PathBuf,
    registry: EngineRegistry,
}

#[derive(Debug, Clone)]
struct ResolvedArtifact {
    engine: EngineKind,
    version: String,
    download_url: String,
    file_name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LockedAppConfig {
    start_url: String,
    #[serde(default)]
    allowed_hosts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct IdentityLaunchPolicy {
    mode: Option<IdentityLaunchMode>,
    core: IdentityLaunchCore,
    locale: IdentityLaunchLocale,
    window: IdentityLaunchWindow,
    screen: IdentityLaunchScreen,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum IdentityLaunchMode {
    Real,
    Auto,
    Manual,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct IdentityLaunchCore {
    user_agent: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct IdentityLaunchLocale {
    navigator_language: String,
    languages: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct IdentityLaunchWindow {
    outer_width: u32,
    outer_height: u32,
    screen_x: i32,
    screen_y: i32,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct IdentityLaunchScreen {
    width: u32,
    height: u32,
}



use runtime_facade_helpers::*;

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod runtime_tests;








