use super::*;

pub(crate) fn collect_keepassxc_allowed_origins(
    state: &AppState,
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Vec<String> {
    let mut origins = BTreeSet::new();
    origins.insert(KEEPASSXC_STORE_EXTENSION_ORIGIN.to_string());
    for origin in keepassxc_profile_store_origins(state, profile) {
        origins.insert(origin);
    }
    for origin in read_keepassxc_origins_from_secure_preferences(state, profile, profile_root) {
        origins.insert(origin);
    }
    origins.into_iter().collect()
}

#[cfg(target_os = "windows")]
pub(crate) fn keepassxc_profile_store_origins(state: &AppState, profile: &ProfileMetadata) -> Vec<String> {
    let store = match state.profile_extension_store.lock() {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let Some(set) = store.profiles.get(&profile.id.to_string()) else {
        return Vec::new();
    };
    set.items
        .values()
        .filter(|item| item.enabled)
        .filter(|item| looks_like_keepassxc_library_item_id(&item.library_item_id))
        .filter_map(|item| item.browser_extension_id.as_deref())
        .map(|extension_id| format!("chrome-extension://{extension_id}/"))
        .collect()
}

#[cfg(target_os = "windows")]
pub(crate) fn collect_keepassxc_firefox_extension_ids(
    state: &AppState,
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    ids.insert(KEEPASSXC_FIREFOX_EXTENSION_ID.to_string());
    let store = match state.profile_extension_store.lock() {
        Ok(value) => value,
        Err(_) => return ids.into_iter().collect(),
    };
    if let Some(set) = store.profiles.get(&profile.id.to_string()) {
        for item in set.items.values() {
            if !item.engine_scope.trim().eq_ignore_ascii_case("firefox") || !item.enabled {
                continue;
            }
            if !looks_like_keepassxc_library_item_id(&item.library_item_id)
                && item.browser_extension_id.as_deref() != Some(KEEPASSXC_FIREFOX_EXTENSION_ID)
            {
                continue;
            }
            if let Some(extension_id) = item.browser_extension_id.clone() {
                ids.insert(extension_id);
            } else if item.library_item_id.contains('@') {
                ids.insert(item.library_item_id.clone());
            } else if let Some(extension_id) = read_firefox_extension_id_from_profile_package(
                item.profile_package_path.as_deref(),
                profile_root,
            ) {
                ids.insert(extension_id);
            }
        }
    }
    ids.into_iter().collect()
}

#[cfg(target_os = "windows")]
pub(crate) fn read_keepassxc_origins_from_secure_preferences(
    state: &AppState,
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Vec<String> {
    let secure_preferences_path = profile_root
        .join("engine-profile")
        .join("Default")
        .join("Secure Preferences");
    let keepassxc_paths = resolve_keepassxc_extension_paths(profile_root);
    if !secure_preferences_path.exists() {
        write_keepassxc_log(
            state,
            profile,
            &format!(
                "KeePassXC secure preferences were not found yet at {}",
                secure_preferences_path.display()
            ),
        );
        return Vec::new();
    }
    if keepassxc_paths.is_empty() {
        write_keepassxc_log(
            state,
            profile,
            &format!(
                "KeePassXC extension directory was not found under {}",
                profile_root
                    .join("extensions")
                    .join("managed")
                    .join("chromium-unpacked")
                    .display()
            ),
        );
        return Vec::new();
    }

    let text = match fs::read_to_string(&secure_preferences_path) {
        Ok(value) => value,
        Err(error) => {
            write_keepassxc_log(
                state,
                profile,
                &format!(
                    "Failed to read KeePassXC secure preferences {}: {}",
                    secure_preferences_path.display(),
                    error
                ),
            );
            return Vec::new();
        }
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            write_keepassxc_log(
                state,
                profile,
                &format!(
                    "Failed to parse KeePassXC secure preferences {}: {}",
                    secure_preferences_path.display(),
                    error
                ),
            );
            return Vec::new();
        }
    };
    let expected_paths = keepassxc_paths
        .iter()
        .map(|path| normalize_windowsish_path(path))
        .collect::<BTreeSet<_>>();
    let mut origins = Vec::new();
    if let Some(settings) = value
        .get("extensions")
        .and_then(|value| value.get("settings"))
        .and_then(Value::as_object)
    {
        for (extension_id, item) in settings {
            let Some(path) = item.get("path").and_then(Value::as_str) else {
                continue;
            };
            if expected_paths.contains(&normalize_windowsish_path(Path::new(path))) {
                let origin = format!("chrome-extension://{extension_id}/");
                write_keepassxc_log(
                    state,
                    profile,
                    &format!(
                        "Discovered KeePassXC runtime origin {} from {}",
                        origin,
                        secure_preferences_path.display()
                    ),
                );
                origins.push(origin);
            }
        }
    }
    if origins.is_empty() {
        write_keepassxc_log(
            state,
            profile,
            &format!(
                "KeePassXC runtime origin was not found in {} for paths {:?}",
                secure_preferences_path.display(),
                keepassxc_paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
            ),
        );
    }
    origins
}

#[cfg(target_os = "windows")]
pub(crate) fn resolve_keepassxc_extension_paths(profile_root: &Path) -> Vec<PathBuf> {
    let extensions_root = profile_root
        .join("extensions")
        .join("managed")
        .join("chromium-unpacked");
    let mut paths = Vec::new();
    let store_id_dir = extensions_root.join(KEEPASSXC_STORE_EXTENSION_ID);
    if store_id_dir.is_dir() {
        paths.push(store_id_dir);
    }
    let read_dir = match fs::read_dir(&extensions_root) {
        Ok(entries) => entries,
        Err(_) => return paths,
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() || paths.iter().any(|candidate| candidate == &path) {
            continue;
        }
        if directory_looks_like_keepassxc(&path) {
            paths.push(path);
        }
    }
    paths
}

#[cfg(target_os = "windows")]
pub(crate) fn directory_looks_like_keepassxc(path: &Path) -> bool {
    let manifest_path = path.join("manifest.json");
    let Ok(text) = fs::read_to_string(manifest_path) else {
        return path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase().contains("keepassxc"))
            .unwrap_or(false);
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    manifest
        .get("name")
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase().contains("keepassxc"))
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn manifest_stable_id(manifest_json: &Value) -> Option<String> {
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

#[cfg(target_os = "windows")]
pub(crate) fn normalize_windowsish_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}

#[cfg(target_os = "windows")]
pub(crate) fn keepassxc_engine_label(engine: &Engine) -> &'static str {
    match engine {
        Engine::Chromium => "Chromium",
        Engine::UngoogledChromium => "Ungoogled Chromium",
        Engine::FirefoxEsr => "Firefox ESR",
        Engine::Librewolf => "LibreWolf",
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn looks_like_keepassxc_library_item_id(value: &str) -> bool {
    value.eq_ignore_ascii_case(KEEPASSXC_STORE_EXTENSION_ID)
        || value.eq_ignore_ascii_case(KEEPASSXC_FIREFOX_EXTENSION_ID)
        || value.to_ascii_lowercase().contains("keepassxc")
}

#[cfg(target_os = "windows")]
pub(crate) fn read_firefox_extension_id_from_profile_package(
    package_path: Option<&str>,
    profile_root: &Path,
) -> Option<String> {
    let _ = profile_root;
    let package_path = package_path?;
    let bytes = fs::read(package_path).ok()?;
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).ok()?;
    let mut manifest = archive.by_name("manifest.json").ok()?;
    let mut text = String::new();
    manifest.read_to_string(&mut text).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value
        .get("browser_specific_settings")
        .and_then(|value| value.get("gecko"))
        .and_then(|value| value.get("id"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| {
            value
                .get("applications")
                .and_then(|value| value.get("gecko"))
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn keepassxc_manifest_debug_summary(manifest: &Value) -> String {
    if let Some(values) = manifest.get("allowed_extensions").and_then(Value::as_array) {
        return format!("allowed_extensions={values:?}");
    }
    if let Some(values) = manifest.get("allowed_origins").and_then(Value::as_array) {
        return format!("allowed_origins={values:?}");
    }
    "no explicit extension list".to_string()
}

#[cfg(target_os = "windows")]

#[path = "keepassxc_bridge_helpers_core_system.rs"]
mod system;
pub(crate) use system::*;


