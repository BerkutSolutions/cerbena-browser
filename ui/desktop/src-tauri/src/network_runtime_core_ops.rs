use super::*;

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

    pub(crate) fn install_sing_box<F>(&self, emit: &mut F) -> Result<NetworkInstallPaths, String>
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

    pub(crate) fn install_openvpn<F>(&self, emit: &mut F) -> Result<NetworkInstallPaths, String>
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

    pub(crate) fn install_tor_bundle<F>(&self, emit: &mut F) -> Result<NetworkInstallPaths, String>
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

    pub(crate) fn install_amneziawg<F>(&self, emit: &mut F) -> Result<NetworkInstallPaths, String>
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

    pub(crate) fn download_file<F>(
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

    pub(crate) fn fetch_tor_bundle_sha256(&self, archive_file: &str) -> Result<String, String> {
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

