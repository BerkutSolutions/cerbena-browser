use super::*;

pub(super) fn ubuntu_preset(platform: AutoPlatform, variant: usize, seed: u64) -> IdentityPreset {
    match variant {
    0 => desktop_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0".to_string(),
    platform: "Linux x86_64".to_string(),
    platform_version: "6.8".to_string(),
    brand: "Firefox".to_string(),
    brand_version: "126".to_string(),
    vendor: "".to_string(),
    vendor_sub: "".to_string(),
    product_sub: "20100101".to_string(),
    },
    HardwareProfile {
    cpu_threads: 8,
    max_touch_points: 0,
    device_memory_gb: 16,
    },
    ScreenProfile {
    width: 1920,
    height: 1200,
    device_pixel_ratio: 1.0,
    avail_width: 1920,
    avail_height: 1154,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 1920,
    outer_height: 1154,
    inner_width: 1886,
    inner_height: 1078,
    screen_x: 0,
    screen_y: 0,
    },
    LocaleProfile {
    navigator_language: "en-US".to_string(),
    languages: vec!["en-US".to_string(), "en".to_string()],
    do_not_track: "1".to_string(),
    timezone_iana: "America/Toronto".to_string(),
    timezone_offset_minutes: 300,
    },
    GeoProfile {
    latitude: 43.6532,
    longitude: -79.3832,
    accuracy_meters: 19.0,
    },
    WebGlProfile {
    vendor: "Mozilla".to_string(),
    renderer: "Mesa Intel(R) Iris(R) Xe Graphics".to_string(),
    params_json: "{\"antialias\":true}".to_string(),
    },
    &["Ubuntu", "Noto Sans", "Liberation Sans", "DejaVu Sans"],
    BatteryProfile {
    charging: true,
    level: 0.83,
    },
    seed,
    ),
    _ => desktop_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (X11; Ubuntu; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36".to_string(),
    platform: "Linux x86_64".to_string(),
    platform_version: "6.8".to_string(),
    brand: "Chromium".to_string(),
    brand_version: "124".to_string(),
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
    width: 2560,
    height: 1600,
    device_pixel_ratio: 1.0,
    avail_width: 2560,
    avail_height: 1550,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 2560,
    outer_height: 1550,
    inner_width: 2514,
    inner_height: 1472,
    screen_x: 0,
    screen_y: 0,
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
    accuracy_meters: 17.0,
    },
    WebGlProfile {
    vendor: "Google Inc. (Mesa)".to_string(),
    renderer: "ANGLE (Mesa AMD Radeon Graphics)".to_string(),
    params_json: "{\"maxTextureSize\":16384}".to_string(),
    },
    &["Ubuntu", "Noto Sans", "Cantarell", "Liberation Sans"],
    BatteryProfile {
    charging: false,
    level: 0.62,
    },
    seed,
    ),
    }
}
