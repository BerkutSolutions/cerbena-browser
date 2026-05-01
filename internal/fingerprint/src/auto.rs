use crate::model::{
    AudioProfile, AutoGeoConfig, AutoPlatform, BatteryProfile, GeoProfile, HardwareProfile,
    IdentityCore, IdentityPreset, IdentityPresetMode, LocaleProfile, ScreenProfile, WebGlProfile,
    WindowProfile,
};

#[derive(Debug, Clone)]
pub struct GeoSource {
    pub timezone_iana: String,
    pub timezone_offset_minutes: i16,
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_meters: f32,
    pub language: String,
}

pub fn generate_auto_preset(platform: AutoPlatform, seed: u64) -> IdentityPreset {
    let variant = variant_index(seed, variant_count(platform));
    let mut preset = match platform {
        AutoPlatform::Windows => match variant {
            0 => desktop_preset(
                platform,
                IdentityCore {
                    user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36".to_string(),
                    platform: "Win32".to_string(),
                    platform_version: "10.0".to_string(),
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
                    inner_width: 1898,
                    inner_height: 948,
                    screen_x: 0,
                    screen_y: 0,
                },
                LocaleProfile {
                    navigator_language: "en-US".to_string(),
                    languages: vec!["en-US".to_string(), "en".to_string()],
                    do_not_track: "1".to_string(),
                    timezone_iana: "America/New_York".to_string(),
                    timezone_offset_minutes: 300,
                },
                GeoProfile {
                    latitude: 40.7128,
                    longitude: -74.006,
                    accuracy_meters: 24.0,
                },
                WebGlProfile {
                    vendor: "Google Inc. (Intel)".to_string(),
                    renderer: "ANGLE (Intel, Intel(R) UHD Graphics Direct3D11 vs_5_0 ps_5_0)".to_string(),
                    params_json: "{\"maxTextureSize\":16384}".to_string(),
                },
                &["Arial", "Segoe UI", "Calibri", "Verdana"],
                BatteryProfile {
                    charging: true,
                    level: 0.92,
                },
                seed,
            ),
            1 => desktop_preset(
                platform,
                IdentityCore {
                    user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:126.0) Gecko/20100101 Firefox/126.0".to_string(),
                    platform: "Win32".to_string(),
                    platform_version: "10.0".to_string(),
                    brand: "Firefox".to_string(),
                    brand_version: "126".to_string(),
                    vendor: "".to_string(),
                    vendor_sub: "".to_string(),
                    product_sub: "20100101".to_string(),
                },
                HardwareProfile {
                    cpu_threads: 12,
                    max_touch_points: 0,
                    device_memory_gb: 16,
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
                    inner_width: 2516,
                    inner_height: 1308,
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
                    accuracy_meters: 18.0,
                },
                WebGlProfile {
                    vendor: "Mozilla".to_string(),
                    renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0)".to_string(),
                    params_json: "{\"antialias\":true}".to_string(),
                },
                &["Arial", "Segoe UI", "Tahoma", "Trebuchet MS"],
                BatteryProfile {
                    charging: false,
                    level: 0.67,
                },
                seed,
            ),
            _ => desktop_preset(
                platform,
                IdentityCore {
                    user_agent: "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".to_string(),
                    platform: "Win32".to_string(),
                    platform_version: "10.0".to_string(),
                    brand: "Chromium".to_string(),
                    brand_version: "122".to_string(),
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
                    inner_width: 1340,
                    inner_height: 636,
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
                    accuracy_meters: 26.0,
                },
                WebGlProfile {
                    vendor: "Google Inc. (Intel)".to_string(),
                    renderer: "ANGLE (Intel, Intel(R) HD Graphics 620 Direct3D11 vs_5_0 ps_5_0)".to_string(),
                    params_json: "{\"maxTextureSize\":16384}".to_string(),
                },
                &["Arial", "Segoe UI", "Calibri"],
                BatteryProfile {
                    charging: true,
                    level: 0.79,
                },
                seed,
            ),
        },
        AutoPlatform::Windows8 => match variant {
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
        },
        AutoPlatform::Macos => match variant {
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
        },
        AutoPlatform::Linux => match variant {
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
        },
        AutoPlatform::Debian => match variant {
            0 => desktop_preset(
                platform,
                IdentityCore {
                    user_agent: "Mozilla/5.0 (X11; Debian; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0".to_string(),
                    platform: "Linux x86_64".to_string(),
                    platform_version: "6.1".to_string(),
                    brand: "Firefox".to_string(),
                    brand_version: "126".to_string(),
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
                    inner_width: 1880,
                    inner_height: 958,
                    screen_x: 0,
                    screen_y: 0,
                },
                LocaleProfile {
                    navigator_language: "fr-FR".to_string(),
                    languages: vec!["fr-FR".to_string(), "fr".to_string(), "en-US".to_string()],
                    do_not_track: "1".to_string(),
                    timezone_iana: "Europe/Paris".to_string(),
                    timezone_offset_minutes: -60,
                },
                GeoProfile {
                    latitude: 48.8566,
                    longitude: 2.3522,
                    accuracy_meters: 24.0,
                },
                WebGlProfile {
                    vendor: "Mozilla".to_string(),
                    renderer: "Mesa Intel(R) UHD Graphics 630".to_string(),
                    params_json: "{\"antialias\":true}".to_string(),
                },
                &["DejaVu Sans", "Liberation Sans", "Noto Sans"],
                BatteryProfile {
                    charging: true,
                    level: 0.77,
                },
                seed,
            ),
            _ => desktop_preset(
                platform,
                IdentityCore {
                    user_agent: "Mozilla/5.0 (X11; Debian; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".to_string(),
                    platform: "Linux x86_64".to_string(),
                    platform_version: "6.1".to_string(),
                    brand: "Chromium".to_string(),
                    brand_version: "122".to_string(),
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
                    height: 1440,
                    device_pixel_ratio: 1.0,
                    avail_width: 2560,
                    avail_height: 1400,
                    color_depth: 24,
                },
                WindowProfile {
                    outer_width: 2560,
                    outer_height: 1400,
                    inner_width: 2510,
                    inner_height: 1322,
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
                    accuracy_meters: 16.0,
                },
                WebGlProfile {
                    vendor: "Google Inc. (AMD)".to_string(),
                    renderer: "ANGLE (Mesa AMD Radeon RX 6600 XT)".to_string(),
                    params_json: "{\"maxTextureSize\":16384}".to_string(),
                },
                &["DejaVu Sans", "Liberation Sans", "Noto Sans Mono"],
                BatteryProfile {
                    charging: false,
                    level: 0.55,
                },
                seed,
            ),
        },
        AutoPlatform::Ubuntu => match variant {
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
        },
        AutoPlatform::Ios => match variant {
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
        },
        AutoPlatform::Android => match variant {
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
        },
    };

    apply_seed_jitter(&mut preset, platform, seed);
    preset
}

