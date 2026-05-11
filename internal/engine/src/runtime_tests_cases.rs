use super::*;
use std::fs;

#[test]
fn prepares_chromium_blocking_extension_from_blocked_domains() {
    let temp = tempfile::tempdir().expect("tempdir");
    let policy_dir = temp.path().join("policy");
    fs::create_dir_all(&policy_dir).expect("policy dir");
    fs::write(
        policy_dir.join("blocked-domains.json"),
        serde_json::to_vec(&vec![
            "youtube.com".to_string(),
            ".example.com".to_string(),
            "youtube.com".to_string(),
        ])
        .expect("serialize blocked domains"),
    )
    .expect("write blocked domains");

    let extension_dir = prepare_chromium_blocking_extension(temp.path())
        .expect("prepare extension")
        .expect("extension dir");

    let manifest_raw =
        fs::read_to_string(extension_dir.join("manifest.json")).expect("manifest");
    let rules_raw = fs::read_to_string(extension_dir.join("rules.json")).expect("rules");
    assert!(manifest_raw.contains("\"manifest_version\": 3"));
    assert!(manifest_raw.contains(&format!(
        "\"version\": \"{}\"",
        chromium_extension_version(CHROMIUM_POLICY_EXTENSION_VERSION)
    )));
    assert!(rules_raw.contains("||youtube.com^"));
    assert!(rules_raw.contains("||example.com^"));
}

#[test]
fn chromium_extension_version_normalizes_hotfix_suffixes() {
    assert_eq!(chromium_extension_version("1.2.3"), "1.2.3");
    assert_eq!(chromium_extension_version("v1.2.3"), "1.2.3");
    assert_eq!(chromium_extension_version("7"), "7");
}

#[test]
fn normalizes_blocked_domains_for_chromium_extension() {
    let temp = tempfile::tempdir().expect("tempdir");
    let policy_dir = temp.path().join("policy");
    fs::create_dir_all(&policy_dir).expect("policy dir");
    fs::write(
        policy_dir.join("blocked-domains.json"),
        serde_json::to_vec(&vec![
            " Reddit.com ".to_string(),
            ".reddit.com".to_string(),
            "".to_string(),
        ])
        .expect("serialize blocked domains"),
    )
    .expect("write blocked domains");

    let domains = blocked_domains_for_profile(temp.path()).expect("domains");
    assert_eq!(domains, vec!["reddit.com".to_string()]);
}

#[test]
fn prepares_locked_app_block_rule_for_chromium_extension() {
    let temp = tempfile::tempdir().expect("tempdir");
    let policy_dir = temp.path().join("policy");
    fs::create_dir_all(&policy_dir).expect("policy dir");
    fs::write(
        policy_dir.join("locked-app.json"),
        serde_json::to_vec(&serde_json::json!({
            "startUrl": "https://discord.com/app",
            "allowedHosts": ["discord.com", "discord.gg"]
        }))
        .expect("serialize locked app"),
    )
    .expect("write locked app");

    let extension_dir = prepare_chromium_blocking_extension(temp.path())
        .expect("prepare extension")
        .expect("extension dir");
    let rules_raw = fs::read_to_string(extension_dir.join("rules.json")).expect("rules");
    assert!(rules_raw.contains("\"regexFilter\": \"^https?://\""));
    assert!(rules_raw.contains("discord.com"));
    assert!(rules_raw.contains("excludedRequestDomains"));
}

#[test]
fn prepares_accept_language_rule_for_chromium_extension() {
    let temp = tempfile::tempdir().expect("tempdir");
    let policy_dir = temp.path().join("policy");
    fs::create_dir_all(&policy_dir).expect("policy dir");
    fs::write(
        policy_dir.join("identity-preset.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "locale": {
                "navigator_language": "ru",
                "languages": ["ru", "en", "en-US"]
            }
        }))
        .expect("serialize identity policy"),
    )
    .expect("write identity policy");

    let extension_dir = prepare_chromium_blocking_extension(temp.path())
        .expect("prepare extension")
        .expect("extension dir");
    let manifest_raw =
        fs::read_to_string(extension_dir.join("manifest.json")).expect("manifest");
    let rules_raw = fs::read_to_string(extension_dir.join("rules.json")).expect("rules");
    assert!(manifest_raw.contains("declarativeNetRequestWithHostAccess"));
    assert!(manifest_raw.contains("\"host_permissions\": ["));
    assert!(rules_raw.contains("\"type\": \"modifyHeaders\""));
    assert!(rules_raw.contains("\"header\": \"Accept-Language\""));
    assert!(rules_raw.contains("\"value\": \"ru,en;q=0.9,en-US;q=0.8\""));
}

