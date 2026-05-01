pub mod auto;
pub mod consistency;
pub mod identity_tab;
pub mod model;
pub mod validate;

pub use auto::{apply_auto_geolocation, generate_auto_preset, GeoSource};
pub use consistency::{validate_consistency, ConsistencyIssue, ConsistencyLevel};
pub use identity_tab::{
    build_identity_preview, validate_identity_tab_save, IdentityTabSaveOutcome,
};
pub use model::{
    AudioProfile, AutoGeoConfig, AutoPlatform, BatteryProfile, GeoProfile, HardwareProfile,
    IdentityCore, IdentityPreset, IdentityPresetMode, LocaleProfile, ScreenProfile, WebGlProfile,
    WindowProfile,
};
pub use validate::validate_identity_preset;
