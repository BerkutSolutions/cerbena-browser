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
    librewolf::LibrewolfAdapter,
    contract::{EngineAdapter, EngineError, EngineKind},
    progress::EngineDownloadProgress,
    registry::EngineRegistry,
    chromium::ChromiumAdapter,
    ungoogled_chromium::UngoogledChromiumAdapter,
};

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/136.0.0.0 Safari/537.36";
const CHROMIUM_SNAPSHOTS_BASE_URL: &str =
    "https://storage.googleapis.com/chromium-browser-snapshots";
const LIBREWOLF_RELEASES_URL: &str =
    "https://api.github.com/repos/librewolf-community/browser-windows/releases?per_page=20";
const LIBREWOLF_WINDOWS_INSTALLATION_URL: &str = "https://librewolf.net/installation/windows/";
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

impl EngineRuntime {
    pub fn new(base_dir: PathBuf) -> Result<Self, EngineError> {
        let install_root = base_dir.join("engines");
        let cache_dir = base_dir.join("cache");
        fs::create_dir_all(&install_root)?;
        fs::create_dir_all(&cache_dir)?;
        let registry = EngineRegistry::new(base_dir.join("installed-engines.json"))?;
        Ok(Self {
            install_root,
            cache_dir,
            registry,
        })
    }

    pub fn installed(&self, engine: EngineKind) -> Result<Option<EngineInstallation>, EngineError> {
        self.registry
            .get(engine)?
            .map(|installation| self.normalize_installation(engine, installation))
            .transpose()
    }

