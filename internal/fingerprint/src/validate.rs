use thiserror::Error;

use crate::model::IdentityPreset;

#[derive(Debug, Error)]
pub enum IdentityValidationError {
    #[error("invalid identity preset: {0}")]
    Invalid(String),
}

pub fn validate_identity_preset(p: &IdentityPreset) -> Result<(), IdentityValidationError> {
    if p.core.user_agent.trim().is_empty() {
        return Err(IdentityValidationError::Invalid(
            "user_agent must not be empty".to_string(),
        ));
    }
    if p.core.platform.trim().is_empty() {
        return Err(IdentityValidationError::Invalid(
            "platform must not be empty".to_string(),
        ));
    }
    if p.hardware.cpu_threads == 0 || p.hardware.cpu_threads > 256 {
        return Err(IdentityValidationError::Invalid(
            "cpu_threads out of range".to_string(),
        ));
    }
    if p.hardware.device_memory_gb == 0 || p.hardware.device_memory_gb > 1024 {
        return Err(IdentityValidationError::Invalid(
            "device_memory_gb out of range".to_string(),
        ));
    }
    if p.screen.width < 320 || p.screen.height < 240 {
        return Err(IdentityValidationError::Invalid(
            "screen dimensions too small".to_string(),
        ));
    }
    if p.window.inner_width > p.window.outer_width || p.window.inner_height > p.window.outer_height
    {
        return Err(IdentityValidationError::Invalid(
            "inner window cannot exceed outer window".to_string(),
        ));
    }
    if p.screen.avail_width > p.screen.width || p.screen.avail_height > p.screen.height {
        return Err(IdentityValidationError::Invalid(
            "available screen size cannot exceed screen size".to_string(),
        ));
    }
    if p.locale.navigator_language.trim().is_empty() || p.locale.languages.is_empty() {
        return Err(IdentityValidationError::Invalid(
            "locale languages must not be empty".to_string(),
        ));
    }
    if !(-90.0..=90.0).contains(&p.geo.latitude) {
        return Err(IdentityValidationError::Invalid(
            "latitude out of range".to_string(),
        ));
    }
    if !(-180.0..=180.0).contains(&p.geo.longitude) {
        return Err(IdentityValidationError::Invalid(
            "longitude out of range".to_string(),
        ));
    }
    if p.geo.accuracy_meters <= 0.0 {
        return Err(IdentityValidationError::Invalid(
            "accuracy_meters must be positive".to_string(),
        ));
    }
    if p.audio.sample_rate < 8000 || p.audio.sample_rate > 192000 {
        return Err(IdentityValidationError::Invalid(
            "audio sample_rate out of range".to_string(),
        ));
    }
    if p.audio.max_channels == 0 || p.audio.max_channels > 32 {
        return Err(IdentityValidationError::Invalid(
            "audio max_channels out of range".to_string(),
        ));
    }
    if !(0.0..=1.0).contains(&p.battery.level) {
        return Err(IdentityValidationError::Invalid(
            "battery level must be between 0 and 1".to_string(),
        ));
    }
    if p.webgl.vendor.trim().is_empty() || p.webgl.renderer.trim().is_empty() {
        return Err(IdentityValidationError::Invalid(
            "webgl vendor/renderer must not be empty".to_string(),
        ));
    }
    if serde_json::from_str::<serde_json::Value>(&p.webgl.params_json).is_err() {
        return Err(IdentityValidationError::Invalid(
            "webgl params_json is not valid JSON".to_string(),
        ));
    }
    if p.fonts.is_empty() {
        return Err(IdentityValidationError::Invalid(
            "fonts list must not be empty".to_string(),
        ));
    }
    Ok(())
}