#[test]
fn accept_language_header_uses_browser_like_quality_weights() {
    assert_eq!(
        build_accept_language_header(&[
            "ru-RU".to_string(),
            "ru".to_string(),
            "en-US".to_string(),
            "en".to_string(),
        ]),
        "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7"
    );
}

#[test]
fn chromium_launch_args_skip_accept_terms_flag() {
    let temp = tempfile::tempdir().expect("tempdir");
    let args = launch_args(
        EngineKind::Chromium,
        temp.path(),
        Some("https://duckduckgo.com"),
        false,
        None,
        false,
    )
    .expect("launch args");

    assert!(!args
        .iter()
        .any(|value| value == "--accept-terms-and-conditions"));
    assert!(!args.iter().any(|value| value == "--enable-logging"));
    assert!(!args.iter().any(|value| value.contains("--log-file=")));
}

#[test]
fn ungoogled_chromium_launch_args_reuse_chromium_family_behavior() {
    let temp = tempfile::tempdir().expect("tempdir");
    let args = launch_args(
        EngineKind::UngoogledChromium,
        temp.path(),
        Some("https://duckduckgo.com"),
        true,
        None,
        true,
    )
    .expect("launch args");

    let expected_user_data_dir = format!(
        "--user-data-dir={}",
        temp.path().join("engine-profile").to_string_lossy()
    );
    assert!(args.iter().any(|value| value == &expected_user_data_dir));
    assert!(args.iter().any(|value| value == "--incognito"));
    assert!(args.iter().any(|value| value == "--disable-sync"));
    assert!(args.iter().any(|value| value == "https://duckduckgo.com"));
}

#[test]
fn chromium_launch_args_apply_identity_policy_overrides() {
    let temp = tempfile::tempdir().expect("tempdir");
    let policy_dir = temp.path().join("policy");
    fs::create_dir_all(&policy_dir).expect("policy dir");
    fs::write(
        policy_dir.join("identity-preset.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "core": {
                "user_agent": "Mozilla/5.0 Test Browser"
            },
            "locale": {
                "navigator_language": "ru",
                "languages": ["ru", "en", "en-GB", "en-US"]
            },
            "window": {
                "outer_width": 1440,
                "outer_height": 920,
                "screen_x": 320,
                "screen_y": 343
            },
            "screen": {
                "width": 2560,
                "height": 1440
            }
        }))
        .expect("serialize identity policy"),
    )
    .expect("write identity policy");

    let args = launch_args(
        EngineKind::Chromium,
        temp.path(),
        Some("https://duckduckgo.com"),
        false,
        None,
        false,
    )
    .expect("launch args");

    assert!(args
        .iter()
        .any(|value| value == "--user-agent=Mozilla/5.0 Test Browser"));
    assert!(args.iter().any(|value| value == "--lang=ru"));
    assert!(args
        .iter()
        .any(|value| value == "--accept-lang=ru,en,en-GB,en-US"));
    assert!(args.iter().any(|value| value == "--window-size=1440,920"));
    assert!(args
        .iter()
        .any(|value| value == "--window-position=320,343"));

    let preferences: serde_json::Value = serde_json::from_slice(
        &fs::read(
            temp.path()
                .join("engine-profile")
                .join("Default")
                .join("Preferences"),
        )
        .expect("read preferences"),
    )
    .expect("parse preferences");
    assert_eq!(
        preferences["intl"]["accept_languages"].as_str(),
        Some("ru,en,en-GB,en-US")
    );
    assert_eq!(
        preferences["intl"]["selected_languages"].as_str(),
        Some("ru,en,en-GB,en-US")
    );
    let local_state: serde_json::Value = serde_json::from_slice(
        &fs::read(temp.path().join("engine-profile").join("Local State"))
            .expect("read local state"),
    )
    .expect("parse local state");
    assert_eq!(local_state["intl"]["app_locale"].as_str(), Some("ru"));
    assert_eq!(
        local_state["intl"]["selected_languages"].as_str(),
        Some("ru,en,en-GB,en-US")
    );

    let env = chromium_launch_environment(temp.path());
    assert_eq!(
        env,
        vec![
            ("LANG".to_string(), "ru.UTF-8".to_string()),
            ("LANGUAGE".to_string(), "ru:en:en-GB:en-US".to_string()),
            ("LC_ALL".to_string(), "ru.UTF-8".to_string()),
        ]
    );
}