    pub fn ensure_ready<F, C>(
        &self,
        engine: EngineKind,
        mut emit: F,
        should_cancel: C,
    ) -> Result<EngineInstallation, EngineError>
    where
        F: FnMut(EngineDownloadProgress),
        C: Fn() -> bool,
    {
        if should_cancel() {
            return Err(EngineError::Download(
                "download interrupted by user".to_string(),
            ));
        }
        if let Some(installed) = self.installed(engine)? {
            if installed.binary_path.exists() {
                return Ok(installed);
            }
        }

        emit(EngineDownloadProgress {
            message: Some("Resolving engine artifact".to_string()),
            ..EngineDownloadProgress::stage(engine, "pending", "resolving")
        });
        eprintln!(
            "[engine-runtime] resolving artifact for {}",
            engine.as_key()
        );

        let artifact = self.resolve_artifact(engine)?;
        emit(EngineDownloadProgress {
            version: artifact.version.clone(),
            message: Some("Resolved engine artifact".to_string()),
            ..EngineDownloadProgress::stage(engine, artifact.version.clone(), "resolved")
        });
        eprintln!(
            "[engine-runtime] resolved {} {} -> {}",
            artifact.engine.as_key(),
            artifact.version,
            artifact.download_url
        );

        let archive_path = self.download_artifact(&artifact, &mut emit, &should_cancel)?;
        emit(EngineDownloadProgress {
            version: artifact.version.clone(),
            message: Some("Installing engine".to_string()),
            ..EngineDownloadProgress::stage(engine, artifact.version.clone(), "extracting")
        });
        eprintln!(
            "[engine-runtime] installing {} {} from {}",
            artifact.engine.as_key(),
            artifact.version,
            archive_path.display()
        );

        let target_dir = self
            .install_root
            .join(engine.as_key())
            .join(artifact.version.clone());
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)?;
        }
        fs::create_dir_all(&target_dir)?;
        if should_cancel() {
            return Err(EngineError::Download(
                "download interrupted by user".to_string(),
            ));
        }
        self.install_archive(&archive_path, &target_dir)?;

        emit(EngineDownloadProgress {
            version: artifact.version.clone(),
            message: Some("Verifying engine installation".to_string()),
            ..EngineDownloadProgress::stage(engine, artifact.version.clone(), "verifying")
        });

        let binary_path = self.locate_binary(engine, &target_dir)?;
        let installation = EngineInstallation {
            engine,
            version: artifact.version.clone(),
            binary_path,
            installed_at_epoch_ms: now_epoch_ms(),
        };
        self.registry.put(installation.clone())?;

        emit(EngineDownloadProgress {
            version: artifact.version.clone(),
            percentage: 100.0,
            message: Some("Engine ready".to_string()),
            ..EngineDownloadProgress::stage(engine, artifact.version, "completed")
        });
        eprintln!(
            "[engine-runtime] ready {} {} at {}",
            installation.engine.as_key(),
            installation.version,
            installation.binary_path.display()
        );
        Ok(installation)
    }

    pub fn launch(
        &self,
        engine: EngineKind,
        profile_root: PathBuf,
        profile_id: uuid::Uuid,
        start_page: String,
        private_mode: bool,
        gateway_proxy_port: Option<u16>,
        runtime_hardening: bool,
    ) -> Result<u32, EngineError> {
        let installation = self
            .installed(engine)?
            .ok_or_else(|| EngineError::Launch("engine is not installed".to_string()))?;
        let binary_path = if matches!(engine, EngineKind::Librewolf) {
            prefer_librewolf_browser_binary(&installation.binary_path)
        } else {
            installation.binary_path
        };
        let request = crate::contract::LaunchRequest {
            profile_id,
            profile_root: profile_root.clone(),
            binary_path,
            args: launch_args(
                engine,
                &profile_root,
                &start_page,
                private_mode,
                gateway_proxy_port,
                runtime_hardening,
            )?,
            env: launch_environment(engine, &profile_root),
        };
        eprintln!(
            "[engine-runtime] launch {} profile={} binary={} args={:?}",
            engine.as_key(),
            profile_id,
            request.binary_path.display(),
            request.args
        );
        match engine {
            EngineKind::Chromium => self.chromium_adapter().launch(request),
            EngineKind::UngoogledChromium => self.ungoogled_chromium_adapter().launch(request),
            EngineKind::Librewolf => self.librewolf_adapter().launch(request),
        }
    }

    pub fn open_url_in_existing_profile(
        &self,
        engine: EngineKind,
        profile_root: PathBuf,
        url: String,
    ) -> Result<(), EngineError> {
        let installation = self
            .installed(engine)?
            .ok_or_else(|| EngineError::Launch("engine is not installed".to_string()))?;
        let binary_path = if matches!(engine, EngineKind::Librewolf) {
            prefer_librewolf_browser_binary(&installation.binary_path)
        } else {
            installation.binary_path
        };
        let args = reopen_args(engine, &profile_root, &url)?;
        eprintln!(
            "[engine-runtime] reopen {} binary={} args={:?}",
            engine.as_key(),
            binary_path.display(),
            args
        );
        let mut command = Command::new(binary_path);
        command.args(&args);
        #[cfg(target_os = "windows")]
        {
            command.creation_flags(0x08000000);
        }
        command
            .spawn()
            .map_err(|e| EngineError::Launch(format!("reopen existing profile failed: {e}")))?;
        Ok(())
    }

    fn resolve_artifact(&self, engine: EngineKind) -> Result<ResolvedArtifact, EngineError> {
        match engine {
            EngineKind::Chromium => self.resolve_chromium_artifact(),
            EngineKind::UngoogledChromium => self.resolve_ungoogled_chromium_artifact(),
            EngineKind::Librewolf => self.resolve_librewolf_artifact(),
        }
    }

    fn resolve_chromium_artifact(&self) -> Result<ResolvedArtifact, EngineError> {
        let client = http_client()?;
        let platform_dir = chromium_snapshot_platform_dir()?;
        let last_change_url = format!("{CHROMIUM_SNAPSHOTS_BASE_URL}/{platform_dir}/LAST_CHANGE");
        let response = client
            .get(&last_change_url)
            .send()
            .map_err(|e| EngineError::Download(e.to_string()))?;
        if !response.status().is_success() {
            return Err(EngineError::Download(format!(
                "chromium LAST_CHANGE request failed with HTTP {}",
                response.status()
            )));
        }
        let revision = response
            .text()
            .map_err(|e| EngineError::Download(e.to_string()))?
            .trim()
            .to_string();
        if revision.is_empty() {
            return Err(EngineError::Download(
                "chromium LAST_CHANGE returned an empty revision".to_string(),
            ));
        }
        let archive_name = chromium_snapshot_archive_name()?;
        let download_url =
            format!("{CHROMIUM_SNAPSHOTS_BASE_URL}/{platform_dir}/{revision}/{archive_name}");
        Ok(ResolvedArtifact {
            engine: EngineKind::Chromium,
            version: revision,
            file_name: archive_name.to_string(),
            download_url,
        })
    }

    fn resolve_librewolf_artifact(&self) -> Result<ResolvedArtifact, EngineError> {
        if cfg!(target_os = "windows") {
            return self.resolve_librewolf_windows_artifact();
        }

        let client = http_client()?;
        let response = client
            .get(LIBREWOLF_RELEASES_URL)
            .send()
            .map_err(|e| EngineError::Download(e.to_string()))?;
        if !response.status().is_success() {
            return Err(EngineError::Download(format!(
                "librewolf releases request failed with HTTP {}",
                response.status()
            )));
        }
        let releases: Vec<GithubRelease> = response
            .json()
            .map_err(|e| EngineError::Download(e.to_string()))?;
        let suffix = librewolf_asset_suffix()?;
        for release in releases {
            if let Some(asset) = release.assets.into_iter().find(|item| {
                let lower = item.name.to_lowercase();
                lower.contains("librewolf") && lower.ends_with(&suffix)
            }) {
                return Ok(ResolvedArtifact {
                    engine: EngineKind::Librewolf,
                    version: release.tag_name,
                    file_name: asset.name,
                    download_url: asset.browser_download_url,
                });
            }
        }
        Err(EngineError::Download(format!(
            "no compatible LibreWolf asset found for suffix {suffix}"
        )))
    }

    fn resolve_ungoogled_chromium_artifact(&self) -> Result<ResolvedArtifact, EngineError> {
        let client = http_client()?;
        let response = client
            .get(ungoogled_chromium_releases_url()?)
            .send()
            .map_err(|e| EngineError::Download(e.to_string()))?;
        if !response.status().is_success() {
            return Err(EngineError::Download(format!(
                "ungoogled-chromium releases request failed with HTTP {}",
                response.status()
            )));
        }
        let releases: Vec<GithubRelease> = response
            .json()
            .map_err(|e| EngineError::Download(e.to_string()))?;
        for release in releases {
            if let Some(asset) = select_ungoogled_chromium_asset(&release)? {
                return Ok(ResolvedArtifact {
                    engine: EngineKind::UngoogledChromium,
                    version: release.tag_name,
                    file_name: asset.name,
                    download_url: asset.browser_download_url,
                });
            }
        }
        let suffixes = ungoogled_chromium_asset_suffixes()?;
        Err(EngineError::Download(format!(
            "no compatible ungoogled-chromium asset found for suffixes {:?}",
            suffixes
        )))
    }

    fn resolve_librewolf_windows_artifact(&self) -> Result<ResolvedArtifact, EngineError> {
        let client = http_client()?;
        let response = client
            .get(LIBREWOLF_WINDOWS_INSTALLATION_URL)
            .send()
            .map_err(|e| EngineError::Download(e.to_string()))?;
        if !response.status().is_success() {
            return Err(EngineError::Download(format!(
                "librewolf windows releases page request failed with HTTP {}",
                response.status()
            )));
        }
        let body = response
            .text()
            .map_err(|e| EngineError::Download(e.to_string()))?;
        let asset_marker = librewolf_windows_portable_marker()?;
        let download_url = extract_librewolf_download_url(&body, &asset_marker).ok_or_else(|| {
            EngineError::Download(format!(
                "no compatible LibreWolf Windows download link found for marker {asset_marker}"
            ))
        })?;
        let file_name = file_name_from_url(&download_url, "librewolf")?;
        let version = parse_librewolf_version_from_file_name(&file_name).ok_or_else(|| {
            EngineError::Download(format!(
                "unable to derive LibreWolf version from file name {file_name}"
            ))
        })?;
        Ok(ResolvedArtifact {
            engine: EngineKind::Librewolf,
            version,
            file_name,
            download_url,
        })
    }

    fn download_artifact<F, C>(
        &self,
        artifact: &ResolvedArtifact,
        emit: &mut F,
        should_cancel: &C,
    ) -> Result<PathBuf, EngineError>
    where
        F: FnMut(EngineDownloadProgress),
        C: Fn() -> bool,
    {
        if cfg!(target_os = "windows") {
            match self.download_artifact_with_curl(artifact, emit, should_cancel) {
                Ok(path) => return Ok(path),
                Err(EngineError::Download(message)) if should_fallback_to_reqwest(&message) => {
                    eprintln!(
                        "[engine-runtime] curl download fallback {} {} reason={}",
                        artifact.engine.as_key(),
                        artifact.version,
                        message
                    );
                    return self.download_artifact_with_reqwest(artifact, emit, should_cancel);
                }
                Err(error) => return Err(error),
            }
        }

        self.download_artifact_with_reqwest(artifact, emit, should_cancel)
    }

    fn download_artifact_with_reqwest<F, C>(
        &self,
        artifact: &ResolvedArtifact,
        emit: &mut F,
        should_cancel: &C,
    ) -> Result<PathBuf, EngineError>
    where
        F: FnMut(EngineDownloadProgress),
        C: Fn() -> bool,
    {
        let client = http_client()?;
        fs::create_dir_all(&self.cache_dir)?;
        let target = self.cache_dir.join(&artifact.file_name);
        let host = host_from_url(&artifact.download_url);
        emit(EngineDownloadProgress {
            version: artifact.version.clone(),
            host: host.clone(),
            message: Some(format!(
                "Connecting to {}",
                host.clone()
                    .unwrap_or_else(|| artifact.download_url.clone())
            )),
            ..EngineDownloadProgress::stage(artifact.engine, artifact.version.clone(), "connecting")
        });
        eprintln!(
            "[engine-runtime] connecting {} {} -> {}",
            artifact.engine.as_key(),
            artifact.version,
            artifact.download_url
        );
        let mut response = client
            .get(&artifact.download_url)
            .send()
            .map_err(|e| EngineError::Download(e.to_string()))?;
        if !response.status().is_success() {
            return Err(EngineError::Download(format!(
                "download failed with HTTP {}",
                response.status()
            )));
        }

        let total = response.content_length();
        let mut file = fs::File::create(&target)?;
        let start = Instant::now();
        let mut last_emit = Instant::now();
        let mut downloaded = 0u64;
        let mut buffer = [0u8; 64 * 1024];

        emit(EngineDownloadProgress {
            host: host.clone(),
            total_bytes: total,
            message: Some("Downloading engine".to_string()),
            ..EngineDownloadProgress::stage(
                artifact.engine,
                artifact.version.clone(),
                "downloading",
            )
        });
        eprintln!(
            "[engine-runtime] download started {} {} host={} total_bytes={}",
            artifact.engine.as_key(),
            artifact.version,
            host.clone().unwrap_or_else(|| "unknown".to_string()),
            total
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );

        loop {
            if should_cancel() {
                let _ = fs::remove_file(&target);
                return Err(EngineError::Download(
                    "download interrupted by user".to_string(),
                ));
            }
            let read = response
                .read(&mut buffer)
                .map_err(|e| EngineError::Download(e.to_string()))?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])?;
            downloaded += read as u64;

            if last_emit.elapsed().as_millis() >= 120 {
                let progress = build_transfer_progress(
                    artifact.engine,
                    artifact.version.clone(),
                    host.clone(),
                    downloaded,
                    total,
                    start.elapsed().as_secs_f64(),
                );
                emit(progress);
                last_emit = Instant::now();
            }
        }
        file.flush()?;

        emit(build_transfer_progress(
            artifact.engine,
            artifact.version.clone(),
            host.clone(),
            downloaded,
            total,
            start.elapsed().as_secs_f64(),
        ));
        eprintln!(
            "[engine-runtime] download finished {} {} bytes={} file={}",
            artifact.engine.as_key(),
            artifact.version,
            downloaded,
            target.display()
        );
        Ok(target)
    }

    fn download_artifact_with_curl<F, C>(
        &self,
        artifact: &ResolvedArtifact,
        emit: &mut F,
        should_cancel: &C,
    ) -> Result<PathBuf, EngineError>
    where
        F: FnMut(EngineDownloadProgress),
        C: Fn() -> bool,
    {
        fs::create_dir_all(&self.cache_dir)?;
        let target = self.cache_dir.join(&artifact.file_name);
        let host = host_from_url(&artifact.download_url);
        let zero_bytes_timeout_secs = zero_bytes_timeout_secs(&host);
        let total = probe_content_length_with_curl(&artifact.download_url);

        emit(EngineDownloadProgress {
            version: artifact.version.clone(),
            host: host.clone(),
            message: Some(format!(
                "Connecting to {}",
                host.clone()
                    .unwrap_or_else(|| artifact.download_url.clone())
            )),
            ..EngineDownloadProgress::stage(artifact.engine, artifact.version.clone(), "connecting")
        });

        eprintln!(
            "[engine-runtime] spawning curl {} {} -> {}",
            artifact.engine.as_key(),
            artifact.version,
            artifact.download_url
        );

        let mut command = Command::new("curl.exe");
        command
            .args([
                "--location",
                "--fail",
                "--silent",
                "--show-error",
                "--connect-timeout",
                "15",
                "--max-time",
                "1800",
                "--retry",
                "3",
                "--retry-all-errors",
                "--user-agent",
                USER_AGENT,
                "--output",
            ])
            .arg(&target)
            .arg(&artifact.download_url)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(target_os = "windows")]
        {
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| EngineError::Download(format!("failed to start curl.exe: {e}")))?;

        let start = Instant::now();
        let mut last_emit = Instant::now();

        emit(EngineDownloadProgress {
            host: host.clone(),
            total_bytes: total,
            message: Some("Downloading engine".to_string()),
            ..EngineDownloadProgress::stage(
                artifact.engine,
                artifact.version.clone(),
                "downloading",
            )
        });

        loop {
            if should_cancel() {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&target);
                return Err(EngineError::Download(
                    "download interrupted by user".to_string(),
                ));
            }
            let downloaded = fs::metadata(&target).map(|meta| meta.len()).unwrap_or(0);
            if downloaded == 0 && start.elapsed().as_secs() >= zero_bytes_timeout_secs {
                let _ = child.kill();
                let _ = child.wait();
                let host_label = host
                    .clone()
                    .unwrap_or_else(|| artifact.download_url.clone());
                return Err(EngineError::Download(format!(
                    "no bytes received from {host_label} within {zero_bytes_timeout_secs} seconds"
                )));
            }

            if let Some(status) = child
                .try_wait()
                .map_err(|e| EngineError::Download(format!("curl wait failed: {e}")))?
            {
                let output = child
                    .wait_with_output()
                    .map_err(|e| EngineError::Download(format!("curl output failed: {e}")))?;
                emit(build_transfer_progress(
                    artifact.engine,
                    artifact.version.clone(),
                    host.clone(),
                    downloaded,
                    total,
                    start.elapsed().as_secs_f64(),
                ));

                if !status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let message = if stderr.is_empty() {
                        format!("curl download failed with exit code {:?}", status.code())
                    } else {
                        format!("curl download failed: {stderr}")
                    };
                    return Err(EngineError::Download(message));
                }

                eprintln!(
                    "[engine-runtime] curl finished {} {} bytes={} file={}",
                    artifact.engine.as_key(),
                    artifact.version,
                    downloaded,
                    target.display()
                );
                return Ok(target);
            }

            if last_emit.elapsed().as_millis() >= 200 {
                emit(build_transfer_progress(
                    artifact.engine,
                    artifact.version.clone(),
                    host.clone(),
                    downloaded,
                    total,
                    start.elapsed().as_secs_f64(),
                ));
                last_emit = Instant::now();
            }

            thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    fn install_archive(&self, archive_path: &Path, target_dir: &Path) -> Result<(), EngineError> {
        let lower = archive_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_lowercase();
        if lower.ends_with(".zip") {
            extract_zip(archive_path, target_dir)
        } else if lower.ends_with(".tar.xz") {
            extract_tar_xz(archive_path, target_dir)
        } else if lower.ends_with(".dmg") {
            Err(EngineError::Install(
                "DMG packages are not supported by this installer build".to_string(),
            ))
        } else {
            let file_name = archive_path
                .file_name()
                .ok_or_else(|| EngineError::Install("archive has no file name".to_string()))?;
            fs::copy(archive_path, target_dir.join(file_name))?;
            Ok(())
        }
    }

    fn locate_binary(&self, engine: EngineKind, root: &Path) -> Result<PathBuf, EngineError> {
        let candidates = match engine {
            EngineKind::Chromium => candidate_names(&[
                "chrome.exe",
                "chrome",
                "chromium-browser.exe",
                "chromium.exe",
                "chromium",
            ]),
            EngineKind::UngoogledChromium => candidate_names(&[
                "chrome.exe",
                "chrome",
                "ungoogled-chromium.exe",
                "ungoogled-chromium",
                "chromium-browser.exe",
                "chromium.exe",
                "chromium",
            ]),
            EngineKind::Librewolf => candidate_names(&[
                "librewolf.exe",
                "librewolf",
                "firefox.exe",
                "firefox",
            ]),
        };
        let binary = find_first_match(root, &candidates).ok_or_else(|| {
            EngineError::Install(format!(
                "unable to locate {} executable under {}",
                engine.as_key(),
                root.display()
            ))
        })?;
        Ok(binary)
    }

    fn chromium_adapter(&self) -> ChromiumAdapter {
        ChromiumAdapter {
            install_root: self.install_root.clone(),
            cache_dir: self.cache_dir.clone(),
        }
    }

    fn ungoogled_chromium_adapter(&self) -> UngoogledChromiumAdapter {
        UngoogledChromiumAdapter {
            install_root: self.install_root.clone(),
            cache_dir: self.cache_dir.clone(),
        }
    }

    fn librewolf_adapter(&self) -> LibrewolfAdapter {
        LibrewolfAdapter {
            install_root: self.install_root.clone(),
            cache_dir: self.cache_dir.clone(),
        }
    }

    fn normalize_installation(
        &self,
        engine: EngineKind,
        mut installation: EngineInstallation,
    ) -> Result<EngineInstallation, EngineError> {
        let normalized_binary_path = match engine {
            EngineKind::Chromium | EngineKind::UngoogledChromium => {
                prefer_chromium_vendor_binary(&installation.binary_path)
            }
            EngineKind::Librewolf => installation.binary_path.clone(),
        };
        if normalized_binary_path != installation.binary_path {
            installation.binary_path = normalized_binary_path;
            self.registry.put(installation.clone())?;
        }
        Ok(installation)
    }
}

