use super::*;

pub(super) fn ios_preset(platform: AutoPlatform, variant: usize, seed: u64) -> IdentityPreset {
    match variant {
    0 => mobile_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1".to_string(),
    platform: "iPhone".to_string(),
    platform_version: "18.4".to_string(),
    brand: "Safari".to_string(),
    brand_version: "18".to_string(),
    vendor: "Apple Computer, Inc.".to_string(),
    vendor_sub: "".to_string(),
    product_sub: "20030107".to_string(),
    },
    HardwareProfile {
    cpu_threads: 6,
    max_touch_points: 5,
    device_memory_gb: 6,
    },
    ScreenProfile {
    width: 1179,
    height: 2556,
    device_pixel_ratio: 3.0,
    avail_width: 1179,
    avail_height: 2478,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 393,
    outer_height: 852,
    inner_width: 393,
    inner_height: 766,
    screen_x: 0,
    screen_y: 0,
    },
    LocaleProfile {
    navigator_language: "en-US".to_string(),
    languages: vec!["en-US".to_string(), "en".to_string()],
    do_not_track: "1".to_string(),
    timezone_iana: "America/New_York".to_string(),
    timezone_offset_minutes: 240,
    },
    GeoProfile {
    latitude: 40.7128,
    longitude: -74.006,
    accuracy_meters: 9.0,
    },
    WebGlProfile {
    vendor: "Apple Inc.".to_string(),
    renderer: "Apple GPU".to_string(),
    params_json: "{\"maxTextureSize\":16384}".to_string(),
    },
    &["San Francisco", "Helvetica Neue", "Arial"],
    BatteryProfile {
    charging: false,
    level: 0.58,
    },
    seed,
    ),
    _ => mobile_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (iPad; CPU OS 17_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/148.0.0.0 Mobile/15E148 Safari/604.1".to_string(),
    platform: "iPad".to_string(),
    platform_version: "17.7".to_string(),
    brand: "Chrome".to_string(),
    brand_version: "148".to_string(),
    vendor: "Google Inc.".to_string(),
    vendor_sub: "".to_string(),
    product_sub: "20030107".to_string(),
    },
    HardwareProfile {
    cpu_threads: 8,
    max_touch_points: 5,
    device_memory_gb: 8,
    },
    ScreenProfile {
    width: 2048,
    height: 2732,
    device_pixel_ratio: 2.0,
    avail_width: 2048,
    avail_height: 2654,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 1024,
    outer_height: 1366,
    inner_width: 1024,
    inner_height: 1278,
    screen_x: 0,
    screen_y: 0,
    },
    LocaleProfile {
    navigator_language: "en-GB".to_string(),
    languages: vec!["en-GB".to_string(), "en".to_string()],
    do_not_track: "1".to_string(),
    timezone_iana: "Europe/London".to_string(),
    timezone_offset_minutes: -60,
    },
    GeoProfile {
    latitude: 51.5072,
    longitude: -0.1276,
    accuracy_meters: 11.0,
    },
    WebGlProfile {
    vendor: "Apple Inc.".to_string(),
    renderer: "Apple GPU".to_string(),
    params_json: "{\"maxTextureSize\":16384}".to_string(),
    },
    &["San Francisco", "Helvetica Neue", "Arial"],
    BatteryProfile {
    charging: true,
    level: 0.73,
    },
    seed,
    ),
    }
}
