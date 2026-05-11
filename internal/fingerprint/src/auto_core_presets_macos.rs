use super::*;

pub(super) fn macos_preset(platform: AutoPlatform, variant: usize, seed: u64) -> IdentityPreset {
    match variant {
    0 => desktop_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15".to_string(),
    platform: "MacIntel".to_string(),
    platform_version: "14.4".to_string(),
    brand: "Safari".to_string(),
    brand_version: "17.4".to_string(),
    vendor: "Apple Computer, Inc.".to_string(),
    vendor_sub: "".to_string(),
    product_sub: "20030107".to_string(),
    },
    HardwareProfile {
    cpu_threads: 8,
    max_touch_points: 0,
    device_memory_gb: 8,
    },
    ScreenProfile {
    width: 2560,
    height: 1600,
    device_pixel_ratio: 2.0,
    avail_width: 1280,
    avail_height: 760,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 1280,
    outer_height: 760,
    inner_width: 1246,
    inner_height: 684,
    screen_x: 0,
    screen_y: 25,
    },
    LocaleProfile {
    navigator_language: "en-US".to_string(),
    languages: vec!["en-US".to_string(), "en".to_string()],
    do_not_track: "1".to_string(),
    timezone_iana: "America/Los_Angeles".to_string(),
    timezone_offset_minutes: 480,
    },
    GeoProfile {
    latitude: 37.7749,
    longitude: -122.4194,
    accuracy_meters: 20.0,
    },
    WebGlProfile {
    vendor: "Apple".to_string(),
    renderer: "Apple MTL Renderer".to_string(),
    params_json: "{\"maxTextureSize\":16384}".to_string(),
    },
    &["Helvetica Neue", "SF Pro Text", "Arial", "Menlo"],
    BatteryProfile {
    charging: true,
    level: 0.88,
    },
    seed,
    ),
    _ => desktop_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_6_6) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36".to_string(),
    platform: "MacIntel".to_string(),
    platform_version: "13.6".to_string(),
    brand: "Chromium".to_string(),
    brand_version: "123".to_string(),
    vendor: "Google Inc.".to_string(),
    vendor_sub: "".to_string(),
    product_sub: "20030107".to_string(),
    },
    HardwareProfile {
    cpu_threads: 8,
    max_touch_points: 0,
    device_memory_gb: 16,
    },
    ScreenProfile {
    width: 3024,
    height: 1964,
    device_pixel_ratio: 2.0,
    avail_width: 1512,
    avail_height: 914,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 1512,
    outer_height: 914,
    inner_width: 1468,
    inner_height: 838,
    screen_x: 0,
    screen_y: 25,
    },
    LocaleProfile {
    navigator_language: "en-GB".to_string(),
    languages: vec!["en-GB".to_string(), "en".to_string()],
    do_not_track: "1".to_string(),
    timezone_iana: "Europe/London".to_string(),
    timezone_offset_minutes: 0,
    },
    GeoProfile {
    latitude: 51.5072,
    longitude: -0.1276,
    accuracy_meters: 18.0,
    },
    WebGlProfile {
    vendor: "Google Inc. (Apple)".to_string(),
    renderer: "ANGLE Metal Renderer: Apple M1 Pro".to_string(),
    params_json: "{\"maxTextureSize\":16384}".to_string(),
    },
    &["Helvetica Neue", "SF Pro Text", "Arial", "Courier"],
    BatteryProfile {
    charging: false,
    level: 0.63,
    },
    seed,
    ),
    }
}
