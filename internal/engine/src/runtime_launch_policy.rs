use super::*;

pub(crate) fn launch_args_impl(
    engine: EngineKind,
    profile_root: &Path,
    start_page: Option<&str>,
    private_mode: bool,
    gateway_proxy_port: Option<u16>,
    runtime_hardening: bool,
) -> Result<Vec<String>, EngineError> {
    let runtime_dir = profile_root.join("engine-profile");
    match engine {
        EngineKind::Chromium | EngineKind::UngoogledChromium => {
            runtime_launch_chromium::launch_args_chromium_family_impl(
                profile_root,
                start_page,
                private_mode,
                gateway_proxy_port,
                runtime_hardening,
            )
        }
        EngineKind::FirefoxEsr | EngineKind::Librewolf => Ok(
            runtime_launch_firefox::launch_args_firefox_family_impl(
                engine,
                &runtime_dir,
                start_page,
            ),
        ),
    }
}

pub(crate) fn reopen_args_impl(
    engine: EngineKind,
    profile_root: &Path,
    url: &str,
) -> Result<Vec<String>, EngineError> {
    let runtime_dir = profile_root.join("engine-profile");
    Ok(match engine {
        EngineKind::Chromium | EngineKind::UngoogledChromium => {
            runtime_launch_chromium::reopen_args_chromium_family_impl(profile_root, &runtime_dir, url)?
        }
        EngineKind::FirefoxEsr | EngineKind::Librewolf => {
            runtime_launch_firefox::reopen_args_firefox_family_impl(&runtime_dir, url)
        }
    })
}

pub(crate) fn launch_environment_impl(
    engine: EngineKind,
    profile_root: &Path,
) -> Vec<(String, String)> {
    match engine {
        EngineKind::Chromium | EngineKind::UngoogledChromium => {
            chromium_launch_environment_impl(profile_root)
        }
        EngineKind::FirefoxEsr | EngineKind::Librewolf => Vec::new(),
    }
}

pub(crate) fn load_identity_launch_policy_impl(
    profile_root: &Path,
) -> Option<IdentityLaunchPolicy> {
    let path = profile_root.join("policy").join("identity-preset.json");
    let raw = fs::read(path).ok()?;
    serde_json::from_slice(&raw).ok()
}

pub(crate) fn chromium_launch_environment_impl(profile_root: &Path) -> Vec<(String, String)> {
    runtime_launch_chromium::chromium_launch_environment_impl(profile_root)
}

pub(crate) fn first_positive_impl(primary: u32, fallback: u32) -> u32 {
    if primary > 0 {
        primary
    } else {
        fallback
    }
}

pub(crate) fn normalize_primary_language_impl(language: &str) -> Option<String> {
    let trimmed = language.trim().replace('_', "-");
    (!trimmed.is_empty()).then_some(trimmed)
}

pub(crate) fn normalize_accept_languages_impl(primary: &str, languages: &[String]) -> Vec<String> {
    let mut normalized = BTreeSet::new();
    let mut ordered = Vec::new();
    for candidate in std::iter::once(primary).chain(languages.iter().map(String::as_str)) {
        let value = candidate.trim().replace('_', "-");
        if value.is_empty() {
            continue;
        }
        let dedupe_key = value.to_ascii_lowercase();
        if normalized.insert(dedupe_key) {
            ordered.push(value);
        }
    }
    ordered
}

pub(crate) fn identity_uses_native_user_agent_impl(identity: &IdentityLaunchPolicy) -> bool {
    matches!(identity.mode, Some(IdentityLaunchMode::Real))
}

pub(crate) fn build_accept_language_header_impl(languages: &[String]) -> String {
    languages
        .iter()
        .enumerate()
        .map(|(index, language)| {
            if index == 0 {
                language.clone()
            } else {
                let quality = 1.0 - (index as f32 * 0.1);
                let quality = quality.max(0.1);
                format!("{language};q={quality:.1}")
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn write_chromium_language_preferences_impl(
    profile_root: &Path,
    languages: &[String],
) -> Result<(), EngineError> {
    let preferences_path = profile_root
        .join("engine-profile")
        .join("Default")
        .join("Preferences");
    if let Some(parent) = preferences_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut value = if preferences_path.exists() {
        serde_json::from_slice::<serde_json::Value>(&fs::read(&preferences_path)?)?
    } else {
        serde_json::json!({})
    };
    if !value.is_object() {
        value = serde_json::json!({});
    }
    let root = value.as_object_mut().ok_or_else(|| {
        EngineError::Launch("chromium preferences root is not an object".to_string())
    })?;
    let intl = root
        .entry("intl".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !intl.is_object() {
        *intl = serde_json::json!({});
    }
    intl["accept_languages"] = serde_json::Value::String(languages.join(","));
    intl["selected_languages"] = serde_json::Value::String(languages.join(","));
    let bytes = serde_json::to_vec_pretty(&value)?;
    fs::write(preferences_path, bytes)?;
    Ok(())
}

pub(crate) fn write_chromium_local_state_locale_impl(
    profile_root: &Path,
    languages: &[String],
) -> Result<(), EngineError> {
    let local_state_path = profile_root.join("engine-profile").join("Local State");
    if let Some(parent) = local_state_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut value = if local_state_path.exists() {
        serde_json::from_slice::<serde_json::Value>(&fs::read(&local_state_path)?)?
    } else {
        serde_json::json!({})
    };
    if !value.is_object() {
        value = serde_json::json!({});
    }
    let root = value.as_object_mut().ok_or_else(|| {
        EngineError::Launch("chromium local state root is not an object".to_string())
    })?;
    let intl = root
        .entry("intl".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !intl.is_object() {
        *intl = serde_json::json!({});
    }
    intl["app_locale"] = serde_json::Value::String(languages[0].clone());
    intl["selected_languages"] = serde_json::Value::String(languages.join(","));
    let bytes = serde_json::to_vec_pretty(&value)?;
    fs::write(local_state_path, bytes)?;
    Ok(())
}

pub(crate) fn chromium_host_resolver_rules_impl(
    profile_root: &Path,
    max_len: usize,
) -> Option<String> {
    let path = profile_root.join("policy").join("blocked-domains.json");
    let raw = fs::read(path).ok()?;
    let domains: Vec<String> = serde_json::from_slice(&raw).ok()?;
    if domains.is_empty() {
        return None;
    }
    let mut rules = String::new();
    for domain in domains {
        let d = domain.trim();
        if d.is_empty() {
            continue;
        }
        let next_rules = [format!("MAP {d} 0.0.0.0"), format!("MAP *.{d} 0.0.0.0")];
        for rule in next_rules {
            let projected_len = if rules.is_empty() {
                rule.len()
            } else {
                rules.len() + 2 + rule.len()
            };
            if projected_len > max_len {
                return None;
            }
            if !rules.is_empty() {
                rules.push_str(", ");
            }
            rules.push_str(&rule);
        }
    }
    (!rules.is_empty()).then_some(rules)
}