fn http_client() -> Result<Client, EngineError> {
    Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(60))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| EngineError::Download(e.to_string()))
}

fn build_transfer_progress(
    engine: EngineKind,
    version: String,
    host: Option<String>,
    downloaded: u64,
    total: Option<u64>,
    elapsed_secs: f64,
) -> EngineDownloadProgress {
    let speed = if elapsed_secs > 0.0 {
        downloaded as f64 / elapsed_secs
    } else {
        0.0
    };
    let percentage = total
        .filter(|value| *value > 0)
        .map(|value| downloaded as f64 / value as f64 * 100.0)
        .unwrap_or(0.0);
    let eta = if speed > 0.0 {
        total.map(|value| ((value.saturating_sub(downloaded)) as f64 / speed).max(0.0))
    } else {
        None
    };
    EngineDownloadProgress {
        engine,
        version,
        stage: "downloading".to_string(),
        host,
        downloaded_bytes: downloaded,
        total_bytes: total,
        percentage,
        speed_bytes_per_sec: speed,
        eta_seconds: eta,
        message: Some("Downloading engine".to_string()),
    }
}

fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<(), EngineError> {
    let file = fs::File::open(archive_path)?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| EngineError::Install(format!("zip open failed: {e}")))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| EngineError::Install(format!("zip entry failed: {e}")))?;
        let out_path = match entry.enclosed_name() {
            Some(path) => target_dir.join(path),
            None => continue,
        };
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

fn extract_tar_xz(archive_path: &Path, target_dir: &Path) -> Result<(), EngineError> {
    let file = fs::File::open(archive_path)?;
    let decoder = XzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(target_dir)
        .map_err(|e| EngineError::Install(format!("tar.xz unpack failed: {e}")))?;
    Ok(())
}

