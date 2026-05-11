use super::*;

const SUPPORTED_LINK_TYPES: &[(&str, &str, bool)] = &[
    ("http", "links.type.http", true),
    ("https", "links.type.https", true),
    ("ftp", "links.type.ftp", false),
    ("mailto", "links.type.mailto", false),
    ("irc", "links.type.irc", false),
    ("mms", "links.type.mms", false),
    ("news", "links.type.news", false),
    ("nntp", "links.type.nntp", false),
    ("sms", "links.type.sms", false),
    ("smsto", "links.type.smsto", false),
    ("snews", "links.type.snews", false),
    ("tel", "links.type.tel", false),
    ("urn", "links.type.urn", false),
    ("webcal", "links.type.webcal", false),
    ("magnet", "links.type.magnet", false),
    ("tg", "links.type.tg", false),
    ("discord", "links.type.discord", false),
    ("slack", "links.type.slack", false),
    ("zoommtg", "links.type.zoommtg", false),
    ("file:mht", "links.type.fileMht", false),
    ("file:mhtml", "links.type.fileMhtml", false),
    ("file:pdf", "links.type.filePdf", false),
    ("file:shtml", "links.type.fileShtml", false),
    ("file:svg", "links.type.fileSvg", false),
    ("file:xhtml", "links.type.fileXhtml", false),
];

pub(crate) fn supported_link_type_label_key_impl(link_type: &str) -> Option<&'static str> {
    SUPPORTED_LINK_TYPES
        .iter()
        .find(|(key, _, _)| *key == link_type)
        .map(|(_, label_key, _)| *label_key)
}

pub(crate) fn normalize_link_type_impl(link_type: &str) -> Option<String> {
    let normalized = link_type.trim().to_ascii_lowercase();
    if supported_link_type_label_key_impl(&normalized).is_some() {
        Some(normalized)
    } else {
        None
    }
}

pub(crate) fn normalize_file_extension_impl(extension: &str) -> String {
    match extension.trim().to_ascii_lowercase().as_str() {
        "xht" | "xhy" => "xhtml".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn detect_link_type_impl(raw_url: &str) -> Result<String, String> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err("link URL is required".to_string());
    }
    if trimmed.starts_with("--") {
        return Err("CLI flags are not external links".to_string());
    }
    if let Ok(parsed) = reqwest::Url::parse(trimmed) {
        if parsed.scheme().eq_ignore_ascii_case("file") {
            let path = parsed.path().trim();
            if let Some(extension) = std::path::Path::new(path)
                .extension()
                .and_then(|value| value.to_str())
            {
                let file_type = format!("file:{}", normalize_file_extension_impl(extension));
                return normalize_link_type_impl(&file_type)
                    .ok_or_else(|| format!("unsupported link type: .{}", extension));
            }
        }
        return normalize_link_type_impl(parsed.scheme())
            .ok_or_else(|| format!("unsupported link type: {}", parsed.scheme()));
    }
    if let Some(extension) = std::path::Path::new(trimmed)
        .extension()
        .and_then(|value| value.to_str())
    {
        let file_type = format!("file:{}", normalize_file_extension_impl(extension));
        if let Some(normalized) = normalize_link_type_impl(&file_type) {
            return Ok(normalized);
        }
    }
    if !trimmed.contains("://") {
        return Ok("https".to_string());
    }
    Err("invalid link URL".to_string())
}

pub(crate) fn link_routing_overview_impl(state: &AppState) -> Result<LinkRoutingOverview, String> {
    let store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    let manager = state
        .manager
        .lock()
        .map_err(|_| "manager lock poisoned".to_string())?;
    let profile_ids = manager
        .list_profiles()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|profile| profile.id.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let global_profile_id = store
        .global_profile_id
        .clone()
        .filter(|profile_id| profile_ids.contains(profile_id));
    let supported_types = SUPPORTED_LINK_TYPES
        .iter()
        .map(|(link_type, label_key, allow_global_default)| {
            let bound = store
                .type_bindings
                .get(*link_type)
                .cloned()
                .filter(|profile_id| profile_ids.contains(profile_id));
            LinkTypeBindingView {
                link_type: (*link_type).to_string(),
                label_key: (*label_key).to_string(),
                uses_global_default: *allow_global_default
                    && bound.is_none()
                    && global_profile_id.is_some(),
                allow_global_default: *allow_global_default,
                profile_id: bound,
            }
        })
        .collect();
    Ok(LinkRoutingOverview {
        global_profile_id,
        supported_types,
    })
}

pub(crate) fn persist_link_routing_impl(state: &AppState) -> Result<(), String> {
    let path = state.link_routing_store_path(&state.app_handle)?;
    let store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    persist_link_routing_store_with_secret(&path, &state.sensitive_store_secret, &store)
}
