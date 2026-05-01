use serde::{Deserialize, Serialize};

use crate::errors::ProfileError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileModalPayload {
    pub general: GeneralTab,
    pub identity: IdentityTab,
    pub vpn_proxy: VpnProxyTab,
    pub dns: DnsTab,
    pub extensions: ExtensionsTab,
    pub security: SecurityTab,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralTab {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub default_start_page: Option<String>,
    pub default_search_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityTab {
    pub mode: String,
    pub platform_target: Option<String>,
    #[serde(default)]
    pub template_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnProxyTab {
    pub route_mode: String,
    pub proxy_url: Option<String>,
    pub vpn_profile_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsTab {
    pub resolver_mode: String,
    pub servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionsTab {
    pub enabled_extension_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityTab {
    pub password_lock_enabled: bool,
    pub ephemeral_mode: bool,
    pub ephemeral_retain_paths: Vec<String>,
}

pub fn validate_modal_payload(payload: &ProfileModalPayload) -> Result<(), ProfileError> {
    if payload.general.name.trim().is_empty() {
        return Err(ProfileError::Validation(
            "profile modal: name is required".to_string(),
        ));
    }
    if payload.general.tags.len() > 32 {
        return Err(ProfileError::Validation(
            "profile modal: too many tags".to_string(),
        ));
    }

    let identity_mode = payload.identity.mode.trim().to_ascii_lowercase();
    if !matches!(identity_mode.as_str(), "auto" | "manual") {
        return Err(ProfileError::Validation(
            "profile modal: invalid identity mode".to_string(),
        ));
    }
    if identity_mode == "auto" {
        let Some(platform_target) = payload
            .identity
            .platform_target
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Err(ProfileError::Validation(
                "profile modal: auto identity requires target platform".to_string(),
            ));
        };
        let supported_platforms = [
            "windows", "windows8", "macos", "linux", "debian", "ubuntu", "ios", "android",
        ];
        if !supported_platforms.contains(&platform_target) {
            return Err(ProfileError::Validation(
                "profile modal: unsupported identity target platform".to_string(),
            ));
        }
    }

    let route = payload.vpn_proxy.route_mode.as_str();
    let valid = ["direct", "proxy", "vpn", "tor", "hybrid"];
    if !valid.contains(&route) {
        return Err(ProfileError::Validation(
            "profile modal: invalid route mode".to_string(),
        ));
    }

    if payload.dns.resolver_mode == "custom" && payload.dns.servers.is_empty() {
        return Err(ProfileError::Validation(
            "profile modal: custom DNS requires at least one server".to_string(),
        ));
    }
    Ok(())
}