fn chromium_snapshot_platform_dir() -> Result<&'static str, EngineError> {
    if cfg!(target_os = "windows") {
        if cfg!(target_arch = "x86_64") {
            return Ok("Win_x64");
        }
        return Err(EngineError::Install(
            "unsupported Chromium Windows architecture".to_string(),
        ));
    }
    if cfg!(target_os = "linux") {
        if cfg!(target_arch = "x86_64") {
            return Ok("Linux_x64");
        }
        return Err(EngineError::Install(
            "unsupported Chromium Linux architecture".to_string(),
        ));
    }
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "x86_64") {
            return Ok("Mac");
        }
        if cfg!(target_arch = "aarch64") {
            return Ok("Mac_Arm");
        }
        return Err(EngineError::Install(
            "unsupported Chromium macOS architecture".to_string(),
        ));
    }
    Err(EngineError::Install(
        "unsupported Chromium operating system".to_string(),
    ))
}

fn chromium_snapshot_archive_name() -> Result<&'static str, EngineError> {
    if cfg!(target_os = "windows") {
        return Ok("chrome-win.zip");
    }
    if cfg!(target_os = "linux") {
        return Ok("chrome-linux.zip");
    }
    if cfg!(target_os = "macos") {
        return Ok("chrome-mac.zip");
    }
    Err(EngineError::Install(
        "unsupported Chromium archive format".to_string(),
    ))
}

fn librewolf_asset_suffix() -> Result<String, EngineError> {
    let (os, arch) = if cfg!(target_os = "windows") {
        (
            "win",
            if cfg!(target_arch = "x86_64") {
                "x86_64"
            } else {
                "arm64"
            },
        )
    } else if cfg!(target_os = "linux") {
        (
            "lin",
            if cfg!(target_arch = "x86_64") {
                "x86_64"
            } else {
                "arm64"
            },
        )
    } else if cfg!(target_os = "macos") {
        (
            "mac",
            if cfg!(target_arch = "x86_64") {
                "x86_64"
            } else {
                "arm64"
            },
        )
    } else {
        return Err(EngineError::Install(
            "unsupported operating system".to_string(),
        ));
    };
    Ok(format!("-{os}.{arch}.zip"))
}

fn librewolf_windows_portable_marker() -> Result<String, EngineError> {
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        return Err(EngineError::Install(
            "unsupported LibreWolf Windows architecture".to_string(),
        ));
    };
    Ok(format!("windows-{arch}-portable.zip"))
}

fn extract_librewolf_download_url(html: &str, asset_marker: &str) -> Option<String> {
    extract_html_hrefs(html)
        .into_iter()
        .find(|href| href.to_ascii_lowercase().contains(&asset_marker.to_ascii_lowercase()))
        .map(|href| normalize_href_url(&href))
}

fn extract_html_hrefs(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = html;
    while let Some(index) = rest.find("href=") {
        let after = &rest[index + 5..];
        let mut chars = after.chars();
        let quote = match chars.next() {
            Some('"') => '"',
            Some('\'') => '\'',
            _ => {
                rest = after;
                continue;
            }
        };
        let quoted = &after[1..];
        let Some(end) = quoted.find(quote) else {
            break;
        };
        let href = quoted[..end].trim();
        if !href.is_empty() {
            urls.push(href.to_string());
        }
        rest = &quoted[end + 1..];
    }
    urls
}

fn normalize_href_url(href: &str) -> String {
    if href.starts_with("https://") || href.starts_with("http://") {
        href.to_string()
    } else if href.starts_with("//") {
        format!("https:{href}")
    } else if href.starts_with('/') {
        format!("https://librewolf.net{href}")
    } else {
        format!("https://librewolf.net/{href}")
    }
}

fn parse_librewolf_version_from_file_name(file_name: &str) -> Option<String> {
    let prefix = "librewolf-";
    let middle = "-windows-";
    let stripped = file_name.strip_prefix(prefix)?;
    let end = stripped.find(middle)?;
    let version = stripped[..end].trim();
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

fn file_name_from_url(url: &str, fallback: &str) -> Result<String, EngineError> {
    let name = url
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            EngineError::Download(format!("unable to derive file name for {fallback}"))
        })?;
    Ok(name.to_string())
}

fn probe_content_length_with_curl(url: &str) -> Option<u64> {
    let mut command = Command::new("curl.exe");
    command
        .args([
            "--location",
            "--silent",
            "--show-error",
            "--head",
            "--user-agent",
            USER_AGENT,
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(0x08000000);
    }
    let output = command.output().ok()?;

    if !output.status.success() {
        return None;
    }

    let headers = String::from_utf8_lossy(&output.stdout);
    headers.lines().rev().find_map(|line| {
        let trimmed = line.trim();
        let (name, value) = trimmed.split_once(':')?;
        if !name.eq_ignore_ascii_case("content-length") {
            return None;
        }
        value.trim().parse::<u64>().ok()
    })
}

fn host_from_url(url: &str) -> Option<String> {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|value| value.to_string()))
}

fn zero_bytes_timeout_secs(host: &Option<String>) -> u64 {
    let Some(host) = host.as_deref() else {
        return DEFAULT_ZERO_BYTES_TIMEOUT_SECS;
    };
    let host = host.to_ascii_lowercase();
    if host == "github.com" || host.ends_with(".github.com") || host.ends_with("githubusercontent.com") {
        return GITHUB_ZERO_BYTES_TIMEOUT_SECS;
    }
    DEFAULT_ZERO_BYTES_TIMEOUT_SECS
}

fn should_fallback_to_reqwest(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("curl download failed: curl: (28) connection timed out")
        || normalized.contains("curl download failed: curl: (28) failed to connect")
        || normalized.contains("no bytes received from")
}

fn candidate_names(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn find_first_match(root: &Path, candidates: &[String]) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let file_name = path.file_name()?.to_string_lossy().to_lowercase();
            if candidates
                .iter()
                .any(|item| item.eq_ignore_ascii_case(&file_name))
            {
                return Some(path);
            }
        }
    }
    None
}

fn prefer_librewolf_browser_binary(current: &Path) -> PathBuf {
    let Some(root) = current.parent() else {
        return current.to_path_buf();
    };
    let librewolf_candidates = candidate_names(&["librewolf.exe", "librewolf"]);
    if let Some(path) = find_first_match(root, &librewolf_candidates) {
        return path;
    }
    let firefox_candidates = candidate_names(&["firefox.exe", "firefox"]);
    if let Some(path) = find_first_match(root, &firefox_candidates) {
        return path;
    }
    let private_candidates = candidate_names(&["private_browsing.exe", "private_browsing"]);
    if let Some(path) = find_first_match(root, &private_candidates) {
        return path;
    }
    current.to_path_buf()
}

fn prefer_chromium_vendor_binary(current: &Path) -> PathBuf {
    let Some(parent) = current.parent() else {
        return current.to_path_buf();
    };
    let current_name = current
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if current_name == "chrome.exe" || current_name == "chrome" {
        return current.to_path_buf();
    }
    let chrome_candidates = candidate_names(&["chrome.exe", "chrome"]);
    if let Some(path) = find_first_match(parent, &chrome_candidates) {
        return path;
    }
    current.to_path_buf()
}