#[test]
fn chromium_real_mode_keeps_native_user_agent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let policy_dir = temp.path().join("policy");
    fs::create_dir_all(&policy_dir).expect("policy dir");
    fs::write(
        policy_dir.join("identity-preset.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "mode": "real",
            "core": {
                "user_agent": "Mozilla/5.0 Launcher WebView"
            },
            "locale": {
                "navigator_language": "ru-RU",
                "languages": ["ru-RU", "ru", "en-US"]
            }
        }))
        .expect("serialize identity policy"),
    )
    .expect("write identity policy");

    let args = launch_args(
        EngineKind::Chromium,
        temp.path(),
        Some("https://duckduckgo.com"),
        false,
        None,
        true,
    )
    .expect("launch args");

    assert!(!args
        .iter()
        .any(|value| value == "--user-agent=Mozilla/5.0 Launcher WebView"));
    assert!(args.iter().any(|value| value == "--lang=ru-RU"));
    assert!(args
        .iter()
        .any(|value| value == "--accept-lang=ru-RU,ru,en-US"));
}

#[test]
fn chromium_prefers_vendor_chrome_binary_over_launcher_alias() {
    let temp = tempfile::tempdir().expect("tempdir");
    let chrome = temp.path().join("chrome.exe");
    let alias = temp.path().join("chromium-browser.exe");
    fs::write(&chrome, b"vendor chrome").expect("write chrome stub");
    fs::write(&alias, b"launcher alias").expect("write alias stub");

    let runtime = EngineRuntime::new(temp.path().join("state")).expect("runtime");
    let launch_binary = runtime
        .locate_binary(EngineKind::Chromium, temp.path())
        .expect("locate chromium binary");

    assert_eq!(launch_binary, chrome);
}

#[test]
fn chromium_prefers_vendor_chrome_from_stored_alias_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let chrome = temp.path().join("chrome.exe");
    let alias = temp.path().join("chromium-browser.exe");
    fs::write(&chrome, b"vendor chrome").expect("write chrome stub");
    fs::write(&alias, b"launcher alias").expect("write alias stub");

    let resolved = prefer_chromium_vendor_binary(&alias);

    assert_eq!(resolved, chrome);
}

#[test]
fn ungoogled_chromium_prefers_vendor_chrome_binary_over_named_wrapper() {
    let temp = tempfile::tempdir().expect("tempdir");
    let chrome = temp.path().join("chrome.exe");
    let wrapper = temp.path().join("ungoogled-chromium.exe");
    fs::write(&chrome, b"vendor chrome").expect("write chrome stub");
    fs::write(&wrapper, b"wrapper").expect("write wrapper stub");

    let runtime = EngineRuntime::new(temp.path().join("state")).expect("runtime");
    let launch_binary = runtime
        .locate_binary(EngineKind::UngoogledChromium, temp.path())
        .expect("locate ungoogled chromium binary");

    assert_eq!(launch_binary, chrome);
}

#[test]
fn extracts_official_librewolf_windows_portable_download_url() {
    let html = r#"
    <a href="https://dl.librewolf.net/release/windows/x86_64/librewolf-150.0.1-1-windows-x86_64-portable.zip">
        Download portable
    </a>
    "#;

    let url = extract_librewolf_download_url(html, "windows-x86_64-portable.zip")
        .expect("portable url");
    assert_eq!(
        url,
        "https://dl.librewolf.net/release/windows/x86_64/librewolf-150.0.1-1-windows-x86_64-portable.zip"
    );
}

#[test]
fn parses_librewolf_version_from_windows_file_name() {
    assert_eq!(
        parse_librewolf_version_from_file_name(
            "librewolf-150.0.1-1-windows-x86_64-portable.zip"
        ),
        Some("150.0.1-1".to_string())
    );
}

#[test]
fn selects_exact_ungoogled_chromium_windows_asset_from_release() {
    let release = GithubRelease {
        tag_name: "147.0.7727.137-1.1".to_string(),
        assets: vec![
            GithubAsset {
                name: "ungoogled-chromium_147.0.7727.137-1.1_installer_x64.exe"
                    .to_string(),
                browser_download_url: "https://example.invalid/installer.exe".to_string(),
            },
            GithubAsset {
                name: "ungoogled-chromium_147.0.7727.137-1.1_windows_x64.zip".to_string(),
                browser_download_url:
                    "https://github.com/ungoogled-software/ungoogled-chromium-windows/releases/download/147.0.7727.137-1.1/ungoogled-chromium_147.0.7727.137-1.1_windows_x64.zip"
                        .to_string(),
            },
        ],
    };

    let asset = select_ungoogled_chromium_asset(&release)
        .expect("select asset")
        .expect("matching asset");

    assert_eq!(
        asset.name,
        "ungoogled-chromium_147.0.7727.137-1.1_windows_x64.zip"
    );
    assert_eq!(
        asset.browser_download_url,
        "https://github.com/ungoogled-software/ungoogled-chromium-windows/releases/download/147.0.7727.137-1.1/ungoogled-chromium_147.0.7727.137-1.1_windows_x64.zip"
    );
}


