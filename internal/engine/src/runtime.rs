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
    camoufox::CamoufoxAdapter,
    contract::{EngineAdapter, EngineError, EngineKind},
    progress::EngineDownloadProgress,
    registry::EngineRegistry,
    wayfern::WayfernAdapter,
};

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/136.0.0.0 Safari/537.36";
const WAYFERN_VERSION_URLS: [&str; 2] = [
    "https://download.wayfern.com/version.json",
    "https://donutbrowser.com/wayfern.json",
];
const CAMOUFOX_RELEASES_URL: &str =
    "https://api.github.com/repos/daijro/camoufox/releases?per_page=20";

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
    tos_version: String,
}

#[derive(Debug, Clone)]
struct ResolvedArtifact {
    engine: EngineKind,
    version: String,
    download_url: String,
    file_name: String,
}

#[derive(Debug, Deserialize)]
struct WayfernVersionInfo {
    version: String,
    downloads: std::collections::HashMap<String, Option<String>>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
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
            tos_version: "2026-04".to_string(),
        })
    }

    pub fn acknowledge_wayfern_tos(
        &self,
        profile_root: &Path,
        profile_id: uuid::Uuid,
    ) -> Result<(), EngineError> {
        self.wayfern_adapter()
            .acknowledge_tos(profile_root, profile_id)
    }

    pub fn installed(&self, engine: EngineKind) -> Result<Option<EngineInstallation>, EngineError> {
        self.registry.get(engine)
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
        if let Some(installed) = self.registry.get(engine)? {
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
            .registry
            .get(engine)?
            .ok_or_else(|| EngineError::Launch("engine is not installed".to_string()))?;
        let binary_path = if matches!(engine, EngineKind::Camoufox) {
            prefer_camoufox_browser_binary(&installation.binary_path)
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
        };
        eprintln!(
            "[engine-runtime] launch {} profile={} binary={} args={:?}",
            engine.as_key(),
            profile_id,
            request.binary_path.display(),
            request.args
        );
        match engine {
            EngineKind::Wayfern => self.wayfern_adapter().launch(request),
            EngineKind::Camoufox => self.camoufox_adapter().launch(request),
        }
    }

    fn resolve_artifact(&self, engine: EngineKind) -> Result<ResolvedArtifact, EngineError> {
        match engine {
            EngineKind::Wayfern => self.resolve_wayfern_artifact(),
            EngineKind::Camoufox => self.resolve_camoufox_artifact(),
        }
    }

    fn resolve_wayfern_artifact(&self) -> Result<ResolvedArtifact, EngineError> {
        let client = http_client()?;
        let mut last_error = None;
        for url in WAYFERN_VERSION_URLS {
            match client.get(url).send() {
                Ok(response) if response.status().is_success() => {
                    let info: WayfernVersionInfo = response
                        .json()
                        .map_err(|e| EngineError::Download(e.to_string()))?;
                    let platform_key = current_platform_key()?;
                    let download_url = info
                        .downloads
                        .get(&platform_key)
                        .and_then(|value| value.clone())
                        .ok_or_else(|| {
                            EngineError::Install(format!(
                                "Wayfern has no compatible download for platform {platform_key}"
                            ))
                        })?;
                    return Ok(ResolvedArtifact {
                        engine: EngineKind::Wayfern,
                        version: info.version,
                        file_name: file_name_from_url(&download_url, "wayfern")?,
                        download_url,
                    });
                }
                Ok(response) => {
                    last_error = Some(format!("{url}: HTTP {}", response.status()));
                }
                Err(error) => {
                    last_error = Some(format!("{url}: {error}"));
                }
            }
        }
        Err(EngineError::Download(last_error.unwrap_or_else(|| {
            "failed to resolve Wayfern version".to_string()
        })))
    }

    fn resolve_camoufox_artifact(&self) -> Result<ResolvedArtifact, EngineError> {
        let client = http_client()?;
        let response = client
            .get(CAMOUFOX_RELEASES_URL)
            .send()
            .map_err(|e| EngineError::Download(e.to_string()))?;
        if !response.status().is_success() {
            return Err(EngineError::Download(format!(
                "camoufox releases request failed with HTTP {}",
                response.status()
            )));
        }
        let releases: Vec<GithubRelease> = response
            .json()
            .map_err(|e| EngineError::Download(e.to_string()))?;
        let suffix = camoufox_asset_suffix()?;
        for release in releases {
            if let Some(asset) = release.assets.into_iter().find(|item| {
                let lower = item.name.to_lowercase();
                lower.starts_with("camoufox-") && lower.ends_with(&suffix)
            }) {
                return Ok(ResolvedArtifact {
                    engine: EngineKind::Camoufox,
                    version: release.tag_name,
                    file_name: asset.name,
                    download_url: asset.browser_download_url,
                });
            }
        }
        Err(EngineError::Download(format!(
            "no compatible Camoufox asset found for suffix {suffix}"
        )))
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
            return self.download_artifact_with_curl(artifact, emit, should_cancel);
        }

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
            if downloaded == 0 && start.elapsed().as_secs() >= 30 {
                let _ = child.kill();
                let _ = child.wait();
                let host_label = host
                    .clone()
                    .unwrap_or_else(|| artifact.download_url.clone());
                return Err(EngineError::Download(format!(
                    "no bytes received from {host_label} within 30 seconds"
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
            EngineKind::Wayfern => {
                candidate_names(&["wayfern.exe", "wayfern", "chrome.exe", "chrome"])
            }
            EngineKind::Camoufox => candidate_names(&[
                "camoufox.exe",
                "camoufox-bin.exe",
                "camoufox",
                "camoufox-bin",
                "firefox.exe",
                "firefox",
            ]),
        };
        find_first_match(root, &candidates).ok_or_else(|| {
            EngineError::Install(format!(
                "unable to locate {} executable under {}",
                engine.as_key(),
                root.display()
            ))
        })
    }

    fn wayfern_adapter(&self) -> WayfernAdapter {
        WayfernAdapter {
            install_root: self.install_root.clone(),
            cache_dir: self.cache_dir.clone(),
            tos_version: self.tos_version.clone(),
        }
    }

    fn camoufox_adapter(&self) -> CamoufoxAdapter {
        CamoufoxAdapter {
            install_root: self.install_root.clone(),
            cache_dir: self.cache_dir.clone(),
        }
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

fn current_platform_key() -> Result<String, EngineError> {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        return Err(EngineError::Install(
            "unsupported operating system".to_string(),
        ));
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        return Err(EngineError::Install("unsupported architecture".to_string()));
    };
    Ok(format!("{os}-{arch}"))
}

fn camoufox_asset_suffix() -> Result<String, EngineError> {
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

fn prefer_camoufox_browser_binary(current: &Path) -> PathBuf {
    let Some(root) = current.parent() else {
        return current.to_path_buf();
    };
    let camoufox_candidates = candidate_names(&["camoufox.exe", "camoufox"]);
    if let Some(path) = find_first_match(root, &camoufox_candidates) {
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
        EngineKind::Wayfern => {
            // Keep launch command size bounded on Windows. Huge domain blocklists can
            // overflow CreateProcess argument limits when passed via host resolver rules.
            const MAX_HOST_RESOLVER_RULES_LEN: usize = 8_192;
            let mut args = vec![
                format!("--user-data-dir={}", runtime_dir.to_string_lossy()),
                "--no-first-run".to_string(),
                "--no-default-browser-check".to_string(),
                "--disable-background-mode".to_string(),
                "--disable-quic".to_string(),
                if runtime_hardening {
                    "--disable-features=AsyncDns,DnsHttpssvc,AutofillServerCommunication,AutofillEnableAccountWalletStorage,PasswordManagerOnboarding".to_string()
                } else {
                    "--disable-features=AsyncDns,DnsHttpssvc".to_string()
                },
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
                    wayfern_host_resolver_rules(profile_root, MAX_HOST_RESOLVER_RULES_LEN)
                {
                    args.push(format!("--host-resolver-rules={host_rules}"));
                }
            }
            let extension_dirs = prepare_wayfern_extension_dirs(profile_root)?;
            if !extension_dirs.is_empty() {
                let joined = extension_dirs
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                args.push(format!("--load-extension={joined}"));
            }
            args.push(start_page.to_string());
            Ok(args)
        }
        EngineKind::Camoufox => {
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

fn wayfern_host_resolver_rules(profile_root: &Path, max_len: usize) -> Option<String> {
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

fn prepare_wayfern_blocking_extension(profile_root: &Path) -> Result<Option<PathBuf>, EngineError> {
    let blocked_domains = blocked_domains_for_profile(profile_root)?;
    if blocked_domains.is_empty() {
        return Ok(None);
    }

    let extension_dir = profile_root.join("policy").join("wayfern-policy-extension");
    fs::create_dir_all(&extension_dir)?;
    let manifest = serde_json::json!({
        "manifest_version": 3,
        "name": "Cerbena Policy Firewall",
        "version": "1.0.6-2",
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
        "permissions": ["declarativeNetRequest", "declarativeNetRequestFeedback"]
    });
    let rules = blocked_domains
        .into_iter()
        .enumerate()
        .map(|(index, domain)| {
            serde_json::json!({
                "id": index + 1,
                "priority": 1,
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
        })
        .collect::<Vec<_>>();

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

fn prepare_wayfern_extension_dirs(profile_root: &Path) -> Result<Vec<PathBuf>, EngineError> {
    let mut dirs = Vec::new();
    if let Some(blocking_extension) = prepare_wayfern_blocking_extension(profile_root)? {
        dirs.push(blocking_extension);
    }
    let profile_extensions_root = profile_root.join("policy").join("wayfern-extensions");
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
    use super::{blocked_domains_for_profile, prepare_wayfern_blocking_extension};
    use std::fs;

    #[test]
    fn prepares_wayfern_blocking_extension_from_blocked_domains() {
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

        let extension_dir = prepare_wayfern_blocking_extension(temp.path())
            .expect("prepare extension")
            .expect("extension dir");

        let manifest_raw =
            fs::read_to_string(extension_dir.join("manifest.json")).expect("manifest");
        let rules_raw = fs::read_to_string(extension_dir.join("rules.json")).expect("rules");
        assert!(manifest_raw.contains("\"manifest_version\": 3"));
        assert!(rules_raw.contains("||youtube.com^"));
        assert!(rules_raw.contains("||example.com^"));
    }

    #[test]
    fn normalizes_blocked_domains_for_wayfern_extension() {
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
}

fn now_epoch_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

impl EngineKind {
    pub fn as_key(&self) -> &'static str {
        match self {
            EngineKind::Wayfern => "wayfern",
            EngineKind::Camoufox => "camoufox",
        }
    }
}
