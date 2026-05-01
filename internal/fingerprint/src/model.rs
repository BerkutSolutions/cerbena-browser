use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityPresetMode {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutoPlatform {
    Windows,
    Windows8,
    Macos,
    Linux,
    Debian,
    Ubuntu,
    Ios,
    Android,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCore {
    pub user_agent: String,
    pub platform: String,
    pub platform_version: String,
    pub brand: String,
    pub brand_version: String,
    pub vendor: String,
    pub vendor_sub: String,
    pub product_sub: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu_threads: u16,
    pub max_touch_points: u8,
    pub device_memory_gb: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenProfile {
    pub width: u16,
    pub height: u16,
    pub device_pixel_ratio: f32,
    pub avail_width: u16,
    pub avail_height: u16,
    pub color_depth: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowProfile {
    pub outer_width: u16,
    pub outer_height: u16,
    pub inner_width: u16,
    pub inner_height: u16,
    pub screen_x: i16,
    pub screen_y: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleProfile {
    pub navigator_language: String,
    pub languages: Vec<String>,
    pub do_not_track: String,
    pub timezone_iana: String,
    pub timezone_offset_minutes: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoProfile {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_meters: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoGeoConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebGlProfile {
    pub vendor: String,
    pub renderer: String,
    pub params_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioProfile {
    pub sample_rate: u32,
    pub max_channels: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryProfile {
    pub charging: bool,
    pub level: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityPreset {
    pub mode: IdentityPresetMode,
    pub auto_platform: Option<AutoPlatform>,
    #[serde(default)]
    pub display_name: Option<String>,
    pub core: IdentityCore,
    pub hardware: HardwareProfile,
    pub screen: ScreenProfile,
    pub window: WindowProfile,
    pub locale: LocaleProfile,
    pub geo: GeoProfile,
    pub auto_geo: AutoGeoConfig,
    pub webgl: WebGlProfile,
    pub canvas_noise_seed: u64,
    pub fonts: Vec<String>,
    pub audio: AudioProfile,
    pub battery: BatteryProfile,
}
