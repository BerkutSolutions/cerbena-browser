use std::{fs, sync::Mutex};

use browser_engine::{
    contract::{EngineAdapter, LaunchRequest},
    CamoufoxAdapter, WayfernAdapter,
};
use browser_fingerprint::{
    generate_auto_preset, validate_consistency, validate_identity_preset, AutoPlatform,
    IdentityPreset, IdentityPresetMode,
};
use browser_profile::{CreateProfileInput, Engine, ProfileManager};
use tempfile::tempdir;

static APPDATA_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn chromium_profile_creation_and_fingerprint_values_are_applied() {
    let _guard = APPDATA_ENV_LOCK.lock().expect("lock appdata env");
    let tmp = tempdir().expect("tempdir");
    let appdata_root = tmp.path().join("appdata");
    fs::create_dir_all(&appdata_root).expect("mk appdata");
    let previous_appdata = std::env::var_os("APPDATA");
    std::env::set_var("APPDATA", &appdata_root);
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let profile = manager
        .create_profile(CreateProfileInput {
            name: "Chromium Profile".to_string(),
            description: Some("Wayfern chromium test".to_string()),
            tags: vec!["chromium".to_string(), "e2e".to_string()],
            engine: Engine::Wayfern,
            default_start_page: Some("https://example.com".to_string()),
            default_search_provider: Some("duckduckgo".to_string()),
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .expect("create profile");

    let mut preset = generate_auto_preset(AutoPlatform::Windows, 101);
    preset.mode = IdentityPresetMode::Manual;
    preset.core.user_agent =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/125.0.0.0 Safari/537.36"
            .to_string();
    preset.core.platform = "Win32".to_string();
    preset.core.brand = "Chromium".to_string();
    preset.core.brand_version = "125".to_string();
    preset.hardware.cpu_threads = 12;
    preset.hardware.device_memory_gb = 16;
    preset.locale.timezone_iana = "Europe/Berlin".to_string();
    preset.geo.latitude = 52.52;
    preset.geo.longitude = 13.40;
    preset.webgl.params_json = "{\"maxTextureSize\":16384}".to_string();

    validate_identity_preset(&preset).expect("valid chromium preset");
    let issues = validate_consistency(&preset, Some("vpn"));
    assert!(issues
        .iter()
        .all(|i| !matches!(i.level, browser_fingerprint::ConsistencyLevel::Blocking)));
    assert!(preset.core.user_agent.contains("Chrome/125"));
    assert_eq!(preset.core.platform, "Win32");
    assert_eq!(preset.hardware.cpu_threads, 12);
    assert_eq!(preset.locale.timezone_iana, "Europe/Berlin");

    let adapter = WayfernAdapter {
        install_root: tmp.path().join("install"),
        cache_dir: tmp.path().join("cache"),
        tos_version: "2026-04".to_string(),
    };
    let profile_root = tmp.path().join(profile.id.to_string());
    fs::create_dir_all(&profile_root).expect("profile root");
    let result = std::panic::catch_unwind(|| {
        adapter
            .acknowledge_tos(&profile_root, profile.id)
            .expect("tos ack");
        let req = LaunchRequest {
            profile_id: profile.id,
            profile_root,
            binary_path: tmp.path().join("bin").join("wayfern.exe"),
            args: vec![
                format!("--user-agent={}", preset.core.user_agent),
                format!("--lang={}", preset.locale.navigator_language),
                format!(
                    "--window-size={},{}",
                    preset.screen.width, preset.screen.height
                ),
                "--engine=chromium".to_string(),
            ],
            env: vec![],
        };
        let plan = adapter.build_launch_plan(req).expect("launch plan");
        assert!(plan.args.iter().any(|a| a.contains("Chrome/125")));
        assert!(plan.args.iter().any(|a| a == "--engine=chromium"));
    });

    match previous_appdata {
        Some(value) => std::env::set_var("APPDATA", value),
        None => std::env::remove_var("APPDATA"),
    }
    result.expect("chromium launch plan");
}

#[test]
fn firefox_profile_creation_and_fingerprint_values_are_applied() {
    let tmp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(tmp.path()).expect("manager");
    let profile = manager
        .create_profile(CreateProfileInput {
            name: "Firefox Profile".to_string(),
            description: Some("Camoufox firefox test".to_string()),
            tags: vec!["firefox".to_string(), "e2e".to_string()],
            engine: Engine::Camoufox,
            default_start_page: Some("https://mozilla.org".to_string()),
            default_search_provider: Some("startpage".to_string()),
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .expect("create profile");

    let mut preset: IdentityPreset = generate_auto_preset(AutoPlatform::Linux, 202);
    preset.mode = IdentityPresetMode::Manual;
    preset.core.user_agent =
        "Mozilla/5.0 (X11; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0".to_string();
    preset.core.platform = "Linux x86_64".to_string();
    preset.core.brand = "Firefox".to_string();
    preset.core.brand_version = "126".to_string();
    preset.core.vendor = "Mozilla".to_string();
    preset.hardware.cpu_threads = 8;
    preset.hardware.device_memory_gb = 8;
    preset.screen.width = 1366;
    preset.screen.height = 768;
    preset.screen.avail_width = 1366;
    preset.screen.avail_height = 728;
    preset.window.outer_width = 1366;
    preset.window.outer_height = 728;
    preset.window.inner_width = 1280;
    preset.window.inner_height = 680;
    preset.locale.timezone_iana = "America/New_York".to_string();
    preset.geo.latitude = 40.71;
    preset.geo.longitude = -74.0;
    preset.webgl.vendor = "Mozilla".to_string();
    preset.webgl.renderer = "WebRender".to_string();
    preset.webgl.params_json = "{\"antialias\":true}".to_string();

    validate_identity_preset(&preset).expect("valid firefox preset");
    let issues = validate_consistency(&preset, Some("proxy"));
    assert!(issues
        .iter()
        .all(|i| !matches!(i.level, browser_fingerprint::ConsistencyLevel::Blocking)));
    assert!(preset.core.user_agent.contains("Firefox/126.0"));
    assert_eq!(preset.core.brand, "Firefox");
    assert_eq!(preset.locale.timezone_iana, "America/New_York");

    let adapter = CamoufoxAdapter {
        install_root: tmp.path().join("install"),
        cache_dir: tmp.path().join("cache"),
    };
    let req = LaunchRequest {
        profile_id: profile.id,
        profile_root: tmp.path().join(profile.id.to_string()),
        binary_path: tmp.path().join("bin").join("camoufox.exe"),
        args: vec![
            format!("--user-agent={}", preset.core.user_agent),
            format!("--lang={}", preset.locale.navigator_language),
            format!(
                "--window-size={},{}",
                preset.screen.width, preset.screen.height
            ),
            "--engine=firefox".to_string(),
        ],
        env: vec![],
    };
    let plan = adapter.build_launch_plan(req).expect("launch plan");
    assert!(plan.args.iter().any(|a| a.contains("Firefox/126.0")));
    assert!(plan.args.iter().any(|a| a == "--engine=firefox"));
}