pub fn apply_auto_geolocation(preset: &mut IdentityPreset, source: &GeoSource) {
    if !preset.auto_geo.enabled {
        return;
    }
    preset.locale.timezone_iana = source.timezone_iana.clone();
    preset.locale.timezone_offset_minutes = source.timezone_offset_minutes;
    preset.locale.navigator_language = source.language.clone();
    if preset.locale.languages.is_empty() {
        preset.locale.languages.push(source.language.clone());
    } else {
        preset.locale.languages[0] = source.language.clone();
    }
    preset.geo.latitude = source.latitude;
    preset.geo.longitude = source.longitude;
    preset.geo.accuracy_meters = source.accuracy_meters;
}

fn variant_count(platform: AutoPlatform) -> usize {
    match platform {
        AutoPlatform::Windows => 3,
        AutoPlatform::Windows8 => 2,
        AutoPlatform::Macos => 2,
        AutoPlatform::Linux => 2,
        AutoPlatform::Debian => 2,
        AutoPlatform::Ubuntu => 2,
        AutoPlatform::Ios => 2,
        AutoPlatform::Android => 2,
    }
}

fn variant_index(seed: u64, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        ((seed ^ (seed >> 17) ^ (seed >> 33)) as usize) % count
    }
}

fn desktop_preset(
    platform: AutoPlatform,
    core: IdentityCore,
    hardware: HardwareProfile,
    screen: ScreenProfile,
    window: WindowProfile,
    locale: LocaleProfile,
    geo: GeoProfile,
    webgl: WebGlProfile,
    fonts: &[&str],
    battery: BatteryProfile,
    seed: u64,
) -> IdentityPreset {
    build_preset(
        platform,
        core,
        hardware,
        screen,
        window,
        locale,
        geo,
        webgl,
        fonts,
        AudioProfile {
            sample_rate: 48_000,
            max_channels: 2,
        },
        battery,
        seed,
    )
}

