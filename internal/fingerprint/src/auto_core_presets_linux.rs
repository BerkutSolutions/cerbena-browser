use super::*;

pub(super) fn linux_preset(platform: AutoPlatform, variant: usize, seed: u64) -> IdentityPreset {
    match variant {
    0 => desktop_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36".to_string(),
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
    width: 1920,
    height: 1080,
    device_pixel_ratio: 1.0,
    avail_width: 1920,
    avail_height: 1040,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 1920,
    outer_height: 1040,
    inner_width: 1888,
    inner_height: 958,
    screen_x: 0,
    screen_y: 0,
    },
    LocaleProfile {
    navigator_language: "en-US".to_string(),
    languages: vec!["en-US".to_string(), "en".to_string()],
    do_not_track: "1".to_string(),
    timezone_iana: "UTC".to_string(),
    timezone_offset_minutes: 0,
    },
    GeoProfile {
    latitude: 48.8566,
    longitude: 2.3522,
    accuracy_meters: 22.0,
    },
    WebGlProfile {
    vendor: "Google Inc. (Mesa)".to_string(),
    renderer: "ANGLE (Mesa Intel(R) UHD Graphics 620 (KBL GT2))".to_string(),
    params_json: "{\"maxTextureSize\":16384}".to_string(),
    },
    &["Noto Sans", "Liberation Sans", "DejaVu Sans", "Ubuntu"],
    BatteryProfile {
    charging: true,
    level: 0.84,
    },
    seed,
    ),
    _ => desktop_preset(
    platform,
    IdentityCore {
    user_agent: "Mozilla/5.0 (X11; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0".to_string(),
    platform: "Linux x86_64".to_string(),
    platform_version: "6.6".to_string(),
    brand: "Firefox".to_string(),
    brand_version: "126".to_string(),
    vendor: "".to_string(),
    vendor_sub: "".to_string(),
    product_sub: "20100101".to_string(),
    },
    HardwareProfile {
    cpu_threads: 6,
    max_touch_points: 0,
    device_memory_gb: 8,
    },
    ScreenProfile {
    width: 2560,
    height: 1440,
    device_pixel_ratio: 1.0,
    avail_width: 2560,
    avail_height: 1400,
    color_depth: 24,
    },
    WindowProfile {
    outer_width: 2560,
    outer_height: 1400,
    inner_width: 2522,
    inner_height: 1324,
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
    accuracy_meters: 18.0,
    },
    WebGlProfile {
    vendor: "Mozilla".to_string(),
    renderer: "Mesa Intel(R) Xe Graphics (TGL GT2)".to_string(),
    params_json: "{\"antialias\":true}".to_string(),
    },
    &["Noto Sans", "Liberation Sans", "DejaVu Sans Mono"],
    BatteryProfile {
    charging: false,
    level: 0.61,
    },
    seed,
    ),
    }
}
