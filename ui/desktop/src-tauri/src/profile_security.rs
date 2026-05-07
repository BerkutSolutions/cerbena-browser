use browser_profile::ProfileMetadata;

pub const TAG_SYSTEM_ACCESS: &str = "ext-system-access";
pub const TAG_KEEPASSXC: &str = "ext-keepassxc";
pub const TAG_DISABLE_EXTENSIONS_LAUNCH: &str = "ext-launch-disabled";
const TAG_PRIVATE: &str = "private";

pub const ERR_LOCKED_REQUIRES_UNLOCK: &str = "profile_protection.locked_profile_requires_unlock";
pub const ERR_SYSTEM_ACCESS_FORBIDDEN: &str = "profile_protection.system_access_forbidden";
pub const ERR_KEEPASSXC_FORBIDDEN: &str = "profile_protection.keepassxc_forbidden";
pub const ERR_MAXIMUM_POLICY_EXTENSIONS_FORBIDDEN: &str =
    "profile_protection.maximum_policy_extensions_forbidden";
pub const ERR_COOKIES_COPY_BLOCKED: &str = "profile_protection.cookies_copy_blocked";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileProtectionAssessment {
    pub protected_profile: bool,
    pub runtime_hardening: bool,
    pub policy_level: String,
    pub enabled_extensions: Vec<String>,
    pub allow_system_access: bool,
    pub allow_keepassxc: bool,
    pub disable_extensions_launch: bool,
    pub deny_reasons: Vec<&'static str>,
}

fn has_tag(tags: &[String], expected: &str) -> bool {
    tags.iter().any(|tag| tag.eq_ignore_ascii_case(expected))
}

pub fn tags_allow_system_access(tags: &[String]) -> bool {
    has_tag(tags, TAG_SYSTEM_ACCESS)
}

pub fn tags_allow_keepassxc(tags: &[String]) -> bool {
    has_tag(tags, TAG_KEEPASSXC)
}

pub fn tags_disable_extension_launch(tags: &[String]) -> bool {
    has_tag(tags, TAG_DISABLE_EXTENSIONS_LAUNCH)
}

pub fn assess_profile(profile: &ProfileMetadata) -> ProfileProtectionAssessment {
    let policy_level = profile
        .tags
        .iter()
        .find_map(|tag| tag.strip_prefix("policy:"))
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "normal".to_string());
    let enabled_extensions = profile
        .tags
        .iter()
        .filter_map(|tag| tag.strip_prefix("ext:").map(str::to_string))
        .collect::<Vec<_>>();
    let allow_system_access = tags_allow_system_access(&profile.tags);
    let allow_keepassxc = tags_allow_keepassxc(&profile.tags);
    let disable_extensions_launch = tags_disable_extension_launch(&profile.tags);
    let private_tag = profile
        .tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case(TAG_PRIVATE));
    let protected_profile = profile.password_lock_enabled
        || profile.ephemeral_mode
        || private_tag
        || matches!(policy_level.as_str(), "high" | "maximum");

    let mut deny_reasons = Vec::new();
    if protected_profile && allow_system_access {
        deny_reasons.push(ERR_SYSTEM_ACCESS_FORBIDDEN);
    }
    if protected_profile && allow_keepassxc {
        deny_reasons.push(ERR_KEEPASSXC_FORBIDDEN);
    }
    if policy_level == "maximum" && !enabled_extensions.is_empty() {
        deny_reasons.push(ERR_MAXIMUM_POLICY_EXTENSIONS_FORBIDDEN);
    }

    ProfileProtectionAssessment {
        protected_profile,
        runtime_hardening: protected_profile,
        policy_level,
        enabled_extensions,
        allow_system_access,
        allow_keepassxc,
        disable_extensions_launch,
        deny_reasons,
    }
}

pub fn first_launch_blocker(profile: &ProfileMetadata) -> Option<&'static str> {
    assess_profile(profile).deny_reasons.into_iter().next()
}

pub fn cookies_copy_allowed(source: &ProfileMetadata, target: &ProfileMetadata) -> bool {
    !assess_profile(source).protected_profile && !assess_profile(target).protected_profile
}

#[cfg(test)]
mod tests {
    use super::{
        assess_profile, cookies_copy_allowed, ERR_KEEPASSXC_FORBIDDEN,
        ERR_MAXIMUM_POLICY_EXTENSIONS_FORBIDDEN, ERR_SYSTEM_ACCESS_FORBIDDEN,
    };
    use browser_profile::{Engine, ProfileMetadata, ProfileState};
    use uuid::Uuid;

    fn sample_profile(tags: &[&str]) -> ProfileMetadata {
        ProfileMetadata {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            description: None,
            tags: tags.iter().map(|value| value.to_string()).collect(),
            engine: Engine::Chromium,
            state: ProfileState::Ready,
            default_start_page: None,
            default_search_provider: None,
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: Vec::new(),
            crypto_version: 1,
            ephemeral_retain_paths: Vec::new(),
            created_at: "0Z".to_string(),
            updated_at: "0Z".to_string(),
        }
    }

    #[test]
    fn protected_profile_rejects_system_access_capabilities() {
        let mut profile = sample_profile(&["policy:high", "ext-system-access", "ext-keepassxc"]);
        profile.password_lock_enabled = true;
        let assessment = assess_profile(&profile);
        assert!(assessment.protected_profile);
        assert!(assessment
            .deny_reasons
            .contains(&ERR_SYSTEM_ACCESS_FORBIDDEN));
        assert!(assessment.deny_reasons.contains(&ERR_KEEPASSXC_FORBIDDEN));
    }

    #[test]
    fn maximum_policy_blocks_enabled_extensions() {
        let profile = sample_profile(&["policy:maximum", "ext:uBlock"]);
        let assessment = assess_profile(&profile);
        assert!(assessment
            .deny_reasons
            .contains(&ERR_MAXIMUM_POLICY_EXTENSIONS_FORBIDDEN));
    }

    #[test]
    fn cookie_copy_is_blocked_for_protected_profiles() {
        let source = sample_profile(&["policy:normal"]);
        let target = sample_profile(&["policy:high"]);
        assert!(!cookies_copy_allowed(&source, &target));
    }
}
