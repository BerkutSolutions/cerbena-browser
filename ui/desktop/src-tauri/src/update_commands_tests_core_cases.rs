use super::*;

#[test]
fn version_normalization_drops_leading_v() {
    assert_eq!(normalize_version("v1.2.3"), "1.2.3");
    assert_eq!(normalize_version("1.2.3"), "1.2.3");
}

#[test]
fn newer_version_detection_uses_semver_like_order() {
    assert!(is_version_newer("1.2.4", "1.2.3"));
    assert!(is_version_newer("2.0.0", "1.9.9"));
    assert!(is_version_newer("1.0.4-1", "1.0.4"));
    assert!(!is_version_newer("1.2.3", "1.2.3"));
    assert!(!is_version_newer("1.1.9", "1.2.3"));
    assert!(!is_version_newer("1.0.4-preview", "1.0.4"));
}

#[test]
fn next_release_version_advances_hotfix_versions() {
    let next = next_release_version("1.0.12-1");
    assert_eq!(next, "1.0.12-2");
    assert!(is_version_newer(&next, "1.0.12-1"));
}

#[test]
fn auto_apply_support_is_limited_to_safe_asset_types() {
    assert!(can_auto_apply_asset("cerbena-windows.zip"));
    assert!(can_auto_apply_asset("cerbena-windows.msi"));
    assert!(!can_auto_apply_asset("cerbena-windows.exe"));
    assert!(!can_auto_apply_asset_for_os(
        "linux",
        "cerbena-browser-linux.zip"
    ));
    assert!(!can_auto_apply_asset_for_os(
        "linux",
        "cerbena-browser_9.9.9_amd64.deb"
    ));
}

#[test]
fn preferred_asset_order_keeps_zip_before_other_formats() {
    assert!(
        asset_rank(SelectedAssetKind::WindowsMsi) < asset_rank(SelectedAssetKind::WindowsZip)
    );
    assert!(
        asset_rank(SelectedAssetKind::WindowsZip) < asset_rank(SelectedAssetKind::WindowsExe)
    );
}

#[test]
fn release_asset_picker_prefers_best_match_for_platform() {
    let assets = vec![
        GithubReleaseAsset {
            name: "cerbena-windows.msi".to_string(),
            browser_download_url: "https://example.invalid/1".to_string(),
        },
        GithubReleaseAsset {
            name: "cerbena-windows.zip".to_string(),
            browser_download_url: "https://example.invalid/2".to_string(),
        },
    ];
    let selected = pick_release_asset_for_context(&assets, "windows", Some("msi"))
        .expect("selected installed asset");
    assert_eq!(selected.asset.name, "cerbena-windows.msi");
    assert_eq!(selected.kind, SelectedAssetKind::WindowsMsi);
    assert_eq!(selected.reason, "windows_installed_context_prefers_msi");
}

#[test]
fn release_asset_picker_prefers_zip_for_portable_windows_context() {
    let assets = vec![
        GithubReleaseAsset {
            name: "cerbena-windows.msi".to_string(),
            browser_download_url: "https://example.invalid/1".to_string(),
        },
        GithubReleaseAsset {
            name: "cerbena-windows.zip".to_string(),
            browser_download_url: "https://example.invalid/2".to_string(),
        },
    ];
    let selected = pick_release_asset_for_context(&assets, "windows", Some("portable_zip"))
        .expect("selected portable asset");
    assert_eq!(selected.asset.name, "cerbena-windows.zip");
    assert_eq!(selected.kind, SelectedAssetKind::WindowsZip);
    assert_eq!(selected.reason, "windows_portable_zip_primary");
}

#[test]
fn release_asset_picker_falls_back_to_msi_when_zip_is_missing() {
    let assets = vec![GithubReleaseAsset {
        name: "cerbena-windows.msi".to_string(),
        browser_download_url: "https://example.invalid/1".to_string(),
    }];
    let selected = pick_release_asset_for_context(&assets, "windows", Some("portable_zip"))
        .expect("selected fallback asset");
    assert_eq!(selected.asset.name, "cerbena-windows.msi");
    assert_eq!(selected.kind, SelectedAssetKind::WindowsMsi);
    assert_eq!(selected.reason, "windows_msi_fallback_when_zip_missing");
}

