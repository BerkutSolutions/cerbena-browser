use serde::{Deserialize, Serialize};

use crate::model::{AutoPlatform, IdentityPreset};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsistencyLevel {
    Warning,
    Blocking,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyIssue {
    pub level: ConsistencyLevel,
    pub code: String,
    pub message: String,
}

pub fn validate_consistency(
    preset: &IdentityPreset,
    active_route: Option<&str>,
) -> Vec<ConsistencyIssue> {
    let mut issues = Vec::new();

    if let Some(platform) = preset.auto_platform {
        match platform {
            AutoPlatform::Windows if !preset.core.platform.to_lowercase().contains("win") => {
                issues.push(blocking(
                    "platform_mismatch",
                    "auto platform windows does not match core platform",
                ));
            }
            AutoPlatform::Windows8
                if !preset.core.platform.to_lowercase().contains("win")
                    || !preset.core.user_agent.contains("Windows NT 6.2") =>
            {
                issues.push(blocking(
                    "platform_mismatch",
                    "auto platform windows8 does not match user-agent or platform",
                ));
            }
            AutoPlatform::Macos if !preset.core.platform.to_lowercase().contains("mac") => {
                issues.push(blocking(
                    "platform_mismatch",
                    "auto platform macos does not match core platform",
                ));
            }
            AutoPlatform::Linux if !preset.core.platform.to_lowercase().contains("linux") => {
                issues.push(blocking(
                    "platform_mismatch",
                    "auto platform linux does not match core platform",
                ));
            }
            AutoPlatform::Debian
                if !preset.core.platform.to_lowercase().contains("linux")
                    || !preset.core.user_agent.to_lowercase().contains("debian") =>
            {
                issues.push(blocking(
                    "platform_mismatch",
                    "auto platform debian does not match user-agent or platform",
                ));
            }
            AutoPlatform::Ubuntu
                if !preset.core.platform.to_lowercase().contains("linux")
                    || !preset.core.user_agent.to_lowercase().contains("ubuntu") =>
            {
                issues.push(blocking(
                    "platform_mismatch",
                    "auto platform ubuntu does not match user-agent or platform",
                ));
            }
            AutoPlatform::Ios
                if {
                    let user_agent = preset.core.user_agent.to_lowercase();
                    !(user_agent.contains("iphone")
                        || user_agent.contains("ipad")
                        || user_agent.contains("cpu iphone os")
                        || user_agent.contains("cpu os"))
                } =>
            {
                issues.push(blocking(
                    "platform_mismatch",
                    "auto platform ios does not match user-agent",
                ));
            }
            AutoPlatform::Android if !preset.core.user_agent.to_lowercase().contains("android") => {
                issues.push(blocking(
                    "platform_mismatch",
                    "auto platform android does not match user-agent",
                ));
            }
            _ => {}
        }
    }

    if preset.screen.device_pixel_ratio > 3.0 && preset.screen.width > 2500 {
        issues.push(warning(
            "unusual_dpr_resolution",
            "high DPR combined with very wide screen may look suspicious",
        ));
    }

    if matches!(
        preset.auto_platform,
        Some(
            AutoPlatform::Windows
                | AutoPlatform::Windows8
                | AutoPlatform::Macos
                | AutoPlatform::Linux
                | AutoPlatform::Debian
                | AutoPlatform::Ubuntu
        )
    ) {
        if preset.screen.width < 1024 || preset.screen.height < 720 {
            issues.push(blocking(
                "desktop_screen_mismatch",
                "desktop auto platform has a phone-like screen size",
            ));
        }
        if preset.hardware.max_touch_points > 2 {
            issues.push(blocking(
                "desktop_touch_mismatch",
                "desktop auto platform exposes too many touch points",
            ));
        }
        if preset.hardware.device_memory_gb < 4 {
            issues.push(blocking(
                "desktop_memory_mismatch",
                "desktop auto platform has unrealistically low device memory",
            ));
        }
    }

    if matches!(preset.auto_platform, Some(AutoPlatform::Ios))
        && (preset.screen.width < 750 || preset.screen.height < 1024)
    {
        issues.push(warning(
            "ios_screen_unusual",
            "ios auto platform has an unusual screen geometry",
        ));
    }

    if matches!(preset.auto_platform, Some(AutoPlatform::Android))
        && (preset.screen.width > 1600 || preset.screen.height < 1800)
    {
        issues.push(warning(
            "android_screen_unusual",
            "android auto platform has an unusual screen geometry",
        ));
    }

    if preset.geo.accuracy_meters > 5000.0 {
        issues.push(warning(
            "low_geo_accuracy",
            "very low geolocation accuracy may conflict with stable identity",
        ));
    }

    if let Some(route) = active_route {
        if route.eq_ignore_ascii_case("tor") && preset.locale.timezone_iana == "UTC" {
            issues.push(warning(
                "tor_utc_timezone",
                "TOR route with static UTC timezone may be fingerprintable",
            ));
        }
    }

    if preset.audio.max_channels > 8 && preset.hardware.device_memory_gb < 2 {
        issues.push(blocking(
            "hardware_audio_conflict",
            "audio channels too high for low memory device profile",
        ));
    }

    issues
}

fn warning(code: &str, message: &str) -> ConsistencyIssue {
    ConsistencyIssue {
        level: ConsistencyLevel::Warning,
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn blocking(code: &str, message: &str) -> ConsistencyIssue {
    ConsistencyIssue {
        level: ConsistencyLevel::Blocking,
        code: code.to_string(),
        message: message.to_string(),
    }
}