fn launch_args(
    engine: EngineKind,
    profile_root: &Path,
    start_page: &str,
    private_mode: bool,
    gateway_proxy_port: Option<u16>,
    runtime_hardening: bool,
) -> Result<Vec<String>, EngineError> {
    let runtime_dir = profile_root.join("engine-profile");
    match engine {
        EngineKind::Chromium | EngineKind::UngoogledChromium => {
            let locked_app = load_locked_app_config(profile_root)?;
            let identity_policy = load_identity_launch_policy(profile_root);
            // Keep launch command size bounded on Windows. Huge domain blocklists can
            // overflow CreateProcess argument limits when passed via host resolver rules.
            const MAX_HOST_RESOLVER_RULES_LEN: usize = 8_192;
            let mut args = vec![
                format!("--user-data-dir={}", runtime_dir.to_string_lossy()),
                "--no-first-run".to_string(),
                "--no-default-browser-check".to_string(),
                "--disable-background-mode".to_string(),
                "--disable-quic".to_string(),
                "--disable-features=AsyncDns,DnsHttpssvc".to_string(),
            ];
            if private_mode {
                args.push("--incognito".to_string());
            }
            if runtime_hardening {
                args.push("--disable-sync".to_string());
                args.push("--disable-save-password-bubble".to_string());
            }
            if let Some(port) = gateway_proxy_port {
                args.push(format!("--proxy-server=http://127.0.0.1:{port}"));
                args.push("--proxy-bypass-list=".to_string());
            }
            if gateway_proxy_port.is_none() {
                if let Some(host_rules) =
                    chromium_host_resolver_rules(profile_root, MAX_HOST_RESOLVER_RULES_LEN)
                {
                    args.push(format!("--host-resolver-rules={host_rules}"));
                }
            }
            apply_chromium_identity_args(profile_root, identity_policy.as_ref(), &mut args)?;
            let extension_dirs = prepare_chromium_extension_dirs(profile_root)?;
            if !extension_dirs.is_empty() {
                let joined = extension_dirs
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                args.push(format!("--load-extension={joined}"));
            }
            if let Some(config) = locked_app {
                let app_url = resolve_locked_app_target_url(&config, start_page);
                args.push(format!("--app={app_url}"));
            } else {
                args.push(start_page.to_string());
            }
            Ok(args)
        }
        EngineKind::Librewolf => {
            let mut args = vec![
                "-profile".to_string(),
                runtime_dir.to_string_lossy().to_string(),
            ];
            let page = start_page.trim();
            if !page.is_empty() && !page.eq_ignore_ascii_case("about:blank") {
                args.push("-new-tab".to_string());
                args.push(page.to_string());
            }
            Ok(args)
        }
    }
}

fn reopen_args(
    engine: EngineKind,
    profile_root: &Path,
    url: &str,
) -> Result<Vec<String>, EngineError> {
    let runtime_dir = profile_root.join("engine-profile");
    Ok(match engine {
        EngineKind::Chromium | EngineKind::UngoogledChromium => {
            let locked_app = load_locked_app_config(profile_root)?;
            let mut args = vec![format!("--user-data-dir={}", runtime_dir.to_string_lossy())];
            if let Some(config) = locked_app {
                args.push(format!(
                    "--app={}",
                    resolve_locked_app_target_url(&config, url)
                ));
            } else {
                args.push(url.trim().to_string());
            }
            args
        }
        EngineKind::Librewolf => vec![
            "-profile".to_string(),
            runtime_dir.to_string_lossy().to_string(),
            "-new-tab".to_string(),
            url.trim().to_string(),
        ],
    })
}

fn load_identity_launch_policy(profile_root: &Path) -> Option<IdentityLaunchPolicy> {
    let path = profile_root.join("policy").join("identity-preset.json");
    let raw = fs::read(path).ok()?;
    serde_json::from_slice(&raw).ok()
}

fn apply_chromium_identity_args(
    profile_root: &Path,
    identity: Option<&IdentityLaunchPolicy>,
    args: &mut Vec<String>,
) -> Result<(), EngineError> {
    let Some(identity) = identity else {
        return Ok(());
    };
    if !identity.core.user_agent.trim().is_empty() && !identity_uses_native_user_agent(identity) {
        args.push(format!("--user-agent={}", identity.core.user_agent.trim()));
    }
    if let Some(language) = normalize_primary_language(&identity.locale.navigator_language) {
        args.push(format!("--lang={language}"));
    }
    let window_width = first_positive(identity.window.outer_width, identity.screen.width);
    let window_height = first_positive(identity.window.outer_height, identity.screen.height);
    if window_width > 0 && window_height > 0 {
        args.push(format!("--window-size={window_width},{window_height}"));
    }
    if identity.window.screen_x != 0 || identity.window.screen_y != 0 {
        args.push(format!(
            "--window-position={},{}",
            identity.window.screen_x, identity.window.screen_y
        ));
    }
    let languages = normalize_accept_languages(
        &identity.locale.navigator_language,
        &identity.locale.languages,
    );
    if !languages.is_empty() {
        args.push(format!("--accept-lang={}", languages.join(",")));
        write_chromium_language_preferences(profile_root, &languages)?;
        write_chromium_local_state_locale(profile_root, &languages)?;
    }
    Ok(())
}

fn launch_environment(engine: EngineKind, profile_root: &Path) -> Vec<(String, String)> {
    match engine {
        EngineKind::Chromium | EngineKind::UngoogledChromium => {
            chromium_launch_environment(profile_root)
        }
        EngineKind::Librewolf => Vec::new(),
    }
}

fn chromium_launch_environment(profile_root: &Path) -> Vec<(String, String)> {
    let Some(identity) = load_identity_launch_policy(profile_root) else {
        return Vec::new();
    };
    let languages = normalize_accept_languages(
        &identity.locale.navigator_language,
        &identity.locale.languages,
    );
    if languages.is_empty() {
        return Vec::new();
    }
    let primary = languages[0].clone();
    vec![
        ("LANG".to_string(), format!("{primary}.UTF-8")),
        ("LANGUAGE".to_string(), languages.join(":")),
        ("LC_ALL".to_string(), format!("{primary}.UTF-8")),
    ]
}

fn first_positive(primary: u32, fallback: u32) -> u32 {
    if primary > 0 {
        primary
    } else {
        fallback
    }
}

fn normalize_primary_language(language: &str) -> Option<String> {
    let trimmed = language.trim().replace('_', "-");
    (!trimmed.is_empty()).then_some(trimmed)
}

fn normalize_accept_languages(primary: &str, languages: &[String]) -> Vec<String> {
    let mut normalized = BTreeSet::new();
    let mut ordered = Vec::new();
    for candidate in std::iter::once(primary).chain(languages.iter().map(String::as_str)) {
        let value = candidate.trim().replace('_', "-");
        if value.is_empty() {
            continue;
        }
        let dedupe_key = value.to_ascii_lowercase();
        if normalized.insert(dedupe_key) {
            ordered.push(value);
        }
    }
    ordered
}

fn identity_uses_native_user_agent(identity: &IdentityLaunchPolicy) -> bool {
    matches!(identity.mode, Some(IdentityLaunchMode::Real))
}

