use super::*;

impl EngineRuntime {
    pub(super) fn download_artifact_impl<F, C>(
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
            match self.download_artifact_with_curl_impl(artifact, emit, should_cancel) {
                Ok(path) => return Ok(path),
                Err(EngineError::Download(message)) if should_fallback_to_reqwest_impl(&message) => {
                    eprintln!(
                        "[engine-runtime] curl download fallback {} {} reason={}",
                        artifact.engine.as_key(),
                        artifact.version,
                        message
                    );
                    return self.download_artifact_with_reqwest_impl(artifact, emit, should_cancel);
                }
                Err(error) => return Err(error),
            }
        }

        self.download_artifact_with_reqwest_impl(artifact, emit, should_cancel)
    }

    pub(super) fn download_artifact_with_reqwest_impl<F, C>(
        &self,
        artifact: &ResolvedArtifact,
        emit: &mut F,
        should_cancel: &C,
    ) -> Result<PathBuf, EngineError>
    where
        F: FnMut(EngineDownloadProgress),
        C: Fn() -> bool,
    {
        let client = http_client_impl()?;
        fs::create_dir_all(&self.cache_dir)?;
        let target = self.cache_dir.join(&artifact.file_name);
        let host = host_from_url_impl(&artifact.download_url);
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
                let progress = build_transfer_progress_impl(
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

        emit(build_transfer_progress_impl(
            artifact.engine,
            artifact.version.clone(),
            host.clone(),
            downloaded,
            total,
            start.elapsed().as_secs_f64(),
        ));
        Ok(target)
    }

    pub(super) fn download_artifact_with_curl_impl<F, C>(
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
        let host = host_from_url_impl(&artifact.download_url);
        let zero_bytes_timeout_secs = zero_bytes_timeout_secs_impl(&host);
        let total = probe_content_length_with_curl_impl(&artifact.download_url);

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
                emit(build_transfer_progress_impl(
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
                return Ok(target);
            }

            if last_emit.elapsed().as_millis() >= 200 {
                emit(build_transfer_progress_impl(
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
}

pub(super) fn http_client_impl() -> Result<Client, EngineError> {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| EngineError::Download(e.to_string()))
}

pub(super) fn build_transfer_progress_impl(
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

pub(super) fn probe_content_length_with_curl_impl(url: &str) -> Option<u64> {
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

pub(super) fn host_from_url_impl(url: &str) -> Option<String> {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|value| value.to_string()))
}

pub(super) fn zero_bytes_timeout_secs_impl(host: &Option<String>) -> u64 {
    let Some(host) = host.as_deref() else {
        return DEFAULT_ZERO_BYTES_TIMEOUT_SECS;
    };
    let host = host.to_ascii_lowercase();
    if host == "github.com"
        || host.ends_with(".github.com")
        || host.ends_with("githubusercontent.com")
    {
        return GITHUB_ZERO_BYTES_TIMEOUT_SECS;
    }
    DEFAULT_ZERO_BYTES_TIMEOUT_SECS
}

pub(super) fn should_fallback_to_reqwest_impl(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("curl download failed: curl: (28) connection timed out")
        || normalized.contains("curl download failed: curl: (28) failed to connect")
        || normalized.contains("no bytes received from")
}
