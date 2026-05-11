use super::*;

#[allow(dead_code)]
pub(crate) fn profile_runtime_has_session_state_impl(engine: EngineKind, profile_dir: &Path) -> bool {
    match engine {
        EngineKind::Chromium | EngineKind::UngoogledChromium => {
            chromium_profile_has_session_state_impl(profile_dir)
        }
        EngineKind::FirefoxEsr | EngineKind::Librewolf => {
            librewolf_profile_has_session_restore_preference_impl(profile_dir)
        }
    }
}

pub(crate) fn librewolf_profile_has_session_restore_preference_impl(profile_dir: &Path) -> bool {
    let prefs_path = profile_dir.join("prefs.js");
    let prefs_restore = fs::read_to_string(&prefs_path)
        .ok()
        .map(|content| {
            content.lines().map(str::trim).any(|line| {
                if line.contains("browser.sessionstore.resume_session_once") && line.contains("true")
                {
                    return true;
                }
                if line.contains("browser.startup.page")
                    && (line.contains(", 3)") || line.contains(",3);") || line.contains(",3)"))
                {
                    return true;
                }
                if line.contains("browser.startup.couldRestoreSession.count") {
                    return !line.contains(", 0)") && !line.contains(",0)") && !line.contains(", 0);");
                }
                false
            })
        })
        .unwrap_or(false);
    if prefs_restore {
        return true;
    }
    let checkpoints_path = profile_dir.join("sessionCheckpoints.json");
    fs::read_to_string(&checkpoints_path)
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .and_then(|value| value.get("sessionstore-windows-restored").and_then(|value| value.as_bool()))
        .unwrap_or(false)
}

#[allow(dead_code)]
pub(crate) fn load_identity_preset_for_profile_impl(
    state: &AppState,
    profile_id: Uuid,
) -> Option<IdentityPreset> {
    let key = profile_id.to_string();
    state
        .identity_store
        .lock()
        .ok()
        .and_then(|store| store.items.get(&key).cloned())
}

#[allow(dead_code)]
pub(crate) fn normalize_start_page_url_impl(default_start_page: Option<&str>) -> String {
    normalize_optional_start_page_url_impl(default_start_page)
        .unwrap_or_else(|| "https://duckduckgo.com".to_string())
}

pub(crate) fn map_search_provider_to_firefox_engine_impl(
    provider: Option<&str>,
) -> Option<&'static str> {
    match provider.unwrap_or("duckduckgo").to_lowercase().as_str() {
        "duckduckgo" => Some("DuckDuckGo"),
        "google" => Some("Google"),
        "bing" => Some("Bing"),
        "yandex" => Some("Yandex"),
        "brave" => Some("Brave"),
        "ecosia" => Some("Ecosia"),
        "startpage" => Some("Startpage"),
        _ => Some("DuckDuckGo"),
    }
}

pub(crate) fn sanitize_librewolf_runtime_prefs_impl(profile_dir: &Path) -> Result<(), std::io::Error> {
    let prefs_path = profile_dir.join("prefs.js");
    if !prefs_path.exists() {
        return Ok(());
    }
    let current = fs::read_to_string(&prefs_path)?;
    let filtered = current
        .lines()
        .filter(|line| {
            !line.contains("browser.newtab.url")
                && !line.contains("browser.startup.homepage")
                && !line.contains("browser.startup.couldRestoreSession.count")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let normalized = if filtered.is_empty() {
        String::new()
    } else {
        format!("{filtered}\n")
    };
    if normalized != current {
        fs::write(&prefs_path, normalized)?;
    }
    Ok(())
}

pub(crate) fn normalize_librewolf_sessionstore_impl(profile_dir: &Path) -> Result<(), std::io::Error> {
    let mut targets = vec![profile_dir.join("sessionstore.jsonlz4")];
    let backups_dir = profile_dir.join("sessionstore-backups");
    if let Ok(entries) = fs::read_dir(&backups_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(|value| {
                        let lower = value.to_ascii_lowercase();
                        lower.ends_with(".jsonlz4")
                            || lower.contains(".jsonlz4-")
                            || lower.ends_with(".baklz4")
                    })
                    .unwrap_or(false)
            {
                targets.push(path);
            }
        }
    }
    for sessionstore_path in targets {
        if !sessionstore_path.exists() {
            continue;
        }
        let Some(mut session_json) = read_mozlz4_json_impl(&sessionstore_path) else {
            eprintln!(
                "[profile-launch] librewolf sessionstore normalize skipped path={} reason=decode_failed",
                sessionstore_path.display()
            );
            continue;
        };
        let Some(windows) = session_json
            .get_mut("windows")
            .and_then(serde_json::Value::as_array_mut)
        else {
            eprintln!(
                "[profile-launch] librewolf sessionstore normalize skipped path={} reason=no_windows_array",
                sessionstore_path.display()
            );
            continue;
        };
        if windows.len() <= 1 {
            eprintln!(
                "[profile-launch] librewolf sessionstore normalize kept path={} windows={}",
                sessionstore_path.display(),
                windows.len()
            );
            continue;
        }
        let previous_window_count = windows.len();
        windows.truncate(1);
        session_json["selectedWindow"] = serde_json::Value::from(1);
        write_mozlz4_json_impl(&sessionstore_path, &session_json)?;
        eprintln!(
            "[profile-launch] librewolf sessionstore normalized path={} windows_before={} windows_after=1",
            sessionstore_path.display(),
            previous_window_count
        );
    }
    Ok(())
}

pub(crate) fn read_mozlz4_json_impl(path: &Path) -> Option<serde_json::Value> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 12 || &bytes[..8] != b"mozLz40\0" {
        return None;
    }
    let uncompressed_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let payload = &bytes[12..];
    let decompressed = lz4::block::decompress(payload, Some(uncompressed_size as i32)).ok()?;
    serde_json::from_slice::<serde_json::Value>(&decompressed).ok()
}

