use super::*;

impl EngineRuntime {
    pub(super) fn install_archive_impl(
        &self,
        archive_path: &Path,
        target_dir: &Path,
    ) -> Result<(), EngineError> {
        let lower = archive_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_lowercase();
        if lower.ends_with(".zip") {
            extract_zip_impl(archive_path, target_dir)
        } else if lower.ends_with(".tar.xz") {
            extract_tar_xz_impl(archive_path, target_dir)
        } else if lower.ends_with(".tar.bz2") {
            extract_tar_bz2_impl(archive_path, target_dir)
        } else if lower.ends_with(".msi") {
            install_windows_msi_archive_impl(archive_path, target_dir)
        } else if lower.ends_with(".exe") {
            extract_firefox_setup_exe_impl(archive_path, target_dir)
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
}

pub(super) fn extract_zip_impl(archive_path: &Path, target_dir: &Path) -> Result<(), EngineError> {
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
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            fs::set_permissions(&out_path, fs::Permissions::from_mode(mode))
                .map_err(|e| EngineError::Install(format!("chmod extracted file failed: {e}")))?;
        }
    }
    Ok(())
}

pub(super) fn extract_tar_xz_impl(
    archive_path: &Path,
    target_dir: &Path,
) -> Result<(), EngineError> {
    let file = fs::File::open(archive_path)?;
    let decoder = XzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(target_dir)
        .map_err(|e| EngineError::Install(format!("tar.xz unpack failed: {e}")))?;
    Ok(())
}

pub(super) fn extract_tar_bz2_impl(
    archive_path: &Path,
    target_dir: &Path,
) -> Result<(), EngineError> {
    let mut command = Command::new("tar");
    command
        .arg("-xjf")
        .arg(archive_path)
        .arg("-C")
        .arg(target_dir);
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .map_err(|e| EngineError::Install(format!("failed to spawn tar for .tar.bz2: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(EngineError::Install(format!(
            "tar.bz2 unpack failed: {}",
            if stderr.is_empty() {
                format!("exit code {:?}", output.status.code())
            } else {
                stderr
            }
        )));
    }
    Ok(())
}

pub(super) fn install_windows_msi_archive_impl(
    archive_path: &Path,
    target_dir: &Path,
) -> Result<(), EngineError> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (archive_path, target_dir);
        return Err(EngineError::Install(
            "MSI installation is supported only on Windows".to_string(),
        ));
    }
    #[cfg(target_os = "windows")]
    {
        fs::create_dir_all(target_dir)?;
        let install_path = target_dir.display().to_string();
        let install_path_backslash = ensure_windows_path_with_trailing_backslash_impl(target_dir);
        let mut errors = Vec::new();

        let modes = vec![
            vec![
                "/i".to_string(),
                archive_path.display().to_string(),
                "/qn".to_string(),
                "/norestart".to_string(),
                format!("EXTRACT_DIR={install_path}"),
            ],
            vec![
                "/i".to_string(),
                archive_path.display().to_string(),
                "/qn".to_string(),
                "/norestart".to_string(),
                format!("INSTALL_DIRECTORY_PATH={install_path}"),
            ],
            vec![
                "/i".to_string(),
                archive_path.display().to_string(),
                "/qn".to_string(),
                "/norestart".to_string(),
                format!("INSTALLDIR={install_path_backslash}"),
                "ALLUSERS=2".to_string(),
                "MSIINSTALLPERUSER=1".to_string(),
            ],
            vec![
                "/a".to_string(),
                archive_path.display().to_string(),
                "/qn".to_string(),
                format!("TARGETDIR={install_path}"),
            ],
            vec![
                "/i".to_string(),
                archive_path.display().to_string(),
                "/qn".to_string(),
                "/norestart".to_string(),
            ],
        ];

        for (idx, args) in modes.into_iter().enumerate() {
            let mut command = Command::new("msiexec");
            command.args(&args);
            command.creation_flags(0x08000000);
            let output = command
                .output()
                .map_err(|e| EngineError::Install(format!("failed to spawn msiexec: {e}")))?;
            if output.status.success()
                && ensure_firefox_payload_available_in_target_dir_impl(target_dir).is_ok()
            {
                return Ok(());
            }
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("exit code {:?}", output.status.code())
            };
            errors.push(format!("mode{}={detail}", idx + 1));
        }

        if ensure_firefox_payload_available_in_target_dir_impl(target_dir).is_ok() {
            return Ok(());
        }
        let target_entries = list_target_dir_entries_impl(target_dir);
        Err(EngineError::Install(format!(
            "msiexec install failed: {}; target_entries={:?}",
            errors.join("; "),
            target_entries
        )))
    }
}