#[test]
fn checksum_extraction_matches_plain_and_nested_asset_paths() {
    let checksums = "\
abc123  cerbena-windows-x64.zip\n\
def456  cerbena-windows-x64/cerbena.exe\n";
    assert_eq!(
        extract_checksum_for_asset(checksums, "cerbena-windows-x64.zip"),
        Some("abc123")
    );
    assert_eq!(
        extract_checksum_for_asset(checksums, "cerbena.exe"),
        Some("def456")
    );
}

#[test]
fn sha256_hex_matches_known_digest() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn signature_verification_variants_add_newline_fallbacks_once() {
    let variants = signature_verification_variants(b"alpha\r\nbeta\r\n");
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0], b"alpha\r\nbeta\r\n");
    assert_eq!(variants[1], b"alpha\nbeta\n");
}

#[test]
fn release_signing_public_keys_include_current_and_legacy_keys() {
    let keys = release_signing_public_keys();
    assert!(keys.len() >= 2);
    assert!(keys
        .iter()
        .any(|key| key.contains("1nCCvDQ4TOZjV1t78V3T3dIz")));
    assert!(keys
        .iter()
        .any(|key| key.contains("sQ/dGNzpHEHiSUvpp8+h4axI")));
}

#[test]
fn auto_update_scheduler_runs_without_prior_check_when_enabled() {
    let store = AppUpdateStore {
        auto_update_enabled: true,
        ..AppUpdateStore::default()
    };
    assert!(should_run_auto_update_check(&store));
}

#[test]
fn missing_auto_update_field_defaults_to_enabled() {
    let store: AppUpdateStore = serde_json::from_str("{}").expect("deserialize update store");
    assert_eq!(store.auto_update_enabled, default_auto_update_enabled());
}

