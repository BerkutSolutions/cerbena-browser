use crate::model::{AutoPlatform, IdentityPreset};

#[path = "auto_core_derivation.rs"]
mod derivation;
#[path = "auto_core_presets.rs"]
mod presets;

pub use derivation::GeoSource;

pub fn generate_auto_preset(platform: AutoPlatform, seed: u64) -> IdentityPreset {
    presets::generate_auto_preset_impl(platform, seed)
}

pub fn apply_auto_geolocation(preset: &mut IdentityPreset, source: &GeoSource) {
    derivation::apply_auto_geolocation_impl(preset, source);
}
