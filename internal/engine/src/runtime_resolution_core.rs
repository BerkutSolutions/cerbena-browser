use std::path::{Path, PathBuf};

use crate::contract::{EngineError, EngineKind};

use super::{
    http_client, runtime_binary, runtime_compat, select_ungoogled_chromium_asset,
    ungoogled_chromium_asset_suffixes, ungoogled_chromium_releases_url, GithubRelease,
    ResolvedArtifact, CHROMIUM_SNAPSHOTS_BASE_URL, FIREFOX_RELEASES_BASE_URL, FIREFOX_VERSIONS_URL,
    LIBREWOLF_LINUX_INSTALLATION_URL, LIBREWOLF_LINUX_MIRROR_RELEASES_URL, LIBREWOLF_RELEASES_URL,
    LIBREWOLF_WINDOWS_INSTALLATION_URL,
};

pub(super) fn resolve_artifact(engine: EngineKind) -> Result<ResolvedArtifact, EngineError> {
    match engine {
        EngineKind::Chromium => resolve_chromium_artifact(),
        EngineKind::UngoogledChromium => resolve_ungoogled_chromium_artifact(),
        EngineKind::FirefoxEsr => resolve_firefox_esr_artifact(),
        EngineKind::Librewolf => resolve_librewolf_artifact(),
    }
}

pub(super) fn installation_target_dir(
    install_root: &Path,
    engine: EngineKind,
    version: &str,
) -> PathBuf {
    install_root.join(engine.as_key()).join(version)
}

pub(super) fn locate_binary(engine: EngineKind, root: &Path) -> Result<PathBuf, EngineError> {
    let candidates = match engine {
        EngineKind::Chromium => runtime_binary::candidate_names_impl(&[
            "chrome.exe",
            "chrome",
            "chromium-browser.exe",
            "chromium.exe",
            "chromium",
        ]),
        EngineKind::UngoogledChromium => runtime_binary::candidate_names_impl(&[
            "chrome.exe",
            "chrome",
            "ungoogled-chromium.exe",
            "ungoogled-chromium",
            "chromium-browser.exe",
            "chromium.exe",
            "chromium",
        ]),
        EngineKind::FirefoxEsr => {
            runtime_binary::candidate_names_impl(&["firefox.exe", "firefox", "librewolf.exe", "librewolf"])
        }
        EngineKind::Librewolf => {
            runtime_binary::candidate_names_impl(&["librewolf.exe", "librewolf", "firefox.exe", "firefox"])
        }
    };
    let binary = if matches!(engine, EngineKind::Librewolf | EngineKind::FirefoxEsr) {
        runtime_binary::find_first_match_impl(root, &candidates)
            .or_else(|| runtime_compat::find_first_suffix_match_impl(root, &[".appimage"], Some("librewolf")))
            .or_else(|| runtime_compat::find_first_suffix_match_impl(root, &[".appimage"], Some("firefox")))
    } else {
        runtime_binary::find_first_match_impl(root, &candidates)
    }
    .ok_or_else(|| {
        EngineError::Install(format!(
            "unable to locate {} executable under {}",
            engine.as_key(),
            root.display()
        ))
    })?;
    Ok(binary)
}

fn resolve_firefox_esr_artifact() -> Result<ResolvedArtifact, EngineError> {
    let client = http_client()?;
    let response = client
        .get(FIREFOX_VERSIONS_URL)
        .send()
        .map_err(|e| EngineError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(EngineError::Download(format!(
            "firefox versions request failed with HTTP {}",
            response.status()
        )));
    }
    let payload: serde_json::Value = response
        .json()
        .map_err(|e| EngineError::Download(e.to_string()))?;
    let version = payload
        .get("FIREFOX_ESR")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| EngineError::Download("FIREFOX_ESR version is missing".to_string()))?
        .to_string();
    let (download_url, file_name) = if cfg!(target_os = "windows") {
        (
            format!(
                "{}/{}/win64/en-US/Firefox%20Setup%20{}.msi",
                FIREFOX_RELEASES_BASE_URL, version, version
            ),
            format!("Firefox-Setup-{version}.msi"),
        )
    } else if cfg!(target_os = "linux") {
        (
            format!(
                "{}/{}/linux-x86_64/en-US/firefox-{}.tar.bz2",
                FIREFOX_RELEASES_BASE_URL, version, version
            ),
            format!("firefox-{version}.tar.bz2"),
        )
    } else if cfg!(target_os = "macos") {
        (
            format!(
                "{}/{}/mac/en-US/Firefox%20{}.dmg",
                FIREFOX_RELEASES_BASE_URL, version, version
            ),
            format!("Firefox-{version}.dmg"),
        )
    } else {
        return Err(EngineError::Download(
            "firefox-esr is not supported on this platform".to_string(),
        ));
    };
    Ok(ResolvedArtifact {
        engine: EngineKind::FirefoxEsr,
        version,
        download_url,
        file_name,
    })
}

