use super::*;

pub(crate) fn chromium_snapshot_platform_dir_impl() -> Result<&'static str, EngineError> {
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

pub(crate) fn chromium_snapshot_archive_name_impl() -> Result<&'static str, EngineError> {
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

pub(crate) fn librewolf_asset_suffix_impl() -> Result<String, EngineError> {
    let (os, arch) = if cfg!(target_os = "windows") {
        ("win", if cfg!(target_arch = "x86_64") { "x86_64" } else { "arm64" })
    } else if cfg!(target_os = "linux") {
        ("lin", if cfg!(target_arch = "x86_64") { "x86_64" } else { "arm64" })
    } else if cfg!(target_os = "macos") {
        ("mac", if cfg!(target_arch = "x86_64") { "x86_64" } else { "arm64" })
    } else {
        return Err(EngineError::Install("unsupported operating system".to_string()));
    };
    Ok(format!("-{os}.{arch}.zip"))
}

pub(crate) fn librewolf_windows_portable_marker_impl() -> Result<String, EngineError> {
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

pub(crate) fn extract_librewolf_download_url_impl(html: &str, asset_marker: &str) -> Option<String> {
    extract_html_hrefs_impl(html)
        .into_iter()
        .find(|href| href.to_ascii_lowercase().contains(&asset_marker.to_ascii_lowercase()))
        .map(|href| normalize_href_url_impl(&href))
}

pub(crate) fn extract_html_hrefs_impl(html: &str) -> Vec<String> {
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

pub(crate) fn normalize_href_url_impl(href: &str) -> String {
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

pub(crate) fn parse_librewolf_version_from_file_name_impl(file_name: &str) -> Option<String> {
    let prefix = "librewolf-";
    let middle = "-windows-";
    let stripped = file_name.strip_prefix(prefix)?;
    let end = stripped.find(middle)?;
    let version = stripped[..end].trim();
    if version.is_empty() { None } else { Some(version.to_string()) }
}

pub(crate) fn file_name_from_url_impl(url: &str, fallback: &str) -> Result<String, EngineError> {
    let name = url
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| EngineError::Download(format!("unable to derive file name for {fallback}")))?;
    Ok(name.to_string())
}

pub(crate) fn parse_librewolf_version_from_appimage_file_name_impl(file_name: &str) -> Option<String> {
    let stripped = file_name.strip_prefix("LibreWolf-")?;
    let trimmed = stripped.strip_suffix(".AppImage")?;
    let mut parts = trimmed.split('-').collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }
    if let Some(last) = parts.last() {
        if last.eq_ignore_ascii_case("x86_64") || last.eq_ignore_ascii_case("aarch64") {
            parts.pop();
        }
    }
    let version = parts.join("-");
    if version.is_empty() { None } else { Some(version) }
}

pub(crate) fn find_first_suffix_match_impl(
    root: &Path,
    suffixes: &[&str],
    contains: Option<&str>,
) -> Option<PathBuf> {
    let suffixes = suffixes
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let needle = contains.map(|value| value.to_ascii_lowercase());
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
            if !suffixes.iter().any(|suffix| file_name.ends_with(suffix)) {
                continue;
            }
            if let Some(ref expected) = needle {
                if !file_name.contains(expected) {
                    continue;
                }
            }
            return Some(path);
        }
    }
    None
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_requires_no_sandbox_for_binary_impl(binary_path: &Path) -> bool {
    if std::env::var("CERBENA_FORCE_CHROMIUM_SANDBOX")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return false;
    }
    let userns_clone_enabled = match fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone") {
        Ok(value) => value.trim() == "1",
        Err(_) => false,
    };
    let apparmor_restricts_userns =
        match fs::read_to_string("/proc/sys/kernel/apparmor_restrict_unprivileged_userns") {
            Ok(value) => value.trim() == "1",
            Err(_) => false,
        };
    if !userns_clone_enabled {
        return true;
    }
    if !apparmor_restricts_userns {
        return false;
    }
    !linux_binary_allowlisted_in_cerbena_apparmor_impl(binary_path)
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_sandbox_probe_summary_impl() -> String {
    let userns = match fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone") {
        Ok(value) => value.trim().to_string(),
        Err(error) => format!("read_error:{error}"),
    };
    let apparmor = match fs::read_to_string("/proc/sys/kernel/apparmor_restrict_unprivileged_userns") {
        Ok(value) => value.trim().to_string(),
        Err(error) => format!("read_error:{error}"),
    };
    format!(
        "kernel.unprivileged_userns_clone={userns}, kernel.apparmor_restrict_unprivileged_userns={apparmor}"
    )
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_binary_allowlisted_in_cerbena_apparmor_impl(binary_path: &Path) -> bool {
    let profile = fs::read_to_string("/etc/apparmor.d/cerbena-chromium")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if profile.is_empty() || !profile.contains("userns") {
        return false;
    }
    let binary = binary_path.to_string_lossy().to_ascii_lowercase();
    if binary.contains("/.local/share/dev.cerbena.app/engine-runtime/engines/chromium/") {
        return profile.contains(".local/share/dev.cerbena.app/engine-runtime/engines/chromium/");
    }
    if binary.contains("/.local/share/cerbena.app/engine-runtime/engines/chromium/") {
        return profile.contains(".local/share/cerbena.app/engine-runtime/engines/chromium/");
    }
    false
}
