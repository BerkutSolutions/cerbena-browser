use super::*;

pub(crate) fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(600))
        .connect_timeout(Duration::from_secs(20))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("http client build: {e}"))
}

pub(crate) fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), String> {
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

pub(crate) fn verify_sha512(path: &Path, expected_hex: &str) -> Result<(), String> {
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

pub(crate) fn hash_file_sha256(path: &Path) -> Result<String, String> {
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

pub(crate) fn hash_file_sha512(path: &Path) -> Result<String, String> {
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

pub(crate) fn unzip_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
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

pub(crate) fn untar_gz_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
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

pub(crate) fn safe_archive_join(target_dir: &Path, relative: &Path) -> Result<PathBuf, String> {
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

pub(crate) fn extract_msi(msi_path: &Path, target_dir: &Path) -> Result<(), String> {
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

pub(crate) fn extract_checksum_value(checksum_file: &str, file_name: &str) -> Option<String> {
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

pub(crate) fn find_file_recursive(root: &Path, file_name: &str) -> Option<PathBuf> {
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

#[allow(dead_code)]
pub(crate) fn can_spawn(binary: &str, probe_arg: &str) -> bool {
    hidden_command(binary)
        .arg(probe_arg)
        .output()
        .map(|output| {
            output.status.success() || !output.stdout.is_empty() || !output.stderr.is_empty()
        })
        .unwrap_or(false)
}

#[allow(dead_code)]
pub(crate) fn find_path_binary(candidates: &[&str]) -> Option<PathBuf> {
    for candidate in candidates {
        if can_spawn(candidate, "--help") || can_spawn(candidate, "version") {
            return Some(PathBuf::from(candidate));
        }
    }
    None
}

pub(crate) fn percent(downloaded: u64, total: Option<u64>) -> f64 {
    let Some(total) = total else {
        return 0.0;
    };
    if total == 0 {
        return 0.0;
    }
    ((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
}

pub(crate) fn now_epoch_ms() -> u128 {
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
