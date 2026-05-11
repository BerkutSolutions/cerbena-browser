use super::*;

pub(crate) fn ungoogled_chromium_releases_url_impl() -> Result<&'static str, EngineError> {
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

pub(crate) fn ungoogled_chromium_asset_suffixes_impl() -> Result<Vec<String>, EngineError> {
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

pub(crate) fn ungoogled_chromium_asset_candidates_impl(
    version: &str,
) -> Result<Vec<String>, EngineError> {
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

pub(crate) fn select_ungoogled_chromium_asset_impl(
    release: &GithubRelease,
) -> Result<Option<GithubAsset>, EngineError> {
    let candidates = ungoogled_chromium_asset_candidates_impl(&release.tag_name)?;
    if let Some(asset) = candidates.iter().find_map(|candidate| {
        release
            .assets
            .iter()
            .find(|item| item.name.eq_ignore_ascii_case(candidate))
            .cloned()
    }) {
        return Ok(Some(asset));
    }
    let suffixes = ungoogled_chromium_asset_suffixes_impl()?;
    Ok(release.assets.iter().find_map(|item| {
        let lower = item.name.to_ascii_lowercase();
        if lower.contains("ungoogled") && suffixes.iter().any(|suffix| lower.ends_with(suffix)) {
            Some(item.clone())
        } else {
            None
        }
    }))
}
