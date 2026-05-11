use super::*;

pub(crate) fn sync_firefox_engine_profile_extensions_impl(
    set: &ProfileExtensionSet,
    profile_root: &Path,
) -> Result<(), String> {
    let runtime_root = profile_root.join("engine-profile").join("extensions");
    fs::create_dir_all(&runtime_root).map_err(|e| format!("create firefox extensions dir: {e}"))?;
    let managed_ids = set
        .items
        .values()
        .filter_map(|item| item.browser_extension_id.clone().map(|id| (id, item.clone())))
        .collect::<BTreeMap<_, _>>();
    for (browser_id, item) in &managed_ids {
        let Some(source_path) = item.profile_package_path.as_deref() else {
            continue;
        };
        let runtime_path = runtime_root.join(format!("{browser_id}.xpi"));
        fs::copy(source_path, &runtime_path)
            .map_err(|e| format!("copy firefox profile extension: {e}"))?;
    }
    for entry in fs::read_dir(&runtime_root).map_err(|e| format!("read firefox extensions dir: {e}"))?
    {
        let entry = entry.map_err(|e| format!("read firefox extension entry: {e}"))?;
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !file_name.ends_with(".xpi") {
            continue;
        }
        let browser_id = file_name.trim_end_matches(".xpi");
        if !managed_ids.contains_key(browser_id) {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

pub(crate) fn cleanup_chromium_external_extension_manifests_impl(
    profile_root: &Path,
    keep_file_names: &BTreeSet<String>,
) -> Result<(), String> {
    let external_root = profile_root.join("engine-profile").join("External Extensions");
    if !external_root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&external_root)
        .map_err(|e| format!("read chromium external extensions dir: {e}"))?
    {
        let entry = entry.map_err(|e| format!("read chromium external extension entry: {e}"))?;
        let path = entry.path();
        if path.is_file() {
            let keep = path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| keep_file_names.contains(value))
                .unwrap_or(false);
            if !keep {
                fs::remove_file(&path)
                    .map_err(|e| format!("remove chromium external extension manifest: {e}"))?;
            }
        }
    }
    Ok(())
}

pub(crate) fn register_chromium_external_manifest_for_item_impl(
    profile_root: &Path,
    item: &ProfileInstalledExtension,
) -> Result<(), String> {
    let Some(browser_id) = item.browser_extension_id.as_deref() else {
        return Ok(());
    };
    let Some(version) = (!item.version.trim().is_empty()).then_some(item.version.trim()) else {
        return Ok(());
    };
    let Some(package_path) = item.profile_package_path.as_deref() else {
        return Ok(());
    };
    let external_root = profile_root.join("engine-profile").join("External Extensions");
    fs::create_dir_all(&external_root)
        .map_err(|e| format!("create chromium external extension root {}: {e}", external_root.display()))?;
    let manifest_path = external_root.join(format!("{browser_id}.json"));
    let payload = serde_json::json!({
        "external_crx": package_path,
        "external_version": version,
    });
    let bytes = serde_json::to_vec_pretty(&payload)
        .map_err(|e| format!("serialize chromium external extension manifest: {e}"))?;
    fs::write(&manifest_path, bytes)
        .map_err(|e| format!("write chromium external extension manifest {}: {e}", manifest_path.display()))?;
    Ok(())
}

pub(crate) fn cleanup_legacy_chromium_extension_root_impl(profile_root: &Path) -> Result<(), String> {
    let legacy_root = profile_root.join("policy").join("chromium-extensions");
    if !legacy_root.exists() {
        return Ok(());
    }
    fs::remove_dir_all(&legacy_root)
        .map_err(|e| format!("remove legacy chromium extension root {}: {e}", legacy_root.display()))
}

pub(crate) fn sanitize_chromium_runtime_extension_state_impl(profile_root: &Path) -> Result<(), String> {
    let secure_preferences_path = profile_root.join("engine-profile").join("Default").join("Secure Preferences");
    if !secure_preferences_path.exists() {
        return Ok(());
    }
    let raw = fs::read_to_string(&secure_preferences_path)
        .map_err(|e| format!("read chromium secure preferences {}: {e}", secure_preferences_path.display()))?;
    let mut value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|e| format!("parse chromium secure preferences {}: {e}", secure_preferences_path.display()))?;
    let Some(settings) = value.get_mut("extensions").and_then(|value| value.get_mut("settings")).and_then(|value| value.as_object_mut()) else {
        return Ok(());
    };
    let legacy_root = profile_root.join("policy").join("chromium-extensions");
    let legacy_prefix = normalize_path_key_impl(legacy_root.to_string_lossy().as_ref());
    let removed_ids = settings
        .iter()
        .filter_map(|(browser_id, item)| {
            let path = item.get("path").and_then(|value| value.as_str()).map(normalize_path_key_impl)?;
            path.starts_with(&legacy_prefix).then(|| browser_id.clone())
        })
        .collect::<Vec<_>>();
    if removed_ids.is_empty() {
        return Ok(());
    }
    settings.retain(|browser_id, _| !removed_ids.iter().any(|value| value == browser_id));
    fs::write(&secure_preferences_path, serde_json::to_vec(&value).map_err(|e| format!("serialize chromium secure preferences {}: {e}", secure_preferences_path.display()))?)
        .map_err(|e| format!("write chromium secure preferences {}: {e}", secure_preferences_path.display()))?;
    cleanup_chromium_extension_runtime_dirs_impl(profile_root, &removed_ids);
    Ok(())
}