#[test]
fn powershell_command_reads_checksum_payloads_from_environment() {
    let script = format!(
        "$a=[Environment]::GetEnvironmentVariable('{checksums}'); \
         $b=[Environment]::GetEnvironmentVariable('{signature}'); \
         if ([string]::IsNullOrWhiteSpace($a) -or [string]::IsNullOrWhiteSpace($b)) {{ exit 2 }}; \
         if ($a -eq 'alpha' -and $b -eq 'beta') {{ exit 0 }}; \
         exit 1",
        checksums = RELEASE_CHECKSUMS_B64_ENV,
        signature = RELEASE_CHECKSUMS_SIGNATURE_B64_ENV
    );
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .env(RELEASE_CHECKSUMS_B64_ENV, "alpha")
        .env(RELEASE_CHECKSUMS_SIGNATURE_B64_ENV, "beta")
        .output()
        .expect("run powershell env transport test");
    assert!(
        output.status.success(),
        "powershell env transport must succeed: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reconcile_update_store_clears_staged_update_once_current_version_is_installed() {
    let mut store = AppUpdateStore {
        latest_version: Some("1.0.6-1".to_string()),
        staged_version: Some(CURRENT_VERSION.to_string()),
        staged_asset_name: Some("cerbena-windows-x64.zip".to_string()),
        staged_asset_path: Some("C:/tmp/update.zip".to_string()),
        pending_apply_on_exit: true,
        updater_handoff_version: Some(CURRENT_VERSION.to_string()),
        status: "applying".to_string(),
        ..AppUpdateStore::default()
    };
    reconcile_update_store_with_current_version(&mut store);
    assert_eq!(store.staged_version, None);
    assert_eq!(store.staged_asset_name, None);
    assert_eq!(store.staged_asset_path, None);
    assert!(!store.pending_apply_on_exit);
    assert_eq!(store.status, "up_to_date");
    assert_eq!(store.latest_version.as_deref(), Some(CURRENT_VERSION));
}

#[test]
fn reconcile_update_store_clears_stale_handoff_state_after_successful_relaunch() {
    let mut store = AppUpdateStore {
        latest_version: Some("1.0.6-1".to_string()),
        staged_asset_name: Some("cerbena-browser-1.2.3.msi".to_string()),
        staged_asset_path: Some("C:/tmp/update.msi".to_string()),
        selected_asset_type: Some("msi".to_string()),
        selected_asset_reason: Some("windows_installed_context_prefers_msi".to_string()),
        install_handoff_mode: Some("direct_msi".to_string()),
        updater_handoff_version: Some(CURRENT_VERSION.to_string()),
        status: "applied_pending_relaunch".to_string(),
        ..AppUpdateStore::default()
    };
    reconcile_update_store_with_current_version(&mut store);
    assert_eq!(store.staged_asset_name, None);
    assert_eq!(store.staged_asset_path, None);
    assert_eq!(store.selected_asset_type, None);
    assert_eq!(store.selected_asset_reason, None);
    assert_eq!(store.install_handoff_mode, None);
    assert_eq!(store.updater_handoff_version, None);
    assert_eq!(store.status, "up_to_date");
}

#[test]
fn relaunch_executable_prefers_cerbena_binary() {
    let temp = tempfile::tempdir().expect("tempdir");
    let cerbena = temp.path().join("cerbena.exe");
    std::fs::write(&cerbena, b"stub").expect("write cerbena");
    let legacy = temp.path().join("browser-desktop-ui.exe");
    std::fs::write(&legacy, b"stub").expect("write legacy");
    assert_eq!(
        resolve_relaunch_executable_path(temp.path()),
        Some(PathBuf::from(cerbena))
    );
}

#[test]
fn zip_apply_helper_stops_existing_launcher_processes_before_copy() {
    let script = build_zip_apply_helper_script(
        4242,
        Path::new("C:/tmp/update.zip"),
        Path::new("C:/Program Files/Cerbena Browser"),
        Some(Path::new("C:/Program Files/Cerbena Browser/cerbena.exe")),
        Some("C:/tmp/runtime_logs.log"),
    );
    assert!(script.contains("Stop-Process -Id $proc.Id -Force"));
    assert!(script.contains("WaitForExit(15000)"));
    assert!(script.contains("@('cerbena.exe','browser-desktop-ui.exe','cerbena-updater.exe')"));
    assert!(script
        .contains("for ($attempt=0; $attempt -lt 10 -and -not $copySucceeded; $attempt++)"));
    assert!(script.contains("CERBENA_UPDATER_AUTO_EXIT_AFTER_SECONDS"));
    assert!(script.contains("[updater-helper][zip]"));
    assert!(script.contains("CERBENA_UPDATER_RUNTIME_LOG"));
}

#[test]
fn msi_apply_helper_uses_quiet_msiexec_and_updates_store() {
    let script = build_msi_apply_helper_script(
        4242,
        Path::new("C:/tmp/update.msi"),
        Some(Path::new("C:/Users/test/AppData/Local/Cerbena Browser")),
        Some("C:/tmp/app_update_store.json"),
        Some("1.2.3"),
        Some("C:/tmp/runtime_logs.log"),
    );
    assert!(script.contains("Start-Process -FilePath 'msiexec.exe'"));
    assert!(script.contains("'/qn'"));
    assert!(script.contains("'/l*v'"));
    assert!(script.contains("@('cerbena.exe','browser-desktop-ui.exe','cerbena-updater.exe','cerbena-launcher.exe')"));
    assert!(script.contains("Stop-Process -Id $proc.Id -Force"));
    assert!(script.contains("WaitForExit(15000)"));
    assert!(script.contains("while (-not $proc.WaitForExit(1000))"));
    assert!(script.contains("$elapsedMs -ge $msiWaitTimeoutMs"));
    assert!(script.contains("msiexec timed out"));
    assert!(script.contains("taskkill.exe"));
    assert!(script.contains("Resolve-RelaunchExecutable"));
    assert!(script.contains("INSTALLDIR="));
    assert!(script.contains("Update-Store 'applied_pending_relaunch'"));
    assert!(script.contains("pendingApplyOnExit"));
    assert!(script.contains("Update-Store 'canceled'"));
    assert!(script.contains("1602"));
    assert!(script.contains("1618"));
    assert!(script.contains("another Windows Installer transaction is already running (1618)"));
    assert!(script.contains("verbose log"));
    assert!(script.contains("[updater-helper][msi]"));
    assert!(script.contains("CERBENA_UPDATER_MSI_INSTALL_DIR"));
    assert!(script.contains("CERBENA_UPDATER_MSI_TIMEOUT_MS"));
    assert!(script.contains("CERBENA_UPDATER_RUNTIME_LOG"));
}

#[test]
fn latest_release_api_url_prefers_env_override() {
    let key = RELEASE_LATEST_API_URL_ENV;
    let previous = std::env::var(key).ok();
    std::env::set_var(key, "http://127.0.0.1:9191/latest");
    assert_eq!(
        resolve_latest_release_api_url(),
        "http://127.0.0.1:9191/latest"
    );
    if let Some(value) = previous {
        std::env::set_var(key, value);
    } else {
        std::env::remove_var(key);
    }
}

#[test]
fn updater_launch_mode_detects_preview_flag() {
    assert!(matches!(
        UpdaterLaunchMode::from_args(["--updater-preview"]),
        UpdaterLaunchMode::Preview
    ));
}

#[test]
fn updater_launch_mode_detects_auto_flag() {
    assert!(matches!(
        UpdaterLaunchMode::from_args(["--updater"]),
        UpdaterLaunchMode::Auto
    ));
}

#[test]
fn auto_launch_mode_triggers_close_after_ready_to_restart() {
    assert!(should_auto_close_updater_after_ready_to_restart(
        UpdaterLaunchMode::Auto
    ));
    assert!(!should_auto_close_updater_after_ready_to_restart(
        UpdaterLaunchMode::Preview
    ));
    assert!(!should_auto_close_updater_after_ready_to_restart(
        UpdaterLaunchMode::Disabled
    ));
}

#[test]
fn trusted_updater_downloads_mocked_newer_release_asset() {
    let asset_name = "cerbena-windows-x64.zip";
    let asset_bytes = b"trusted-update-asset".to_vec();
    let checksum = sha256_hex(&asset_bytes);
    let next_version = next_release_version(CURRENT_VERSION);
    let checksums_text = format!("{checksum}  {asset_name}\n");
    let base = spawn_http_server(vec![(
        format!("/{asset_name}"),
        asset_bytes.clone(),
        "application/octet-stream",
        Vec::new(),
    )]);
    let release_payload = format!(
        r#"{{
            "tag_name":"v{version}",
            "html_url":"https://example.invalid/releases/v{version}",
            "assets":[
                {{"name":"checksums.txt","browser_download_url":"{base}/checksums.txt"}},
                {{"name":"checksums.sig","browser_download_url":"{base}/checksums.sig"}},
                {{"name":"{asset_name}","browser_download_url":"{base}/{asset_name}"}}
            ]
        }}"#,
        version = next_version,
        asset_name = asset_name,
        base = base
    );
    let api_base = spawn_http_server(vec![(
        "/latest".to_string(),
        release_payload.into_bytes(),
        "application/json",
        Vec::new(),
    )]);
    let client = build_release_http_client(Duration::from_secs(5), false)
        .expect("build discovery client");
    let candidate = fetch_latest_release_from_url(&client, &format!("{api_base}/latest"))
        .expect("discover mocked release");
    assert!(is_version_newer(&candidate.version, CURRENT_VERSION));
    assert_eq!(candidate.asset_name.as_deref(), Some(asset_name));
    let download_client =
        build_release_http_client(Duration::from_secs(5), true).expect("build download client");
    let downloaded = download_release_bytes(
        &download_client,
        candidate.asset_url.as_deref().expect("asset url"),
        "release asset",
    )
    .expect("download mocked asset");
    let security_bundle = VerifiedReleaseSecurityBundle { checksums_text };
    ensure_asset_matches_verified_checksum(&security_bundle, asset_name, &downloaded)
        .expect("verify checksum");
    assert_eq!(downloaded, asset_bytes);
}

#[test]
fn trusted_updater_download_tolerates_bad_content_encoding_headers() {
    let payload = b"plain-binary-payload".to_vec();
    let base = spawn_http_server(vec![(
        "/asset.zip".to_string(),
        payload.clone(),
        "application/octet-stream",
        vec![("Content-Encoding", "gzip")],
    )]);
    let client = build_release_http_client(Duration::from_secs(5), true)
        .expect("build raw download client");
    let downloaded =
        download_release_bytes(&client, &format!("{base}/asset.zip"), "release asset")
            .expect("download payload with broken content encoding header");
    assert_eq!(downloaded, payload);
}