fn mobile_preset(
    platform: AutoPlatform,
    core: IdentityCore,
    hardware: HardwareProfile,
    screen: ScreenProfile,
    window: WindowProfile,
    locale: LocaleProfile,
    geo: GeoProfile,
    webgl: WebGlProfile,
    fonts: &[&str],
    battery: BatteryProfile,
    seed: u64,
) -> IdentityPreset {
    build_preset(
        platform,
        core,
        hardware,
        screen,
        window,
        locale,
        geo,
        webgl,
        fonts,
        AudioProfile {
            sample_rate: 48_000,
            max_channels: 2,
        },
        battery,
        seed,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_preset(
    platform: AutoPlatform,
    core: IdentityCore,
    hardware: HardwareProfile,
    screen: ScreenProfile,
    window: WindowProfile,
    locale: LocaleProfile,
    geo: GeoProfile,
    webgl: WebGlProfile,
    fonts: &[&str],
    audio: AudioProfile,
    battery: BatteryProfile,
    seed: u64,
) -> IdentityPreset {
    IdentityPreset {
        mode: IdentityPresetMode::Auto,
        auto_platform: Some(platform),
        display_name: Some(auto_platform_display_name(platform).to_string()),
        core,
        hardware,
        screen,
        window,
        locale,
        geo,
        auto_geo: AutoGeoConfig { enabled: false },
        webgl,
        canvas_noise_seed: seed,
        fonts: fonts.iter().map(|value| value.to_string()).collect(),
        audio,
        battery,
    }
}

fn auto_platform_display_name(platform: AutoPlatform) -> &'static str {
    match platform {
        AutoPlatform::Windows => "Windows (Auto)",
        AutoPlatform::Windows8 => "Windows 8 (Auto)",
        AutoPlatform::Macos => "macOS (Auto)",
        AutoPlatform::Linux => "Linux (Auto)",
        AutoPlatform::Debian => "Debian (Auto)",
        AutoPlatform::Ubuntu => "Ubuntu (Auto)",
        AutoPlatform::Ios => "iOS (Auto)",
        AutoPlatform::Android => "Android (Auto)",
    }
}

fn apply_seed_jitter(preset: &mut IdentityPreset, platform: AutoPlatform, seed: u64) {
    let width_loss = 12 + ((seed >> 2) % 18) as u16;
    let height_loss = if matches!(platform, AutoPlatform::Android | AutoPlatform::Ios) {
        54 + ((seed >> 4) % 22) as u16
    } else {
        74 + ((seed >> 4) % 24) as u16
    };

    if preset.window.outer_width > width_loss {
        preset.window.inner_width = preset.window.outer_width.saturating_sub(width_loss);
    }
    if preset.window.outer_height > height_loss {
        preset.window.inner_height = preset.window.outer_height.saturating_sub(height_loss);
    }

    if !matches!(platform, AutoPlatform::Android | AutoPlatform::Ios) {
        preset.window.screen_x = ((seed >> 10) % 2) as i16 * 8;
        preset.window.screen_y = ((seed >> 12) % 2) as i16 * 24;
    }

    let battery_steps = ((seed >> 6) % 7) as f32 * 0.02;
    let battery_sign = if ((seed >> 9) & 1) == 0 { 1.0 } else { -1.0 };
    preset.battery.level = (preset.battery.level + battery_steps * battery_sign).clamp(0.18, 0.99);
    preset.canvas_noise_seed =
        seed ^ ((preset.screen.width as u64) << 16) ^ preset.screen.height as u64;
    preset.geo.accuracy_meters += ((seed >> 15) % 7) as f32;
}
