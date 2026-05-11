use crate::model::{
    AudioProfile, AutoGeoConfig, AutoPlatform, BatteryProfile, GeoProfile, HardwareProfile,
    IdentityCore, IdentityPreset, IdentityPresetMode, LocaleProfile, ScreenProfile, WebGlProfile,
    WindowProfile,
};

#[path = "auto_core_presets_windows.rs"]
mod windows;
#[path = "auto_core_presets_windows8.rs"]
mod windows8;
#[path = "auto_core_presets_macos.rs"]
mod macos;
#[path = "auto_core_presets_linux.rs"]
mod linux;
#[path = "auto_core_presets_debian.rs"]
mod debian;
#[path = "auto_core_presets_ubuntu.rs"]
mod ubuntu;
#[path = "auto_core_presets_ios.rs"]
mod ios;
#[path = "auto_core_presets_android.rs"]
mod android;

pub(super) fn generate_auto_preset_impl(platform: AutoPlatform, seed: u64) -> IdentityPreset {
    let variant = variant_index(seed, variant_count(platform));
    let mut preset = match platform {
        AutoPlatform::Windows => windows::windows_preset(platform, variant, seed),
        AutoPlatform::Windows8 => windows8::windows8_preset(platform, variant, seed),
        AutoPlatform::Macos => macos::macos_preset(platform, variant, seed),
        AutoPlatform::Linux => linux::linux_preset(platform, variant, seed),
        AutoPlatform::Debian => debian::debian_preset(platform, variant, seed),
        AutoPlatform::Ubuntu => ubuntu::ubuntu_preset(platform, variant, seed),
        AutoPlatform::Ios => ios::ios_preset(platform, variant, seed),
        AutoPlatform::Android => android::android_preset(platform, variant, seed),
    };

    super::derivation::apply_seed_jitter_impl(&mut preset, platform, seed);
    preset
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