pub(super) fn extract_firefox_setup_exe_impl(
    archive_path: &Path,
    target_dir: &Path,
) -> Result<(), EngineError> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (archive_path, target_dir);
        return Err(EngineError::Install(
            "Firefox setup extraction is supported only on Windows".to_string(),
        ));
    }
    #[cfg(target_os = "windows")]
    {
        fs::create_dir_all(target_dir)?;
        let mut command = Command::new(archive_path);
        command
            .arg(format!("/ExtractDir={}", target_dir.display()))
            .arg("/S");
        command.creation_flags(0x08000000);
        let output = command
            .output()
            .map_err(|e| EngineError::Install(format!("failed to spawn Firefox setup EXE: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Err(EngineError::Install(format!(
                "firefox setup extract failed: {}",
                if !stderr.is_empty() {
                    stderr
                } else if !stdout.is_empty() {
                    stdout
                } else {
                    format!("exit code {:?}", output.status.code())
                }
            )));
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn ensure_firefox_payload_available_in_target_dir_impl(target_dir: &Path) -> Result<(), EngineError> {
    if find_first_match(target_dir, &candidate_names(&["firefox.exe"])).is_some() {
        return Ok(());
    }
    let source_candidates = discover_firefox_installation_candidates_impl();
    if let Some(source_firefox_exe) = source_candidates.into_iter().find(|path| path.is_file()) {
        let Some(source_root) = source_firefox_exe.parent() else {
            return Ok(());
        };
        let mirrored_root = target_dir.join("Mozilla Firefox");
        if mirrored_root.exists() {
            fs::remove_dir_all(&mirrored_root)?;
        }
        copy_dir_recursive_impl(source_root, &mirrored_root)?;
        return Ok(());
    }
    Err(EngineError::Install(format!(
        "firefox-esr install completed but firefox.exe was not found in target {} or standard install directories",
        target_dir.display()
    )))
}

#[cfg(target_os = "windows")]
fn discover_firefox_installation_candidates_impl() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        let root = PathBuf::from(program_files);
        candidates.push(root.join("Mozilla Firefox").join("firefox.exe"));
        candidates.push(root.join("Firefox ESR").join("firefox.exe"));
    }
    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        let root = PathBuf::from(program_files_x86);
        candidates.push(root.join("Mozilla Firefox").join("firefox.exe"));
        candidates.push(root.join("Firefox ESR").join("firefox.exe"));
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let root = PathBuf::from(local_app_data);
        candidates.push(root.join("Mozilla Firefox").join("firefox.exe"));
        candidates.push(root.join("Programs").join("Mozilla Firefox").join("firefox.exe"));
        candidates.push(root.join("Programs").join("Firefox ESR").join("firefox.exe"));
    }
    dedupe_existing_paths_impl(candidates)
}

#[cfg(target_os = "windows")]
fn dedupe_existing_paths_impl(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    let mut seen = BTreeSet::new();
    for path in paths {
        let key = path.to_string_lossy().to_ascii_lowercase();
        if seen.insert(key) {
            deduped.push(path);
        }
    }
    deduped
}

#[cfg(target_os = "windows")]
fn ensure_windows_path_with_trailing_backslash_impl(path: &Path) -> String {
    let mut value = path.to_string_lossy().to_string();
    if !value.ends_with('\\') {
        value.push('\\');
    }
    value
}

#[cfg(target_os = "windows")]
fn copy_dir_recursive_impl(source: &Path, destination: &Path) -> Result<(), EngineError> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive_impl(&source_path, &destination_path)?;
        } else {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn list_target_dir_entries_impl(target_dir: &Path) -> Vec<String> {
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(target_dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            let kind = if path.is_dir() { "dir" } else { "file" };
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("<non-utf8>");
            entries.push(format!("{kind}:{name}"));
        }
    }
    entries.sort();
    entries
}