fn build_accept_language_header(languages: &[String]) -> String {
    languages
        .iter()
        .enumerate()
        .map(|(index, language)| {
            if index == 0 {
                language.clone()
            } else {
                let quality = 1.0 - (index as f32 * 0.1);
                let quality = quality.max(0.1);
                format!("{language};q={quality:.1}")
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn write_chromium_language_preferences(
    profile_root: &Path,
    languages: &[String],
) -> Result<(), EngineError> {
    let preferences_path = profile_root
        .join("engine-profile")
        .join("Default")
        .join("Preferences");
    if let Some(parent) = preferences_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut value = if preferences_path.exists() {
        serde_json::from_slice::<serde_json::Value>(&fs::read(&preferences_path)?)?
    } else {
        serde_json::json!({})
    };
    if !value.is_object() {
        value = serde_json::json!({});
    }
    let root = value.as_object_mut().ok_or_else(|| {
        EngineError::Launch("chromium preferences root is not an object".to_string())
    })?;
    let intl = root
        .entry("intl".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !intl.is_object() {
        *intl = serde_json::json!({});
    }
    intl["accept_languages"] = serde_json::Value::String(languages.join(","));
    intl["selected_languages"] = serde_json::Value::String(languages.join(","));
    let bytes = serde_json::to_vec_pretty(&value)?;
    fs::write(preferences_path, bytes)?;
    Ok(())
}

fn write_chromium_local_state_locale(
    profile_root: &Path,
    languages: &[String],
) -> Result<(), EngineError> {
    let local_state_path = profile_root.join("engine-profile").join("Local State");
    if let Some(parent) = local_state_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut value = if local_state_path.exists() {
        serde_json::from_slice::<serde_json::Value>(&fs::read(&local_state_path)?)?
    } else {
        serde_json::json!({})
    };
    if !value.is_object() {
        value = serde_json::json!({});
    }
    let root = value.as_object_mut().ok_or_else(|| {
        EngineError::Launch("chromium local state root is not an object".to_string())
    })?;
    let intl = root
        .entry("intl".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !intl.is_object() {
        *intl = serde_json::json!({});
    }
    intl["app_locale"] = serde_json::Value::String(languages[0].clone());
    intl["selected_languages"] = serde_json::Value::String(languages.join(","));
    let bytes = serde_json::to_vec_pretty(&value)?;
    fs::write(local_state_path, bytes)?;
    Ok(())
}

fn chromium_host_resolver_rules(profile_root: &Path, max_len: usize) -> Option<String> {
    let path = profile_root.join("policy").join("blocked-domains.json");
    let raw = fs::read(path).ok()?;
    let domains: Vec<String> = serde_json::from_slice(&raw).ok()?;
    if domains.is_empty() {
        return None;
    }
    let mut rules = String::new();
    for domain in domains {
        let d = domain.trim();
        if d.is_empty() {
            continue;
        }
        let next_rules = [format!("MAP {d} 0.0.0.0"), format!("MAP *.{d} 0.0.0.0")];
        for rule in next_rules {
            let projected_len = if rules.is_empty() {
                rule.len()
            } else {
                rules.len() + 2 + rule.len()
            };
            if projected_len > max_len {
                return None;
            }
            if !rules.is_empty() {
                rules.push_str(", ");
            }
            rules.push_str(&rule);
        }
    }
    (!rules.is_empty()).then_some(rules)
}

fn chromium_extension_version(raw: &str) -> String {
    let normalized = raw.trim().trim_start_matches('v');
    let mut parts = Vec::new();
    for segment in normalized.split(['.', '-']) {
        if segment.is_empty() {
            continue;
        }
        if segment.chars().all(|ch| ch.is_ascii_digit()) {
            parts.push(segment.to_string());
        } else {
            break;
        }
        if parts.len() == 4 {
            break;
        }
    }
    if parts.is_empty() {
        "1".to_string()
    } else {
        parts.join(".")
    }
}

fn prepare_chromium_blocking_extension(profile_root: &Path) -> Result<Option<PathBuf>, EngineError> {
    let blocked_domains = blocked_domains_for_profile(profile_root)?;
    let locked_app = load_locked_app_config(profile_root)?;
    let identity = load_identity_launch_policy(profile_root);
    let accept_languages = identity
        .as_ref()
        .map(|policy| {
            normalize_accept_languages(&policy.locale.navigator_language, &policy.locale.languages)
        })
        .unwrap_or_default();
    if blocked_domains.is_empty() && locked_app.is_none() && accept_languages.is_empty() {
        return Ok(None);
    }

    let extension_dir = profile_root.join("policy").join("chromium-policy-extension");
    fs::create_dir_all(&extension_dir)?;
    let manifest = serde_json::json!({
        "manifest_version": 3,
        "name": "Cerbena Policy Firewall",
        "version": chromium_extension_version(CHROMIUM_POLICY_EXTENSION_VERSION),
        "description": "Profile-scoped outbound policy enforcement for blocked domains.",
        "declarative_net_request": {
            "rule_resources": [
                {
                    "id": "policy_rules",
                    "enabled": true,
                    "path": "rules.json"
                }
            ]
        },
        "permissions": [
            "declarativeNetRequest",
            "declarativeNetRequestFeedback",
            "declarativeNetRequestWithHostAccess"
        ],
        "host_permissions": ["<all_urls>"]
    });
    let mut rules = Vec::new();
    if !accept_languages.is_empty() {
        let accept_language_header = build_accept_language_header(&accept_languages);
        rules.push(serde_json::json!({
            "id": 1,
            "priority": 1,
            "action": {
                "type": "modifyHeaders",
                "requestHeaders": [
                    {
                        "header": "Accept-Language",
                        "operation": "set",
                        "value": accept_language_header
                    }
                ]
            },
            "condition": {
                "regexFilter": "^https?://",
                "resourceTypes": [
                    "main_frame",
                    "sub_frame",
                    "stylesheet",
                    "script",
                    "image",
                    "font",
                    "object",
                    "xmlhttprequest",
                    "ping",
                    "media",
                    "websocket",
                    "webtransport",
                    "other"
                ]
            }
        }));
    }
    let base_rule_id = rules.len() + 1;
    rules.extend(
        blocked_domains
            .into_iter()
            .enumerate()
            .map(|(index, domain)| {
                serde_json::json!({
                    "id": base_rule_id + index,
                    "priority": 2,
                    "action": { "type": "block" },
                    "condition": {
                        "urlFilter": format!("||{domain}^"),
                        "resourceTypes": [
                            "main_frame",
                            "sub_frame",
                            "stylesheet",
                            "script",
                            "image",
                            "font",
                            "object",
                            "xmlhttprequest",
                            "ping",
                            "media",
                            "websocket",
                            "webtransport",
                            "other"
                        ]
                    }
                })
            }),
    );
    if let Some(config) = locked_app {
        let allowed_hosts = config
            .allowed_hosts
            .into_iter()
            .map(|host| host.trim().trim_start_matches('.').to_lowercase())
            .filter(|host| !host.is_empty())
            .collect::<Vec<_>>();
        if !allowed_hosts.is_empty() {
            rules.push(serde_json::json!({
                "id": rules.len() + 1,
                "priority": 3,
                "action": { "type": "block" },
                "condition": {
                    "regexFilter": "^https?://",
                    "excludedRequestDomains": allowed_hosts,
                    "resourceTypes": [
                        "main_frame",
                        "sub_frame",
                        "stylesheet",
                        "script",
                        "image",
                        "font",
                        "object",
                        "xmlhttprequest",
                        "ping",
                        "media",
                        "websocket",
                        "webtransport",
                        "other"
                    ]
                }
            }));
        }
    }

    fs::write(
        extension_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        extension_dir.join("rules.json"),
        serde_json::to_vec_pretty(&rules)?,
    )?;
    Ok(Some(extension_dir))
}

fn load_locked_app_config(profile_root: &Path) -> Result<Option<LockedAppConfig>, EngineError> {
    let path = profile_root.join("policy").join("locked-app.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read(path)?;
    let config = serde_json::from_slice::<LockedAppConfig>(&raw)?;
    Ok(Some(config))
}

fn resolve_locked_app_target_url(config: &LockedAppConfig, requested_url: &str) -> String {
    let trimmed = requested_url.trim();
    if trimmed.is_empty() {
        return config.start_url.clone();
    }
    let Ok(parsed) = reqwest::Url::parse(trimmed) else {
        return config.start_url.clone();
    };
    let Some(host) = parsed.host_str().map(|value| value.to_ascii_lowercase()) else {
        return config.start_url.clone();
    };
    let allowed = config.allowed_hosts.iter().any(|candidate| {
        let normalized = candidate
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        host == normalized || host.ends_with(&format!(".{normalized}"))
    });
    if allowed {
        trimmed.to_string()
    } else {
        config.start_url.clone()
    }
}

fn prepare_chromium_extension_dirs(profile_root: &Path) -> Result<Vec<PathBuf>, EngineError> {
    let mut dirs = Vec::new();
    if let Some(blocking_extension) = prepare_chromium_blocking_extension(profile_root)? {
        dirs.push(blocking_extension);
    }
    let profile_extensions_root = profile_root.join("policy").join("chromium-extensions");
    if profile_extensions_root.exists() {
        let mut discovered = fs::read_dir(profile_extensions_root)?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        discovered.sort();
        dirs.extend(discovered);
    }
    Ok(dirs)
}

fn blocked_domains_for_profile(profile_root: &Path) -> Result<Vec<String>, EngineError> {
    let path = profile_root.join("policy").join("blocked-domains.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read(path)?;
    let domains: Vec<String> = serde_json::from_slice(&raw)?;
    let normalized = domains
        .into_iter()
        .map(|domain| domain.trim().trim_start_matches('.').to_lowercase())
        .filter(|domain| !domain.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{
        blocked_domains_for_profile, build_accept_language_header, chromium_extension_version,
        extract_librewolf_download_url, launch_args, parse_librewolf_version_from_file_name,
        prefer_chromium_vendor_binary, prepare_chromium_blocking_extension,
        chromium_launch_environment, select_ungoogled_chromium_asset, EngineKind, EngineRuntime,
        GithubAsset, GithubRelease, CHROMIUM_POLICY_EXTENSION_VERSION,
    };
    use std::fs;

    #[test]
    fn prepares_chromium_blocking_extension_from_blocked_domains() {
        let temp = tempfile::tempdir().expect("tempdir");
        let policy_dir = temp.path().join("policy");
        fs::create_dir_all(&policy_dir).expect("policy dir");
        fs::write(
            policy_dir.join("blocked-domains.json"),
            serde_json::to_vec(&vec![
                "youtube.com".to_string(),
                ".example.com".to_string(),
                "youtube.com".to_string(),
            ])
            .expect("serialize blocked domains"),
        )
        .expect("write blocked domains");

        let extension_dir = prepare_chromium_blocking_extension(temp.path())
            .expect("prepare extension")
            .expect("extension dir");

        let manifest_raw =
            fs::read_to_string(extension_dir.join("manifest.json")).expect("manifest");
        let rules_raw = fs::read_to_string(extension_dir.join("rules.json")).expect("rules");
        assert!(manifest_raw.contains("\"manifest_version\": 3"));
        assert!(manifest_raw.contains(&format!(
            "\"version\": \"{}\"",
            chromium_extension_version(CHROMIUM_POLICY_EXTENSION_VERSION)
        )));
        assert!(rules_raw.contains("||youtube.com^"));
        assert!(rules_raw.contains("||example.com^"));
    }

    #[test]
    fn chromium_extension_version_normalizes_hotfix_suffixes() {
        assert_eq!(chromium_extension_version("1.2.3"), "1.2.3");
        assert_eq!(chromium_extension_version("v1.2.3"), "1.2.3");
        assert_eq!(chromium_extension_version("7"), "7");
    }

    #[test]
    fn normalizes_blocked_domains_for_chromium_extension() {
        let temp = tempfile::tempdir().expect("tempdir");
        let policy_dir = temp.path().join("policy");
        fs::create_dir_all(&policy_dir).expect("policy dir");
        fs::write(
            policy_dir.join("blocked-domains.json"),
            serde_json::to_vec(&vec![
                " Reddit.com ".to_string(),
                ".reddit.com".to_string(),
                "".to_string(),
            ])
            .expect("serialize blocked domains"),
        )
        .expect("write blocked domains");

        let domains = blocked_domains_for_profile(temp.path()).expect("domains");
        assert_eq!(domains, vec!["reddit.com".to_string()]);
    }

    #[test]
    fn prepares_locked_app_block_rule_for_chromium_extension() {
        let temp = tempfile::tempdir().expect("tempdir");
        let policy_dir = temp.path().join("policy");
        fs::create_dir_all(&policy_dir).expect("policy dir");
        fs::write(
            policy_dir.join("locked-app.json"),
            serde_json::to_vec(&serde_json::json!({
                "startUrl": "https://discord.com/app",
                "allowedHosts": ["discord.com", "discord.gg"]
            }))
            .expect("serialize locked app"),
        )
        .expect("write locked app");

        let extension_dir = prepare_chromium_blocking_extension(temp.path())
            .expect("prepare extension")
            .expect("extension dir");
        let rules_raw = fs::read_to_string(extension_dir.join("rules.json")).expect("rules");
        assert!(rules_raw.contains("\"regexFilter\": \"^https?://\""));
        assert!(rules_raw.contains("discord.com"));
        assert!(rules_raw.contains("excludedRequestDomains"));
    }

    #[test]
    fn prepares_accept_language_rule_for_chromium_extension() {
        let temp = tempfile::tempdir().expect("tempdir");
        let policy_dir = temp.path().join("policy");
        fs::create_dir_all(&policy_dir).expect("policy dir");
        fs::write(
            policy_dir.join("identity-preset.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "locale": {
                    "navigator_language": "ru",
                    "languages": ["ru", "en", "en-US"]
                }
            }))
            .expect("serialize identity policy"),
        )
        .expect("write identity policy");

        let extension_dir = prepare_chromium_blocking_extension(temp.path())
            .expect("prepare extension")
            .expect("extension dir");
        let manifest_raw =
            fs::read_to_string(extension_dir.join("manifest.json")).expect("manifest");
        let rules_raw = fs::read_to_string(extension_dir.join("rules.json")).expect("rules");
        assert!(manifest_raw.contains("declarativeNetRequestWithHostAccess"));
        assert!(manifest_raw.contains("\"host_permissions\": ["));
        assert!(rules_raw.contains("\"type\": \"modifyHeaders\""));
        assert!(rules_raw.contains("\"header\": \"Accept-Language\""));
        assert!(rules_raw.contains("\"value\": \"ru,en;q=0.9,en-US;q=0.8\""));
    }

    #[test]
    fn accept_language_header_uses_browser_like_quality_weights() {
        assert_eq!(
            build_accept_language_header(&[
                "ru-RU".to_string(),
                "ru".to_string(),
                "en-US".to_string(),
                "en".to_string(),
            ]),
            "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7"
        );
    }

    #[test]
    fn chromium_launch_args_skip_accept_terms_flag() {
        let temp = tempfile::tempdir().expect("tempdir");
        let args = launch_args(
            EngineKind::Chromium,
            temp.path(),
            "https://duckduckgo.com",
            false,
            None,
            false,
        )
        .expect("launch args");

        assert!(!args
            .iter()
            .any(|value| value == "--accept-terms-and-conditions"));
        assert!(!args.iter().any(|value| value == "--enable-logging"));
        assert!(!args.iter().any(|value| value.contains("--log-file=")));
    }

    #[test]
    fn ungoogled_chromium_launch_args_reuse_chromium_family_behavior() {
        let temp = tempfile::tempdir().expect("tempdir");
        let args = launch_args(
            EngineKind::UngoogledChromium,
            temp.path(),
            "https://duckduckgo.com",
            true,
            None,
            true,
        )
        .expect("launch args");

        let expected_user_data_dir =
            format!("--user-data-dir={}", temp.path().join("engine-profile").to_string_lossy());
        assert!(args
            .iter()
            .any(|value| value == &expected_user_data_dir));
        assert!(args.iter().any(|value| value == "--incognito"));
        assert!(args.iter().any(|value| value == "--disable-sync"));
        assert!(args.iter().any(|value| value == "https://duckduckgo.com"));
    }

    #[test]
    fn chromium_launch_args_apply_identity_policy_overrides() {
        let temp = tempfile::tempdir().expect("tempdir");
        let policy_dir = temp.path().join("policy");
        fs::create_dir_all(&policy_dir).expect("policy dir");
        fs::write(
            policy_dir.join("identity-preset.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "core": {
                    "user_agent": "Mozilla/5.0 Test Browser"
                },
                "locale": {
                    "navigator_language": "ru",
                    "languages": ["ru", "en", "en-GB", "en-US"]
                },
                "window": {
                    "outer_width": 1440,
                    "outer_height": 920,
                    "screen_x": 320,
                    "screen_y": 343
                },
                "screen": {
                    "width": 2560,
                    "height": 1440
                }
            }))
            .expect("serialize identity policy"),
        )
        .expect("write identity policy");

        let args = launch_args(
            EngineKind::Chromium,
            temp.path(),
            "https://duckduckgo.com",
            false,
            None,
            false,
        )
        .expect("launch args");

        assert!(args
            .iter()
            .any(|value| value == "--user-agent=Mozilla/5.0 Test Browser"));
        assert!(args.iter().any(|value| value == "--lang=ru"));
        assert!(args
            .iter()
            .any(|value| value == "--accept-lang=ru,en,en-GB,en-US"));
        assert!(args.iter().any(|value| value == "--window-size=1440,920"));
        assert!(args
            .iter()
            .any(|value| value == "--window-position=320,343"));

        let preferences: serde_json::Value = serde_json::from_slice(
            &fs::read(
                temp.path()
                    .join("engine-profile")
                    .join("Default")
                    .join("Preferences"),
            )
            .expect("read preferences"),
        )
        .expect("parse preferences");
        assert_eq!(
            preferences["intl"]["accept_languages"].as_str(),
            Some("ru,en,en-GB,en-US")
        );
        assert_eq!(
            preferences["intl"]["selected_languages"].as_str(),
            Some("ru,en,en-GB,en-US")
        );
        let local_state: serde_json::Value = serde_json::from_slice(
            &fs::read(temp.path().join("engine-profile").join("Local State"))
                .expect("read local state"),
        )
        .expect("parse local state");
        assert_eq!(local_state["intl"]["app_locale"].as_str(), Some("ru"));
        assert_eq!(
            local_state["intl"]["selected_languages"].as_str(),
            Some("ru,en,en-GB,en-US")
        );

        let env = chromium_launch_environment(temp.path());
        assert_eq!(
            env,
            vec![
                ("LANG".to_string(), "ru.UTF-8".to_string()),
                ("LANGUAGE".to_string(), "ru:en:en-GB:en-US".to_string()),
                ("LC_ALL".to_string(), "ru.UTF-8".to_string()),
            ]
        );
    }

    #[test]
    fn chromium_real_mode_keeps_native_user_agent() {
        let temp = tempfile::tempdir().expect("tempdir");
        let policy_dir = temp.path().join("policy");
        fs::create_dir_all(&policy_dir).expect("policy dir");
        fs::write(
            policy_dir.join("identity-preset.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "mode": "real",
                "core": {
                    "user_agent": "Mozilla/5.0 Launcher WebView"
                },
                "locale": {
                    "navigator_language": "ru-RU",
                    "languages": ["ru-RU", "ru", "en-US"]
                }
            }))
            .expect("serialize identity policy"),
        )
        .expect("write identity policy");

        let args = launch_args(
            EngineKind::Chromium,
            temp.path(),
            "https://duckduckgo.com",
            false,
            None,
            true,
        )
        .expect("launch args");

        assert!(!args
            .iter()
            .any(|value| value == "--user-agent=Mozilla/5.0 Launcher WebView"));
        assert!(args.iter().any(|value| value == "--lang=ru-RU"));
        assert!(args
            .iter()
            .any(|value| value == "--accept-lang=ru-RU,ru,en-US"));
    }

    #[test]
    fn chromium_prefers_vendor_chrome_binary_over_launcher_alias() {
        let temp = tempfile::tempdir().expect("tempdir");
        let chrome = temp.path().join("chrome.exe");
        let alias = temp.path().join("chromium-browser.exe");
        fs::write(&chrome, b"vendor chrome").expect("write chrome stub");
        fs::write(&alias, b"launcher alias").expect("write alias stub");

        let runtime = EngineRuntime::new(temp.path().join("state")).expect("runtime");
        let launch_binary = runtime
            .locate_binary(EngineKind::Chromium, temp.path())
            .expect("locate chromium binary");

        assert_eq!(launch_binary, chrome);
    }

    #[test]
    fn chromium_prefers_vendor_chrome_from_stored_alias_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let chrome = temp.path().join("chrome.exe");
        let alias = temp.path().join("chromium-browser.exe");
        fs::write(&chrome, b"vendor chrome").expect("write chrome stub");
        fs::write(&alias, b"launcher alias").expect("write alias stub");

        let resolved = prefer_chromium_vendor_binary(&alias);

        assert_eq!(resolved, chrome);
    }

    #[test]
    fn ungoogled_chromium_prefers_vendor_chrome_binary_over_named_wrapper() {
        let temp = tempfile::tempdir().expect("tempdir");
        let chrome = temp.path().join("chrome.exe");
        let wrapper = temp.path().join("ungoogled-chromium.exe");
        fs::write(&chrome, b"vendor chrome").expect("write chrome stub");
        fs::write(&wrapper, b"wrapper").expect("write wrapper stub");

        let runtime = EngineRuntime::new(temp.path().join("state")).expect("runtime");
        let launch_binary = runtime
            .locate_binary(EngineKind::UngoogledChromium, temp.path())
            .expect("locate ungoogled chromium binary");

        assert_eq!(launch_binary, chrome);
    }

    #[test]
    fn extracts_official_librewolf_windows_portable_download_url() {
        let html = r#"
        <a href="https://dl.librewolf.net/release/windows/x86_64/librewolf-150.0.1-1-windows-x86_64-portable.zip">
            Download portable
        </a>
        "#;

        let url = extract_librewolf_download_url(html, "windows-x86_64-portable.zip")
            .expect("portable url");
        assert_eq!(
            url,
            "https://dl.librewolf.net/release/windows/x86_64/librewolf-150.0.1-1-windows-x86_64-portable.zip"
        );
    }

    #[test]
    fn parses_librewolf_version_from_windows_file_name() {
        assert_eq!(
            parse_librewolf_version_from_file_name(
                "librewolf-150.0.1-1-windows-x86_64-portable.zip"
            ),
            Some("150.0.1-1".to_string())
        );
    }

    #[test]
    fn selects_exact_ungoogled_chromium_windows_asset_from_release() {
        let release = GithubRelease {
            tag_name: "147.0.7727.137-1.1".to_string(),
            assets: vec![
                GithubAsset {
                    name: "ungoogled-chromium_147.0.7727.137-1.1_installer_x64.exe"
                        .to_string(),
                    browser_download_url: "https://example.invalid/installer.exe".to_string(),
                },
                GithubAsset {
                    name: "ungoogled-chromium_147.0.7727.137-1.1_windows_x64.zip".to_string(),
                    browser_download_url:
                        "https://github.com/ungoogled-software/ungoogled-chromium-windows/releases/download/147.0.7727.137-1.1/ungoogled-chromium_147.0.7727.137-1.1_windows_x64.zip"
                            .to_string(),
                },
            ],
        };

        let asset = select_ungoogled_chromium_asset(&release)
            .expect("select asset")
            .expect("matching asset");

        assert_eq!(
            asset.name,
            "ungoogled-chromium_147.0.7727.137-1.1_windows_x64.zip"
        );
        assert_eq!(
            asset.browser_download_url,
            "https://github.com/ungoogled-software/ungoogled-chromium-windows/releases/download/147.0.7727.137-1.1/ungoogled-chromium_147.0.7727.137-1.1_windows_x64.zip"
        );
    }
}

