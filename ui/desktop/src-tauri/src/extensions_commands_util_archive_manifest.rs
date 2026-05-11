use super::*;

pub(crate) fn manifest_engine_scope(
    manifest_json: &serde_json::Value,
    file_name: &str,
    store_url: Option<&str>,
) -> Option<String> {
    if manifest_json
        .get("browser_specific_settings")
        .and_then(|value| value.get("gecko"))
        .is_some()
        || manifest_json
            .get("applications")
            .and_then(|value| value.get("gecko"))
            .is_some()
    {
        return Some("firefox".to_string());
    }
    if manifest_json.get("minimum_chrome_version").is_some() {
        return Some("chromium".to_string());
    }
    let lower = file_name.to_lowercase();
    Some(if lower.ends_with(".xpi") {
        "firefox".to_string()
    } else if lower.ends_with(".crx") {
        "chromium".to_string()
    } else if let Some(url) = store_url {
        super::library::infer_engine_scope_impl(Some(url), file_name)
    } else {
        "chromium/firefox".to_string()
    })
}

pub(crate) fn manifest_logo_data_url<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    manifest_json: &serde_json::Value,
) -> Option<String> {
    let icons = manifest_json.get("icons")?.as_object()?;
    let icon_path = icons
        .iter()
        .filter_map(|(size, value)| Some((size.parse::<u32>().ok()?, value.as_str()?)))
        .max_by_key(|(size, _)| *size)
        .map(|(_, value)| value.to_string())?;
    let mut file = zip.by_name(&icon_path).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let icon_path_lower = icon_path.to_lowercase();
    let mime = if icon_path_lower.ends_with(".svg") {
        "image/svg+xml"
    } else if icon_path_lower.ends_with(".jpg") || icon_path_lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if icon_path_lower.ends_with(".webp") {
        "image/webp"
    } else {
        "image/png"
    };
    Some(format!("data:{mime};base64,{}", BASE64_STANDARD.encode(bytes)))
}

pub(crate) fn manifest_localized_string<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    manifest_json: &serde_json::Value,
    raw_value: Option<&str>,
) -> Option<String> {
    let value = raw_value?;
    if !value.starts_with("__MSG_") {
        return Some(value.to_string());
    }
    let key = value
        .strip_prefix("__MSG_")
        .and_then(|item| item.strip_suffix("__"))?;
    for locale in locale_candidates(
        manifest_json
            .get("default_locale")
            .and_then(|item| item.as_str()),
    ) {
        let path = format!("_locales/{locale}/messages.json");
        let Some(raw_messages) = super::archive_io::read_zip_text(zip, &path) else {
            continue;
        };
        let json = serde_json::from_str::<serde_json::Value>(&raw_messages).ok()?;
        if let Some(message) = json
            .get(key)
            .and_then(|item| item.get("message"))
            .and_then(|item| item.as_str())
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
        {
            return Some(message);
        }
    }
    None
}

pub(crate) fn manifest_stable_id(manifest_json: &serde_json::Value) -> Option<String> {
    manifest_json
        .get("browser_specific_settings")
        .and_then(|value| value.get("gecko"))
        .and_then(|value| value.get("id"))
        .and_then(|value| value.as_str())
        .or_else(|| {
            manifest_json
                .get("applications")
                .and_then(|value| value.get("gecko"))
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
        })
        .map(|value| value.to_string())
}

pub(crate) fn locale_candidates(default_locale: Option<&str>) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(locale) = default_locale.map(str::trim).filter(|value| !value.is_empty()) {
        candidates.push(locale.to_string());
        candidates.push(locale.replace('-', "_"));
        candidates.push(locale.replace('_', "-"));
        if let Some((language, _)) = locale.split_once(['-', '_']) {
            candidates.push(language.to_string());
        }
    }
    candidates.push("en".to_string());
    candidates.push("en_US".to_string());
    candidates.push("en-US".to_string());
    candidates.sort();
    candidates.dedup();
    candidates
}

pub(crate) fn infer_package_extension(file_name: &str, store_url: Option<&str>) -> String {
    let lower = file_name.to_lowercase();
    if lower.ends_with(".xpi") {
        return "xpi".to_string();
    }
    if lower.ends_with(".crx") {
        return "crx".to_string();
    }
    if lower.ends_with(".zip") {
        return "zip".to_string();
    }
    if let Some(url) = store_url {
        if url.to_lowercase().contains("addons.mozilla.org") {
            return "xpi".to_string();
        }
        if url.to_lowercase().contains("chromewebstore.google.com")
            || url.to_lowercase().contains("chrome.google.com")
        {
            return "crx".to_string();
        }
    }
    "zip".to_string()
}

pub(crate) fn package_display_name(file_name: &str, package_extension: &str) -> String {
    let fallback = format!("extension.{package_extension}");
    Path::new(file_name)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
}

pub(crate) fn store_url_fallback_id(store_url: Option<&str>) -> Option<String> {
    let url = store_url?;
    super::store::extract_chrome_web_store_id(url)
        .or_else(|| super::store::extract_amo_slug(url))
        .filter(|value| !value.trim().is_empty())
}
