use super::*;

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
        eprintln!("[engine-runtime] resolving artifact for {}", engine.as_key());

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

        let target_dir =
            runtime_resolution::installation_target_dir(&self.install_root, engine, &artifact.version);
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
        #[cfg(unix)]
        ensure_engine_binary_executable(&binary_path)?;
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
        start_page: Option<String>,
        private_mode: bool,
        gateway_proxy_port: Option<u16>,
        runtime_hardening: bool,
    ) -> Result<u32, EngineError> {
        let installation = self
            .installed(engine)?
            .ok_or_else(|| EngineError::Launch("engine is not installed".to_string()))?;
        let binary_path = runtime_launch_dispatch::resolved_binary_for_engine(engine, installation);
        #[allow(unused_mut)]
        let mut request = crate::contract::LaunchRequest {
            profile_id,
            profile_root: profile_root.clone(),
            binary_path,
            args: launch_args(
                engine,
                &profile_root,
                start_page.as_deref(),
                private_mode,
                gateway_proxy_port,
                runtime_hardening,
            )?,
            env: launch_environment(engine, &profile_root),
        };
        if matches!(engine, EngineKind::Librewolf) {
            sanitize_librewolf_launch_args(&mut request.args);
        }
        #[cfg(target_os = "linux")]
        {
            if matches!(engine, EngineKind::Chromium | EngineKind::UngoogledChromium)
                && linux_requires_no_sandbox_for_binary(&request.binary_path)
            {
                eprintln!(
                    "[engine-runtime] linux sandbox unavailable: {}; launching Chromium in compatibility mode",
                    linux_sandbox_probe_summary()
                );
                request.args.push("--disable-setuid-sandbox".to_string());
                request.args.push("--no-sandbox".to_string());
            }
        }
        #[cfg(unix)]
        ensure_engine_binary_executable(&request.binary_path)?;
        #[cfg(unix)]
        ensure_engine_helpers_executable(engine, &request.binary_path)?;
        eprintln!(
            "[engine-runtime] launch {} profile={} binary={} args={:?}",
            engine.as_key(),
            profile_id,
            request.binary_path.display(),
            request.args
        );
        runtime_launch_dispatch::dispatch_launch(self, engine, request)
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
        let binary_path = runtime_launch_dispatch::resolved_binary_for_engine(engine, installation);
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
        runtime_resolution::resolve_artifact(engine)
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
        self.download_artifact_impl(artifact, emit, should_cancel)
    }

    fn install_archive(&self, archive_path: &Path, target_dir: &Path) -> Result<(), EngineError> {
        self.install_archive_impl(archive_path, target_dir)
    }

    pub(super) fn locate_binary(&self, engine: EngineKind, root: &Path) -> Result<PathBuf, EngineError> {
        runtime_resolution::locate_binary(engine, root)
    }

    pub(crate) fn chromium_adapter(&self) -> ChromiumAdapter {
        ChromiumAdapter {
            install_root: self.install_root.clone(),
            cache_dir: self.cache_dir.clone(),
        }
    }

    pub(crate) fn ungoogled_chromium_adapter(&self) -> UngoogledChromiumAdapter {
        UngoogledChromiumAdapter {
            install_root: self.install_root.clone(),
            cache_dir: self.cache_dir.clone(),
        }
    }

    pub(crate) fn librewolf_adapter(&self) -> LibrewolfAdapter {
        LibrewolfAdapter {
            install_root: self.install_root.clone(),
            cache_dir: self.cache_dir.clone(),
        }
    }

    pub(crate) fn firefox_esr_adapter(&self) -> FirefoxEsrAdapter {
        FirefoxEsrAdapter {
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
            EngineKind::FirefoxEsr => prefer_librewolf_browser_binary(&installation.binary_path),
            EngineKind::Librewolf => installation.binary_path.clone(),
        };
        if normalized_binary_path != installation.binary_path {
            installation.binary_path = normalized_binary_path;
            self.registry.put(installation.clone())?;
        }
        Ok(installation)
    }
}