fn now_epoch_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn ungoogled_chromium_releases_url() -> Result<&'static str, EngineError> {
    if cfg!(target_os = "windows") {
        Ok(UNGOOGLED_CHROMIUM_WINDOWS_RELEASES_URL)
    } else if cfg!(target_os = "macos") {
        Ok(UNGOOGLED_CHROMIUM_MACOS_RELEASES_URL)
    } else if cfg!(target_os = "linux") {
        Ok(UNGOOGLED_CHROMIUM_LINUX_RELEASES_URL)
    } else {
        Err(EngineError::Download(
            "ungoogled-chromium is not supported on this platform".to_string(),
        ))
    }
}

fn ungoogled_chromium_asset_suffixes() -> Result<Vec<String>, EngineError> {
    if cfg!(target_os = "windows") {
        Ok(vec![
            "_windows_x64.zip".to_string(),
            "portable_windows_x64.zip".to_string(),
            "windows_x64.zip".to_string(),
        ])
    } else if cfg!(target_os = "macos") {
        Ok(vec![".dmg".to_string()])
    } else if cfg!(target_os = "linux") {
        Ok(vec![".tar.xz".to_string()])
    } else {
        Err(EngineError::Download(
            "ungoogled-chromium is not supported on this platform".to_string(),
        ))
    }
}

