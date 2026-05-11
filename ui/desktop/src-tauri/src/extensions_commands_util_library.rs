use super::*;

pub(crate) fn normalize_tags_impl(tags: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    let mut seen = BTreeSet::new();
    for value in tags {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = trimmed.to_ascii_lowercase();
        if seen.insert(normalized) {
            unique.push(trimmed.to_string());
        }
    }
    unique
}

pub(crate) fn persist_library_impl(
    state: &AppState,
    store: &ExtensionLibraryStore,
) -> Result<(), String> {
    let path = state.extension_library_path(&state.app_handle)?;
    persist_extension_library_store(&path, store)
}

pub(crate) fn build_extension_id_impl(seed: &str, library: &ExtensionLibraryStore) -> String {
    let mut base = seed
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if base.is_empty() {
        base = "extension".to_string();
    }
    let mut candidate = base.clone();
    let mut index = 2u32;
    while library.items.contains_key(&candidate) {
        candidate = format!("{base}-{index}");
        index += 1;
    }
    candidate
}

pub(crate) fn infer_extension_name_impl(store_url: Option<&str>, source_value: &str) -> String {
    let seed = store_url.unwrap_or(source_value);
    seed.rsplit('/')
        .find(|segment| !segment.trim().is_empty())
        .map(|segment| segment.replace('-', " ").replace('_', " "))
        .filter(|segment| !segment.trim().is_empty())
        .unwrap_or_else(|| "Extension".to_string())
}

pub(crate) fn infer_engine_scope_impl(store_url: Option<&str>, source_value: &str) -> String {
    let store = store_url.unwrap_or_default().to_lowercase();
    let source = source_value.to_lowercase();
    if store.contains("addons.mozilla.org") || source.ends_with(".xpi") {
        "firefox".to_string()
    } else if store.contains("chromewebstore.google.com")
        || store.contains("chrome.google.com")
        || source.ends_with(".crx")
    {
        "chromium".to_string()
    } else {
        "chromium/firefox".to_string()
    }
}

pub(crate) fn normalize_engine_scope_impl(value: &str) -> String {
    let normalized = value.trim().to_lowercase();
    if normalized == "firefox" {
        "firefox".to_string()
    } else if normalized == "chromium" {
        "chromium".to_string()
    } else {
        "chromium/firefox".to_string()
    }
}

pub(crate) fn normalized_extension_variants_impl(
    item: &ExtensionLibraryItem,
) -> Vec<ExtensionPackageVariant> {
    let mut variants = item
        .package_variants
        .iter()
        .filter_map(|variant| {
            let engine_scope = normalize_engine_scope_impl(&variant.engine_scope);
            let version = variant.version.trim().to_string();
            if engine_scope.is_empty() || version.is_empty() {
                return None;
            }
            Some(ExtensionPackageVariant {
                engine_scope,
                version,
                source_kind: variant.source_kind.trim().to_string(),
                source_value: variant.source_value.trim().to_string(),
                logo_url: variant.logo_url.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
                store_url: variant.store_url.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
                package_path: variant.package_path.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
                package_file_name: variant.package_file_name.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
            })
        })
        .collect::<Vec<_>>();

    if variants.is_empty() {
        variants.push(ExtensionPackageVariant {
            engine_scope: normalize_engine_scope_impl(&item.engine_scope),
            version: item.version.trim().to_string(),
            source_kind: item.source_kind.trim().to_string(),
            source_value: item.source_value.trim().to_string(),
            logo_url: item.logo_url.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
            store_url: item.store_url.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
            package_path: item.package_path.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
            package_file_name: item.package_file_name.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
        });
    }

    variants.sort_by(|left, right| left.engine_scope.cmp(&right.engine_scope));
    variants.dedup_by(|left, right| left.engine_scope == right.engine_scope);
    variants
}

pub(crate) fn package_variant_for_engine_impl(
    item: &ExtensionLibraryItem,
    engine_scope: &str,
) -> Option<ExtensionPackageVariant> {
    let expected = normalize_engine_scope_impl(engine_scope);
    normalized_extension_variants_impl(item)
        .into_iter()
        .find(|variant| normalize_engine_scope_impl(&variant.engine_scope) == expected)
}

