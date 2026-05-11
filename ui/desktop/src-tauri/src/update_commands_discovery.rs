use super::*;

pub(crate) fn fetch_latest_release_impl(client: &Client) -> Result<ReleaseCandidate, String> {
    let latest_release_url = resolve_latest_release_api_url();
    fetch_latest_release_from_url_impl(client, &latest_release_url)
}

pub(crate) fn fetch_latest_release_from_url_impl(
    client: &Client,
    latest_release_url: &str,
) -> Result<ReleaseCandidate, String> {
    let response = client
        .get(latest_release_url)
        .send()
        .map_err(|e| format!("request latest release: {e}"))?;
    if !response.status().is_success() {
        return Err(describe_http_failure(
            response,
            "latest release request",
            Some(latest_release_url),
        ));
    }
    let release: GithubRelease = response
        .json()
        .map_err(|e| format!("parse latest release payload: {e}"))?;
    let version = normalize_version(&release.tag_name);
    let asset = pick_release_asset_impl(&release.assets);
    let checksums = release
        .assets
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case(RELEASE_CHECKSUMS_ASSET));
    let checksums_signature = release.assets.iter().find(|item| {
        item.name
            .eq_ignore_ascii_case(RELEASE_CHECKSUMS_SIGNATURE_ASSET)
    });
    Ok(ReleaseCandidate {
        version,
        release_url: release.html_url,
        asset_name: asset.map(|item| item.asset.name.clone()),
        asset_url: asset.map(|item| item.asset.browser_download_url.clone()),
        asset_type: asset.map(|item| item.kind.label().to_string()),
        asset_selection_reason: asset.map(|item| item.reason.to_string()),
        install_handoff_mode: asset.map(|item| item.kind.handoff_mode().to_string()),
        checksums_url: checksums.map(|item| item.browser_download_url.clone()),
        checksums_signature_url: checksums_signature.map(|item| item.browser_download_url.clone()),
    })
}

pub(crate) fn pick_release_asset_impl(
    assets: &[GithubReleaseAsset],
) -> Option<SelectedReleaseAsset<'_>> {
    let os = std::env::consts::OS;
    let install_mode = if os == "windows" {
        Some(current_windows_install_mode())
    } else {
        None
    };
    pick_release_asset_for_context_impl(assets, os, install_mode.as_deref())
}

pub(crate) fn pick_release_asset_for_context_impl<'a>(
    assets: &'a [GithubReleaseAsset],
    os: &str,
    windows_install_mode: Option<&str>,
) -> Option<SelectedReleaseAsset<'a>> {
    let candidates = assets
        .iter()
        .filter_map(|asset| classify_release_asset_impl(os, asset))
        .collect::<Vec<_>>();

    if os == "windows" {
        let prefer_msi = windows_install_mode.unwrap_or("portable_zip") != "portable_zip";
        if prefer_msi {
            if let Some(asset) = candidates
                .iter()
                .copied()
                .find(|item| item.kind == SelectedAssetKind::WindowsMsi)
            {
                return Some(SelectedReleaseAsset {
                    reason: "windows_installed_context_prefers_msi",
                    ..asset
                });
            }
        }
        if let Some(asset) = candidates
            .iter()
            .copied()
            .find(|item| item.kind == SelectedAssetKind::WindowsZip)
        {
            return Some(SelectedReleaseAsset {
                reason: if prefer_msi {
                    "windows_portable_zip_fallback"
                } else {
                    "windows_portable_zip_primary"
                },
                ..asset
            });
        }
        if let Some(asset) = candidates
            .iter()
            .copied()
            .find(|item| item.kind == SelectedAssetKind::WindowsMsi)
        {
            return Some(SelectedReleaseAsset {
                reason: "windows_msi_fallback_when_zip_missing",
                ..asset
            });
        }
        if let Some(asset) = candidates
            .iter()
            .copied()
            .find(|item| item.kind == SelectedAssetKind::WindowsExe)
        {
            return Some(SelectedReleaseAsset {
                reason: "windows_manual_installer_fallback",
                ..asset
            });
        }
        return None;
    }

    candidates
        .into_iter()
        .min_by_key(|item| asset_rank(item.kind))
        .map(|item| SelectedReleaseAsset {
            reason: match item.kind {
                SelectedAssetKind::LinuxTarGz => "linux_tar_gz_primary",
                SelectedAssetKind::LinuxZip => "linux_zip_fallback",
                SelectedAssetKind::MacZip => "mac_zip_primary",
                _ => "generic_asset_selection",
            },
            ..item
        })
}

pub(crate) fn classify_release_asset_impl<'a>(
    os: &str,
    asset: &'a GithubReleaseAsset,
) -> Option<SelectedReleaseAsset<'a>> {
    let name = asset.name.to_ascii_lowercase();
    match os {
        "windows" if name.ends_with(".msi") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::WindowsMsi,
            reason: "windows_msi_candidate",
        }),
        "windows" if name.ends_with(".zip") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::WindowsZip,
            reason: "windows_zip_candidate",
        }),
        "windows" if name.ends_with(".exe") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::WindowsExe,
            reason: "windows_exe_candidate",
        }),
        "linux" if name.ends_with(".tar.gz") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::LinuxTarGz,
            reason: "linux_tar_gz_candidate",
        }),
        "linux" if name.ends_with(".zip") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::LinuxZip,
            reason: "linux_zip_candidate",
        }),
        "macos" if name.ends_with(".zip") => Some(SelectedReleaseAsset {
            asset,
            kind: SelectedAssetKind::MacZip,
            reason: "mac_zip_candidate",
        }),
        _ => None,
    }
}