pub(crate) fn sync_chromium_store_from_browser_impl(
    state: &AppState,
    profile: &ProfileMetadata,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    let secure_preferences_path = state.profile_root.join(profile.id.to_string()).join("engine-profile").join("Default").join("Secure Preferences");
    if !secure_preferences_path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(&secure_preferences_path)
        .map_err(|e| format!("read chromium secure preferences: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|e| format!("parse chromium secure preferences: {e}"))?;
    let Some(settings) = value.get("extensions").and_then(|value| value.get("settings")).and_then(|value| value.as_object()) else {
        return Ok(false);
    };
    let mut observed_by_path = BTreeMap::new();
    let mut observed_by_id = BTreeMap::new();
    let managed_root = state.profile_root.join(profile.id.to_string()).join("extensions").join("managed").join("chromium-unpacked");
    let legacy_root = state.profile_root.join(profile.id.to_string()).join("policy").join("chromium-extensions");
    let managed_prefix = normalize_path_key_impl(managed_root.to_string_lossy().as_ref());
    let legacy_prefix = normalize_path_key_impl(legacy_root.to_string_lossy().as_ref());
    for (browser_id, item) in settings {
        let Some(path) = item.get("path").and_then(|value| value.as_str()) else {
            continue;
        };
        let enabled = item.get("disable_reasons").and_then(|value| value.as_array()).map(|value| value.is_empty()).unwrap_or(true);
        let normalized = normalize_path_key_impl(path);
        let _ = normalized.starts_with(&managed_prefix) || normalized.starts_with(&legacy_prefix);
        observed_by_path.insert(normalized, (browser_id.clone(), enabled));
        observed_by_id.insert(browser_id.clone(), enabled);
    }
    let mut changed = false;
    let current_ids = set.items.keys().cloned().collect::<Vec<_>>();
    for library_item_id in current_ids {
        let Some(item) = set.items.get_mut(&library_item_id) else {
            continue;
        };
        let matched = item
            .profile_unpacked_path
            .as_deref()
            .and_then(|path| observed_by_path.get(&normalize_path_key_impl(path)).cloned())
            .or_else(|| {
                item.browser_extension_id
                    .as_deref()
                    .and_then(|browser_id| observed_by_id.get(browser_id).copied())
                    .map(|enabled| (item.browser_extension_id.clone().unwrap_or_default(), enabled))
            });
        let Some((browser_id, enabled)) = matched else {
            set.items.remove(&library_item_id);
            changed = true;
            continue;
        };
        if item.browser_extension_id.as_deref() != Some(browser_id.as_str()) {
            item.browser_extension_id = Some(browser_id.clone());
            changed = true;
        }
        if item.enabled != enabled {
            item.enabled = enabled;
            changed = true;
        }
    }
    Ok(changed)
}

pub(crate) fn sync_firefox_store_from_browser_impl(
    state: &AppState,
    profile: &ProfileMetadata,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    let extensions_json_path = state.profile_root.join(profile.id.to_string()).join("engine-profile").join("extensions.json");
    if !extensions_json_path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(&extensions_json_path)
        .map_err(|e| format!("read firefox extensions.json: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|e| format!("parse firefox extensions.json: {e}"))?;
    let Some(addons) = value.get("addons").and_then(|value| value.as_array()) else {
        return Ok(false);
    };
    let mut observed = BTreeMap::new();
    for addon in addons {
        let Some(location) = addon.get("location").and_then(|value| value.as_str()) else {
            continue;
        };
        if location != "app-profile" {
            continue;
        }
        let Some(id) = addon.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        let enabled = addon.get("userDisabled").and_then(|value| value.as_bool()).map(|value| !value).unwrap_or(true);
        observed.insert(id.to_string(), enabled);
    }
    let current_ids = set.items.keys().cloned().collect::<Vec<_>>();
    let mut changed = false;
    for library_item_id in current_ids {
        let Some(item) = set.items.get_mut(&library_item_id) else {
            continue;
        };
        let Some(browser_id) = item.browser_extension_id.as_deref() else {
            continue;
        };
        let Some(enabled) = observed.get(browser_id) else {
            set.items.remove(&library_item_id);
            changed = true;
            continue;
        };
        if item.enabled != *enabled {
            item.enabled = *enabled;
            changed = true;
        }
    }
    Ok(changed)
}

fn normalize_path_key_impl(path: &str) -> String {
    path.replace('/', "\\").trim().to_ascii_lowercase()
}

fn cleanup_chromium_extension_runtime_dirs_impl(profile_root: &Path, removed_ids: &[String]) {
    if removed_ids.is_empty() {
        return;
    }
    let roots = [
        profile_root.join("engine-profile").join("Default").join("Local Extension Settings"),
        profile_root.join("engine-profile").join("Default").join("Sync Extension Settings"),
        profile_root.join("engine-profile").join("Default").join("Managed Extension Settings"),
        profile_root.join("engine-profile").join("Default").join("Extensions"),
    ];
    for root in roots {
        for browser_id in removed_ids {
            let target = root.join(browser_id);
            if target.exists() {
                let _ = fs::remove_dir_all(&target);
            }
        }
    }
}
