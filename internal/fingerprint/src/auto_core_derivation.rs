use crate::model::{AutoPlatform, IdentityPreset};

#[derive(Debug, Clone)]
pub struct GeoSource {
    pub timezone_iana: String,
    pub timezone_offset_minutes: i16,
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_meters: f32,
    pub language: String,
}

pub(super) fn apply_seed_jitter_impl(preset: &mut IdentityPreset, platform: AutoPlatform, seed: u64) {
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

pub(crate) fn apply_auto_geolocation_impl(preset: &mut IdentityPreset, source: &GeoSource) {
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