pub(crate) fn write_mozlz4_json_impl(path: &Path, value: &serde_json::Value) -> Result<(), std::io::Error> {
    let json_bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    let compressed =
        lz4::block::compress(&json_bytes, None, false).map_err(std::io::Error::other)?;
    let mut encoded = Vec::with_capacity(12 + compressed.len());
    encoded.extend_from_slice(b"mozLz40\0");
    encoded.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    encoded.extend_from_slice(&compressed);
    fs::write(path, encoded)
}

pub(crate) fn clear_stale_librewolf_lock_files_impl(profile_dir: &Path) -> Result<Vec<String>, std::io::Error> {
    if !crate::process_tracking::find_profile_process_pids_for_dir(profile_dir).is_empty() {
        return Ok(Vec::new());
    }
    let lock_paths = [
        profile_dir.join("parent.lock"),
        profile_dir.join(".parentlock"),
        profile_dir.join("lock"),
    ];
    let mut removed = Vec::new();
    for path in lock_paths {
        if !path.exists() {
            continue;
        }
        fs::remove_file(&path)?;
        removed.push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
    }
    Ok(removed)
}

pub(crate) fn clear_stale_librewolf_search_state_impl(
    profile_dir: &Path,
    default_search_provider: Option<&str>,
) -> Result<Vec<String>, std::io::Error> {
    if default_search_provider
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return Ok(Vec::new());
    }
    let search_state_paths = [
        profile_dir.join("search.json.mozlz4"),
        profile_dir.join("search.sqlite"),
        profile_dir.join("search.sqlite-wal"),
        profile_dir.join("search.sqlite-shm"),
    ];
    let mut removed = Vec::new();
    for path in search_state_paths {
        if !path.exists() {
            continue;
        }
        fs::remove_file(&path)?;
        removed.push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
    }
    Ok(removed)
}

pub(crate) fn prune_librewolf_restore_backups_impl(profile_dir: &Path) -> Result<(), std::io::Error> {
    let sessionstore_path = profile_dir.join("sessionstore.jsonlz4");
    if !sessionstore_path.exists() {
        return Ok(());
    }
    let backups_dir = profile_dir.join("sessionstore-backups");
    if !backups_dir.exists() {
        return Ok(());
    }
    let mut removed = Vec::new();
    if let Ok(entries) = fs::read_dir(&backups_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let lower = name.to_ascii_lowercase();
            let should_remove = lower == "previous.jsonlz4"
                || lower == "recovery.jsonlz4"
                || lower == "recovery.baklz4";
            if !should_remove {
                continue;
            }
            if fs::remove_file(&path).is_ok() {
                removed.push(name.to_string());
            }
        }
    }
    if !removed.is_empty() {
        removed.sort();
        eprintln!(
            "[profile-launch] librewolf pruned restore backups dir={} files={:?}",
            backups_dir.display(),
            removed
        );
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn chromium_profile_has_session_state_impl(profile_dir: &Path) -> bool {
    let default_dir = profile_dir.join("Default");
    [
        default_dir.join("Current Session"),
        default_dir.join("Current Tabs"),
        default_dir.join("Last Session"),
        default_dir.join("Last Tabs"),
    ]
    .into_iter()
    .any(|path| path.is_file())
        || default_dir.join("Sessions").is_dir()
            && fs::read_dir(default_dir.join("Sessions"))
                .ok()
                .into_iter()
                .flat_map(|entries| entries.flatten())
                .any(|entry| entry.path().is_file())
}

pub(crate) fn apply_librewolf_identity_prefs_impl(
    user_js_lines: &mut Vec<String>,
    identity_preset: Option<&IdentityPreset>,
) {
    let Some(identity_preset) = identity_preset else {
        return;
    };
    let user_agent = identity_preset.core.user_agent.trim();
    if !user_agent.is_empty()
        && !matches!(
            identity_preset.mode,
            browser_fingerprint::IdentityPresetMode::Real
        )
    {
        user_js_lines.push(format!(
            "user_pref(\"general.useragent.override\", \"{}\");",
            escape_firefox_pref_string_impl(user_agent)
        ));
    }
    let language = identity_preset.locale.navigator_language.trim();
    if !language.is_empty() {
        user_js_lines.push(format!(
            "user_pref(\"intl.locale.requested\", \"{}\");",
            escape_firefox_pref_string_impl(language)
        ));
    }
    let accept_languages = identity_preset
        .locale
        .languages
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if !accept_languages.is_empty() {
        user_js_lines.push(format!(
            "user_pref(\"intl.accept_languages\", \"{}\");",
            escape_firefox_pref_string_impl(&accept_languages.join(","))
        ));
    }
    user_js_lines.push("user_pref(\"privacy.spoof_english\", 0);".to_string());
}

pub(crate) fn escape_firefox_pref_string_impl(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn normalize_optional_start_page_url_impl(default_start_page: Option<&str>) -> Option<String> {
    let raw = default_start_page
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if raw.contains("://")
        || raw.starts_with("about:")
        || raw.starts_with("chrome:")
        || raw.starts_with("file:")
        || raw.starts_with("data:")
    {
        return Some(raw.to_string());
    }
    Some(format!("https://{raw}"))
}

