use browser_fingerprint::{
    apply_auto_geolocation, generate_auto_preset, validate_consistency, validate_identity_preset,
    validate_identity_tab_save, AutoPlatform, GeoSource, IdentityPreset, IdentityPresetMode,
    ScreenProfile, WindowProfile,
};

#[test]
fn auto_preset_generation_produces_coherent_profile() {
    let preset = generate_auto_preset(AutoPlatform::Windows, 42);
    assert_eq!(preset.mode, IdentityPresetMode::Auto);
    assert!(preset.core.user_agent.contains("Windows"));
    validate_identity_preset(&preset).expect("valid auto preset");
}

#[test]
fn windows8_and_ubuntu_presets_use_expected_platform_traits() {
    let windows8 = generate_auto_preset(AutoPlatform::Windows8, 77);
    let ubuntu = generate_auto_preset(AutoPlatform::Ubuntu, 91);

    assert!(windows8.core.user_agent.contains("Windows NT 6.2"));
    assert_eq!(windows8.hardware.max_touch_points, 0);
    assert!(ubuntu.core.user_agent.to_lowercase().contains("ubuntu"));
    assert!(ubuntu.core.platform.to_lowercase().contains("linux"));
}

#[test]
fn ios_presets_use_mobile_apple_traits() {
    let ios = generate_auto_preset(AutoPlatform::Ios, 31);
    let user_agent = ios.core.user_agent.to_lowercase();
    assert!(user_agent.contains("iphone") || user_agent.contains("ipad"));
    assert!(ios.hardware.max_touch_points >= 5);
    validate_identity_preset(&ios).expect("valid ios auto preset");
}

#[test]
fn manual_core_and_screen_validation_works() {
    let mut p = generate_auto_preset(AutoPlatform::Linux, 1);
    p.mode = IdentityPresetMode::Manual;
    p.core.user_agent = "CustomUA/1.0".to_string();
    p.screen = ScreenProfile {
        width: 1366,
        height: 768,
        device_pixel_ratio: 1.0,
        avail_width: 1366,
        avail_height: 728,
        color_depth: 24,
    };
    p.window = WindowProfile {
        outer_width: 1366,
        outer_height: 728,
        inner_width: 1200,
        inner_height: 650,
        screen_x: 10,
        screen_y: 10,
    };
    validate_identity_preset(&p).expect("manual preset valid");
}

#[test]
fn invalid_window_geometry_is_rejected() {
    let mut p: IdentityPreset = generate_auto_preset(AutoPlatform::Macos, 0);
    p.window.inner_width = p.window.outer_width + 1;
    assert!(validate_identity_preset(&p).is_err());
}

#[test]
fn auto_geo_binding_updates_locale_and_geo() {
    let mut p = generate_auto_preset(AutoPlatform::Linux, 2);
    p.auto_geo.enabled = true;
    apply_auto_geolocation(
        &mut p,
        &GeoSource {
            timezone_iana: "Europe/Moscow".to_string(),
            timezone_offset_minutes: 180,
            latitude: 55.75,
            longitude: 37.61,
            accuracy_meters: 25.0,
            language: "ru-RU".to_string(),
        },
    );
    assert_eq!(p.locale.timezone_iana, "Europe/Moscow");
    assert_eq!(p.locale.navigator_language, "ru-RU");
    assert_eq!(p.geo.latitude, 55.75);
}

#[test]
fn webgl_audio_battery_fields_are_validated() {
    let mut p = generate_auto_preset(AutoPlatform::Windows, 9);
    p.webgl.params_json = "{\"maxTextureSize\":8192}".to_string();
    p.audio.sample_rate = 44100;
    p.audio.max_channels = 2;
    p.battery.level = 0.75;
    p.fonts = vec!["Arial".to_string(), "Segoe UI".to_string()];
    validate_identity_preset(&p).expect("extended fields valid");
}

#[test]
fn consistency_validator_returns_issues() {
    let mut p = generate_auto_preset(AutoPlatform::Android, 1);
    p.core.user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64)".to_string();
    p.locale.timezone_iana = "UTC".to_string();
    let issues = validate_consistency(&p, Some("tor"));
    assert!(!issues.is_empty());
}

#[test]
fn identity_tab_save_is_blocked_for_blocking_consistency_issue() {
    let mut p = generate_auto_preset(AutoPlatform::Windows, 13);
    p.core.platform = "linux x86_64".to_string();
    let outcome = validate_identity_tab_save(&p, None);
    assert!(!outcome.allowed_to_save);
    assert!(!outcome.preview.blocking_issues.is_empty());
}