fn ungoogled_chromium_asset_candidates(version: &str) -> Result<Vec<String>, EngineError> {
    if cfg!(target_os = "windows") {
        Ok(vec![
            format!("ungoogled-chromium_{version}_windows_x64.zip"),
            format!("ungoogled-chromium_{version}_portable_windows_x64.zip"),
            format!("ungoogled-chromium_{version}_installer_x64.exe"),
        ])
    } else if cfg!(target_os = "macos") {
        Ok(vec![format!("ungoogled-chromium_{version}.dmg")])
    } else if cfg!(target_os = "linux") {
        Ok(vec![format!("ungoogled-chromium_{version}.tar.xz")])
    } else {
        Err(EngineError::Download(
            "ungoogled-chromium is not supported on this platform".to_string(),
        ))
    }
}

fn select_ungoogled_chromium_asset(
    release: &GithubRelease,
) -> Result<Option<GithubAsset>, EngineError> {
    let candidates = ungoogled_chromium_asset_candidates(&release.tag_name)?;
    if let Some(asset) = candidates.iter().find_map(|candidate| {
        release
            .assets
            .iter()
            .find(|item| item.name.eq_ignore_ascii_case(candidate))
            .cloned()
    }) {
        return Ok(Some(asset));
    }

    let suffixes = ungoogled_chromium_asset_suffixes()?;
    Ok(release.assets.iter().find_map(|item| {
        let lower = item.name.to_ascii_lowercase();
        if lower.contains("ungoogled")
            && suffixes.iter().any(|suffix| lower.ends_with(suffix))
        {
            Some(item.clone())
        } else {
            None
        }
    }))
}
