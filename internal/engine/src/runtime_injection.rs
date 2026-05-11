use super::*;

pub(super) fn chromium_extension_version_impl(raw: &str) -> String {
    let normalized = raw.trim().trim_start_matches('v');
    let mut parts = Vec::new();
    for segment in normalized.split(['.', '-']) {
        if segment.is_empty() {
            continue;
        }
        if segment.chars().all(|ch| ch.is_ascii_digit()) {
            parts.push(segment.to_string());
        } else {
            break;
        }
        if parts.len() == 4 {
            break;
        }
    }
    if parts.is_empty() {
        "1".to_string()
    } else {
        parts.join(".")
    }
}

pub(super) fn prepare_chromium_blocking_extension_impl(
    profile_root: &Path,
) -> Result<Option<PathBuf>, EngineError> {
    let blocked_domains = blocked_domains_for_profile_impl(profile_root)?;
    let locked_app = load_locked_app_config_impl(profile_root)?;
    let identity = load_identity_launch_policy(profile_root);
    let accept_languages = identity
        .as_ref()
        .map(|policy| {
            normalize_accept_languages(&policy.locale.navigator_language, &policy.locale.languages)
        })
        .unwrap_or_default();
    if blocked_domains.is_empty() && locked_app.is_none() && accept_languages.is_empty() {
        return Ok(None);
    }

    let extension_dir = profile_root
        .join("policy")
        .join("chromium-policy-extension");
    fs::create_dir_all(&extension_dir)?;
    let manifest = serde_json::json!({
        "manifest_version": 3,
        "name": "Cerbena Policy Firewall",
        "version": chromium_extension_version_impl(CHROMIUM_POLICY_EXTENSION_VERSION),
        "description": "Profile-scoped outbound policy enforcement for blocked domains.",
        "declarative_net_request": {
            "rule_resources": [
                {
                    "id": "policy_rules",
                    "enabled": true,
                    "path": "rules.json"
                }
            ]
        },
        "permissions": [
            "declarativeNetRequest",
            "declarativeNetRequestFeedback",
            "declarativeNetRequestWithHostAccess"
        ],
        "host_permissions": ["<all_urls>"]
    });
    let mut rules = Vec::new();
    if !accept_languages.is_empty() {
        let accept_language_header = build_accept_language_header(&accept_languages);
        rules.push(serde_json::json!({
            "id": 1,
            "priority": 1,
            "action": {
                "type": "modifyHeaders",
                "requestHeaders": [
                    {
                        "header": "Accept-Language",
                        "operation": "set",
                        "value": accept_language_header
                    }
                ]
            },
            "condition": {
                "regexFilter": "^https?://",
                "resourceTypes": [
                    "main_frame", "sub_frame", "stylesheet", "script", "image", "font",
                    "object", "xmlhttprequest", "ping", "media", "websocket", "webtransport", "other"
                ]
            }
        }));
    }
    let base_rule_id = rules.len() + 1;
    rules.extend(
        blocked_domains
            .into_iter()
            .enumerate()
            .map(|(index, domain)| {
                serde_json::json!({
                    "id": base_rule_id + index,
                    "priority": 2,
                    "action": { "type": "block" },
                    "condition": {
                        "urlFilter": format!("||{domain}^"),
                        "resourceTypes": [
                            "main_frame", "sub_frame", "stylesheet", "script", "image", "font",
                            "object", "xmlhttprequest", "ping", "media", "websocket", "webtransport", "other"
                        ]
                    }
                })
            }),
    );
    if let Some(config) = locked_app {
        let allowed_hosts = config
            .allowed_hosts
            .into_iter()
            .map(|host| host.trim().trim_start_matches('.').to_lowercase())
            .filter(|host| !host.is_empty())
            .collect::<Vec<_>>();
        if !allowed_hosts.is_empty() {
            rules.push(serde_json::json!({
                "id": rules.len() + 1,
                "priority": 3,
                "action": { "type": "block" },
                "condition": {
                    "regexFilter": "^https?://",
                    "excludedRequestDomains": allowed_hosts,
                    "resourceTypes": [
                        "main_frame", "sub_frame", "stylesheet", "script", "image", "font",
                        "object", "xmlhttprequest", "ping", "media", "websocket", "webtransport", "other"
                    ]
                }
            }));
        }
    }

    fs::write(
        extension_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        extension_dir.join("rules.json"),
        serde_json::to_vec_pretty(&rules)?,
    )?;
    Ok(Some(extension_dir))
}

pub(super) fn load_locked_app_config_impl(
    profile_root: &Path,
) -> Result<Option<LockedAppConfig>, EngineError> {
    let path = profile_root.join("policy").join("locked-app.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read(path)?;
    let config = serde_json::from_slice::<LockedAppConfig>(&raw)?;
    Ok(Some(config))
}

pub(super) fn resolve_locked_app_target_url_impl(
    config: &LockedAppConfig,
    requested_url: &str,
) -> String {
    let trimmed = requested_url.trim();
    if trimmed.is_empty() {
        return config.start_url.clone();
    }
    let Ok(parsed) = reqwest::Url::parse(trimmed) else {
        return config.start_url.clone();
    };
    let Some(host) = parsed.host_str().map(|value| value.to_ascii_lowercase()) else {
        return config.start_url.clone();
    };
    let allowed = config.allowed_hosts.iter().any(|candidate| {
        let normalized = candidate
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        host == normalized || host.ends_with(&format!(".{normalized}"))
    });
    if allowed {
        trimmed.to_string()
    } else {
        config.start_url.clone()
    }
}

pub(super) fn prepare_chromium_extension_dirs_impl(
    profile_root: &Path,
) -> Result<Vec<PathBuf>, EngineError> {
    let mut dirs = Vec::new();
    if let Some(blocking_extension) = prepare_chromium_blocking_extension_impl(profile_root)? {
        dirs.push(blocking_extension);
    }
    let managed_root = profile_root
        .join("extensions")
        .join("managed")
        .join("chromium-unpacked");
    if managed_root.is_dir() {
        let mut discovered = fs::read_dir(managed_root)?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                if !path.is_dir() {
                    return false;
                }
                let external_marker = path.join(".cerbena-prefer-external-manifest");
                !external_marker.is_file()
            })
            .collect::<Vec<_>>();
        discovered.sort();
        dirs.extend(discovered);
    }
    Ok(dirs)
}

pub(super) fn blocked_domains_for_profile_impl(
    profile_root: &Path,
) -> Result<Vec<String>, EngineError> {
    let path = profile_root.join("policy").join("blocked-domains.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read(path)?;
    let domains: Vec<String> = serde_json::from_slice(&raw)?;
    let normalized = domains
        .into_iter()
        .map(|domain| domain.trim().trim_start_matches('.').to_lowercase())
        .filter(|domain| !domain.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(normalized)
}