pub(crate) fn primary_extension_variant_impl(
    item: &ExtensionLibraryItem,
) -> Option<ExtensionPackageVariant> {
    let variants = normalized_extension_variants_impl(item);
    if variants.is_empty() {
        return None;
    }
    variants
        .iter()
        .find(|variant| normalize_engine_scope_impl(&variant.engine_scope) == "chromium/firefox")
        .cloned()
        .or_else(|| variants.iter().find(|variant| normalize_engine_scope_impl(&variant.engine_scope) == "chromium").cloned())
        .or_else(|| variants.iter().find(|variant| normalize_engine_scope_impl(&variant.engine_scope) == "firefox").cloned())
        .or_else(|| variants.into_iter().next())
}

pub(crate) fn combined_engine_scope_from_variants_impl(variants: &[ExtensionPackageVariant]) -> String {
    let has_chromium = variants
        .iter()
        .any(|variant| normalize_engine_scope_impl(&variant.engine_scope) == "chromium");
    let has_firefox = variants
        .iter()
        .any(|variant| normalize_engine_scope_impl(&variant.engine_scope) == "firefox");
    if has_chromium && has_firefox {
        "chromium/firefox".to_string()
    } else if has_firefox {
        "firefox".to_string()
    } else if has_chromium {
        "chromium".to_string()
    } else {
        "chromium/firefox".to_string()
    }
}

pub(crate) fn sync_extension_item_legacy_fields_impl(item: &mut ExtensionLibraryItem) {
    let variants = normalized_extension_variants_impl(item);
    item.package_variants = variants.clone();
    item.engine_scope = combined_engine_scope_from_variants_impl(&variants);
    if let Some(primary) = primary_extension_variant_impl(item) {
        item.version = primary.version;
        item.source_kind = primary.source_kind;
        item.source_value = primary.source_value;
        item.logo_url = primary.logo_url;
        item.store_url = primary.store_url;
        item.package_path = primary.package_path;
        item.package_file_name = primary.package_file_name;
    }
}

pub(crate) fn normalized_extension_match_name_impl(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn build_package_variant_impl(
    request: &ImportExtensionLibraryRequest,
    metadata: &DerivedExtensionMetadata,
    engine_scope: &str,
    version: &str,
    package_path: Option<String>,
    package_file_name: Option<String>,
    normalized_store_url: Option<&str>,
    logo_url_override: Option<&str>,
) -> ExtensionPackageVariant {
    ExtensionPackageVariant {
        engine_scope: normalize_engine_scope_impl(engine_scope),
        version: version.to_string(),
        source_kind: request.source_kind.clone(),
        source_value: request.source_value.clone(),
        logo_url: logo_url_override
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .or_else(|| metadata.logo_url.clone()),
        store_url: normalized_store_url.map(str::to_string),
        package_path,
        package_file_name,
    }
}

pub(crate) fn find_merge_target_extension_id_impl(
    library_store: &ExtensionLibraryStore,
    display_name: &str,
    requested_engine_scope: &str,
) -> Option<String> {
    let expected_name = normalized_extension_match_name_impl(display_name);
    let expected_scope = normalize_engine_scope_impl(requested_engine_scope);
    if expected_name.is_empty() {
        return None;
    }

    library_store
        .items
        .values()
        .find(|item| {
            normalized_extension_match_name_impl(&item.display_name) == expected_name
                && package_variant_for_engine_impl(item, &expected_scope).is_none()
        })
        .map(|item| item.id.clone())
        .or_else(|| {
            library_store
                .items
                .values()
                .find(|item| normalized_extension_match_name_impl(&item.display_name) == expected_name)
                .map(|item| item.id.clone())
        })
}

pub(crate) fn engine_scope_matches_profile_impl(engine_scope: &str, engine: Engine) -> bool {
    match normalize_engine_scope_impl(engine_scope).as_str() {
        "firefox" => matches!(engine, Engine::Librewolf | Engine::FirefoxEsr),
        "chromium" => engine.is_chromium_family(),
        _ => true,
    }
}

pub(crate) fn validate_assigned_profiles_impl(
    state: &AppState,
    engine_scope: &str,
    assigned_profile_ids: &[String],
) -> Result<(), String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "profile manager lock poisoned".to_string())?;
    for profile_id in assigned_profile_ids {
        let uuid = uuid::Uuid::parse_str(profile_id)
            .map_err(|e| format!("profile id parse failed: {e}"))?;
        let profile = manager
            .get_profile(uuid)
            .map_err(|e| format!("profile lookup failed: {e}"))?;
        if !engine_scope_matches_profile_impl(engine_scope, profile.engine) {
            return Err(format!(
                "extension engine scope `{engine_scope}` is incompatible with profile `{}`",
                profile.name
            ));
        }
    }
    Ok(())
}
