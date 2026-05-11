use super::*;

pub(super) fn windows8_preset(platform: AutoPlatform, variant: usize, seed: u64) -> IdentityPreset {
    match variant {
    0 => desktop_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (Windows NT 6.2; Win64; x64; rv:115.0) Gecko/20100101 Firefox/115.0".to_string(),
    platform: "Win32".to_string(),
    platform_version: "6.2".to_string(),
    brand: "Firefox".to_string(),
    brand_version: "115".to_string(),
    vendor: "".to_string(),
    vendor_sub: "".to_string(),
    product_sub: "20100101".to_string(),
    },
    HardwareProfile {
    cpu_threads: 4,
    max_touch_points: 0,
    device_memory_gb: 8,
    },
    ScreenProfile {
    width: 1366,
    height: 768,
    device_pixel_ratio: 1.0,
    avail_width: 1366,
    avail_height: 728,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 1366,
    outer_height: 728,
    inner_width: 1334,
    inner_height: 650,
    screen_x: 0,
    screen_y: 0,
    },
    LocaleProfile {
    navigator_language: "en-US".to_string(),
    languages: vec!["en-US".to_string(), "en".to_string()],
    do_not_track: "1".to_string(),
    timezone_iana: "America/Chicago".to_string(),
    timezone_offset_minutes: 360,
    },
    GeoProfile {
    latitude: 41.8781,
    longitude: -87.6298,
    accuracy_meters: 30.0,
    },
    WebGlProfile {
    vendor: "Mozilla".to_string(),
    renderer: "ANGLE (Intel, Intel(R) HD Graphics 4000 Direct3D11 vs_5_0 ps_5_0)".to_string(),
    params_json: "{\"antialias\":true}".to_string(),
    },
    &["Arial", "Segoe UI", "Tahoma"],
    BatteryProfile {
    charging: true,
    level: 0.72,
    },
    seed,
    ),
    _ => desktop_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (Windows NT 6.2; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/109.0.0.0 Safari/537.36".to_string(),
    platform: "Win32".to_string(),
    platform_version: "6.2".to_string(),
    brand: "Chromium".to_string(),
    brand_version: "109".to_string(),
    vendor: "Google Inc.".to_string(),
    vendor_sub: "".to_string(),
    product_sub: "20030107".to_string(),
    },
    HardwareProfile {
    cpu_threads: 4,
    max_touch_points: 0,
    device_memory_gb: 8,
    },
    ScreenProfile {
    width: 1600,
    height: 900,
    device_pixel_ratio: 1.0,
    avail_width: 1600,
    avail_height: 860,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 1600,
    outer_height: 860,
    inner_width: 1562,
    inner_height: 776,
    screen_x: 0,
    screen_y: 0,
    },
    LocaleProfile {
    navigator_language: "ru-RU".to_string(),
    languages: vec!["ru-RU".to_string(), "ru".to_string(), "en-US".to_string()],
    do_not_track: "1".to_string(),
    timezone_iana: "Europe/Moscow".to_string(),
    timezone_offset_minutes: -180,
    },
    GeoProfile {
    latitude: 55.7558,
    longitude: 37.6173,
    accuracy_meters: 28.0,
    },
    WebGlProfile {
    vendor: "Google Inc. (NVIDIA)".to_string(),
    renderer: "ANGLE (NVIDIA, NVIDIA GeForce GT 740M Direct3D11 vs_5_0 ps_5_0)".to_string(),
    params_json: "{\"maxTextureSize\":8192}".to_string(),
    },
    &["Arial", "Segoe UI", "Calibri"],
    BatteryProfile {
    charging: false,
    level: 0.58,
    },
    seed,
    ),
    }
}