fn resolve_chromium_artifact() -> Result<ResolvedArtifact, EngineError> {
    let client = http_client()?;
    let platform_dir = runtime_compat::chromium_snapshot_platform_dir_impl()?;
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
    let archive_name = runtime_compat::chromium_snapshot_archive_name_impl()?;
    let download_url = format!("{CHROMIUM_SNAPSHOTS_BASE_URL}/{platform_dir}/{revision}/{archive_name}");
    Ok(ResolvedArtifact {
        engine: EngineKind::Chromium,
        version: revision,
        file_name: archive_name.to_string(),
        download_url,
    })
}

fn resolve_librewolf_artifact() -> Result<ResolvedArtifact, EngineError> {
    if cfg!(target_os = "windows") {
        return resolve_librewolf_windows_artifact();
    }
    if cfg!(target_os = "linux") {
        return resolve_librewolf_linux_artifact();
    }

    let client = http_client()?;
    let suffix = runtime_compat::librewolf_asset_suffix_impl()?;
    let release_urls = [LIBREWOLF_RELEASES_URL, LIBREWOLF_LINUX_MIRROR_RELEASES_URL];
    let mut errors = Vec::new();
    for release_url in release_urls {
        let response = match client.get(release_url).send() {
            Ok(response) => response,
            Err(error) => {
                errors.push(format!("{release_url}: {error}"));
                continue;
            }
        };
        if !response.status().is_success() {
            errors.push(format!("{release_url}: HTTP {}", response.status()));
            continue;
        }
        let releases: Vec<GithubRelease> = match response.json() {
            Ok(releases) => releases,
            Err(error) => {
                errors.push(format!("{release_url}: invalid JSON ({error})"));
                continue;
            }
        };
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
    }
    Err(EngineError::Download(format!(
        "no compatible LibreWolf asset found for suffix {suffix}; sources: {}",
        errors.join("; ")
    )))
}

fn resolve_librewolf_linux_artifact() -> Result<ResolvedArtifact, EngineError> {
    let client = http_client()?;
    let response = client
        .get(LIBREWOLF_LINUX_INSTALLATION_URL)
        .send()
        .map_err(|e| EngineError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(EngineError::Download(format!(
            "librewolf linux install page request failed with HTTP {}",
            response.status()
        )));
    }
    let body = response
        .text()
        .map_err(|e| EngineError::Download(e.to_string()))?;
    let arch_marker = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(EngineError::Download(
            "unsupported Linux architecture for LibreWolf".to_string(),
        ));
    };
    let href = runtime_compat::extract_html_hrefs_impl(&body)
        .into_iter()
        .find(|candidate| {
            let lower = candidate.to_ascii_lowercase();
            lower.contains("librewolf")
                && lower.contains(&arch_marker.to_ascii_lowercase())
                && lower.ends_with(".appimage")
        })
        .ok_or_else(|| {
            EngineError::Download(format!(
                "no compatible LibreWolf AppImage link found on {} for architecture {}",
                LIBREWOLF_LINUX_INSTALLATION_URL, arch_marker
            ))
        })?;
    let download_url = runtime_compat::normalize_href_url_impl(&href);
    let file_name = runtime_compat::file_name_from_url_impl(&download_url, "librewolf appimage")?;
    let version = runtime_compat::parse_librewolf_version_from_appimage_file_name_impl(&file_name)
        .unwrap_or_else(|| "linux-appimage".to_string());
    Ok(ResolvedArtifact {
        engine: EngineKind::Librewolf,
        version,
        file_name,
        download_url,
    })
}

fn resolve_ungoogled_chromium_artifact() -> Result<ResolvedArtifact, EngineError> {
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

fn resolve_librewolf_windows_artifact() -> Result<ResolvedArtifact, EngineError> {
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
    let asset_marker = runtime_compat::librewolf_windows_portable_marker_impl()?;
    let download_url = runtime_compat::extract_librewolf_download_url_impl(&body, &asset_marker)
        .ok_or_else(|| {
            EngineError::Download(format!(
                "no compatible LibreWolf Windows download link found for marker {asset_marker}"
            ))
        })?;
    let file_name = runtime_compat::file_name_from_url_impl(&download_url, "librewolf")?;
    let version = runtime_compat::parse_librewolf_version_from_file_name_impl(&file_name).ok_or_else(|| {
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
