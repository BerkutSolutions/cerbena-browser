use super::*;

pub(super) fn android_preset(platform: AutoPlatform, variant: usize, seed: u64) -> IdentityPreset {
    match variant {
    0 => mobile_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.6367.82 Mobile Safari/537.36".to_string(),
    platform: "Linux armv8l".to_string(),
    platform_version: "14".to_string(),
    brand: "Chromium".to_string(),
    brand_version: "124".to_string(),
    vendor: "Google Inc.".to_string(),
    vendor_sub: "".to_string(),
    product_sub: "20030107".to_string(),
    },
    HardwareProfile {
    cpu_threads: 8,
    max_touch_points: 10,
    device_memory_gb: 8,
    },
    ScreenProfile {
    width: 1080,
    height: 2400,
    device_pixel_ratio: 2.625,
    avail_width: 1080,
    avail_height: 2330,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 412,
    outer_height: 915,
    inner_width: 412,
    inner_height: 824,
    screen_x: 0,
    screen_y: 0,
    },
    LocaleProfile {
    navigator_language: "en-US".to_string(),
    languages: vec!["en-US".to_string(), "en".to_string()],
    do_not_track: "1".to_string(),
    timezone_iana: "America/Los_Angeles".to_string(),
    timezone_offset_minutes: 480,
    },
    GeoProfile {
    latitude: 34.0522,
    longitude: -118.2437,
    accuracy_meters: 12.0,
    },
    WebGlProfile {
    vendor: "Google Inc. (Qualcomm)".to_string(),
    renderer: "ANGLE (Qualcomm, Adreno 740 OpenGL ES 3.2)".to_string(),
    params_json: "{\"maxTextureSize\":16384}".to_string(),
    },
    &["Roboto", "Noto Sans", "Google Sans"],
    BatteryProfile {
    charging: false,
    level: 0.48,
    },
    seed,
    ),
    _ => mobile_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (Linux; Android 14; SM-S921B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.6312.118 Mobile Safari/537.36".to_string(),
    platform: "Linux armv8l".to_string(),
    platform_version: "14".to_string(),
    brand: "Chromium".to_string(),
    brand_version: "123".to_string(),
    vendor: "Google Inc.".to_string(),
    vendor_sub: "".to_string(),
    product_sub: "20030107".to_string(),
    },
    HardwareProfile {
    cpu_threads: 8,
    max_touch_points: 10,
    device_memory_gb: 12,
    },
    ScreenProfile {
    width: 1080,
    height: 2340,
    device_pixel_ratio: 3.0,
    avail_width: 1080,
    avail_height: 2272,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 360,
    outer_height: 780,
    inner_width: 360,
    inner_height: 706,
    screen_x: 0,
    screen_y: 0,
    },
    LocaleProfile {
    navigator_language: "de-DE".to_string(),
    languages: vec!["de-DE".to_string(), "de".to_string(), "en-US".to_string()],
    do_not_track: "1".to_string(),
    timezone_iana: "Europe/Berlin".to_string(),
    timezone_offset_minutes: -60,
    },
    GeoProfile {
    latitude: 52.52,
    longitude: 13.405,
    accuracy_meters: 10.0,
    },
    WebGlProfile {
    vendor: "Google Inc. (ARM)".to_string(),
    renderer: "ANGLE (ARM, Mali-G715)".to_string(),
    params_json: "{\"maxTextureSize\":16384}".to_string(),
    },
    &["Roboto", "Noto Sans", "Samsung Sans"],
    BatteryProfile {
    charging: true,
    level: 0.71,
    },
    seed,
    ),
    }
}
