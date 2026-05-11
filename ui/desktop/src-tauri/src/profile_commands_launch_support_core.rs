use super::*;

pub(crate) fn emit_profile_launch_progress(
    app_handle: &tauri::AppHandle,
    profile_id: Uuid,
    stage_key: &str,
    message_key: &str,
    done: bool,
    error: Option<&str>,
) {
    let _ = app_handle.emit(
        "profile-launch-progress",
        serde_json::json!({
            "profileId": profile_id.to_string(),
            "stageKey": stage_key,
            "messageKey": message_key,
            "done": done,
            "error": error,
        }),
    );
}

pub(crate) fn wait_for_profile_process_startup_impl(
    user_data_dir: &Path,
    spawned_pid: u32,
    engine: EngineKind,
    prelaunch_profile_pids: &std::collections::BTreeSet<u32>,
) -> Result<u32, String> {
    let startup_timeout = if engine.is_chromium_family() {
        Duration::from_millis(2600)
    } else {
        Duration::from_millis(1400)
    };
    let poll_interval = Duration::from_millis(200);
    let started_at = Instant::now();
    let mut last_seen_pid = spawned_pid;

    while started_at.elapsed() < startup_timeout {
        if matches!(engine, EngineKind::Librewolf) {
            let mut pids =
                crate::process_tracking::find_profile_process_pids_for_dir(user_data_dir);
            if !pids.is_empty() {
                pids.sort_unstable();
                let preferred = pids
                    .iter()
                    .copied()
                    .filter(|pid| *pid != spawned_pid && !prelaunch_profile_pids.contains(pid))
                    .max()
                    .or_else(|| {
                        pids.iter()
                            .copied()
                            .filter(|pid| *pid != spawned_pid)
                            .max()
                    })
                    .or_else(|| pids.iter().copied().find(|pid| *pid == spawned_pid))
                    .or_else(|| pids.last().copied())
                    .unwrap_or(spawned_pid);
                last_seen_pid = preferred;
                let candidates = describe_profile_process_candidates(user_data_dir);
                let main_window_pid = find_profile_main_window_pid_for_dir(user_data_dir);
                if candidates.len() > 1
                    && candidates.iter().any(|value| {
                        value.contains("librewolf")
                            || value.contains("firefox")
                            || value.contains("private_browsing")
                    })
                {
                    eprintln!(
                        "[profile-launch] startup candidate set profile_dir={} spawned_pid={} selected_pid={} main_window_pid={:?} candidates={:?}",
                        user_data_dir.display(),
                        spawned_pid,
                        preferred,
                        main_window_pid,
                        candidates
                    );
                }
                if preferred != spawned_pid && is_pid_running(spawned_pid) {
                    eprintln!(
                        "[profile-launch] librewolf bootstrap pid preserved old_pid={} selected_pid={}",
                        spawned_pid,
                        preferred
                    );
                }
                if preferred == spawned_pid && started_at.elapsed() + poll_interval < startup_timeout {
                    thread::sleep(poll_interval);
                    continue;
                }
                if is_pid_running(preferred) {
                    return Ok(preferred);
                }
            }
        }
        if let Some(actual_pid) = find_profile_main_window_pid_for_dir(user_data_dir)
            .or_else(|| find_profile_process_pid_for_dir(user_data_dir))
        {
            last_seen_pid = actual_pid;
            let candidates = describe_profile_process_candidates(user_data_dir);
            let main_window_pid = find_profile_main_window_pid_for_dir(user_data_dir);
            if candidates.len() > 1
                && candidates.iter().any(|value| {
                    value.contains("librewolf")
                        || value.contains("firefox")
                        || value.contains("private_browsing")
                })
            {
                eprintln!(
                    "[profile-launch] startup candidate set profile_dir={} spawned_pid={} selected_pid={} main_window_pid={:?} candidates={:?}",
                    user_data_dir.display(),
                    spawned_pid,
                    actual_pid,
                    main_window_pid,
                    candidates
                );
            }
            if is_pid_running(actual_pid) {
                return Ok(actual_pid);
            }
        } else if is_pid_running(last_seen_pid) {
            return Ok(last_seen_pid);
        }
        thread::sleep(poll_interval);
    }

    if let Some(actual_pid) = find_profile_main_window_pid_for_dir(user_data_dir)
        .or_else(|| find_profile_process_pid_for_dir(user_data_dir))
    {
        last_seen_pid = actual_pid;
        if is_pid_running(actual_pid) {
            return Ok(actual_pid);
        }
    }
    if is_pid_running(last_seen_pid) {
        return Ok(last_seen_pid);
    }

    Err(format!(
        "Browser process exited during startup pid={last_seen_pid} candidates={:?}",
        describe_profile_process_candidates(user_data_dir)
    ))
}

pub(crate) fn prepare_librewolf_profile_runtime_impl(
    profile_dir: &Path,
    default_start_page: Option<&str>,
    default_search_provider: Option<&str>,
    gateway_proxy_port: Option<u16>,
    runtime_hardening: bool,
    identity_preset: Option<&IdentityPreset>,
) -> Result<(), std::io::Error> {
    runtime_prep::prepare_librewolf_profile_runtime_impl(
        profile_dir,
        default_start_page,
        default_search_provider,
        gateway_proxy_port,
        runtime_hardening,
        identity_preset,
    )
}

pub(crate) fn emit_librewolf_launch_diagnostics(stage: &str, profile_dir: &Path) {
    let user_js_lines = firefox_pref_snapshot(profile_dir.join("user.js"));
    let prefs_js_lines = firefox_pref_snapshot(profile_dir.join("prefs.js"));
    let session_files = librewolf_session_snapshot(profile_dir);
    let session_checkpoints =
        json_file_snapshot(profile_dir.join("sessionCheckpoints.json"), 512);
    let times_json = json_file_snapshot(profile_dir.join("times.json"), 512);
    let restore_signals = librewolf_restore_signal_snapshot(profile_dir);
    let lock_files = [
        profile_dir.join("parent.lock"),
        profile_dir.join(".parentlock"),
        profile_dir.join("lock"),
    ]
    .into_iter()
    .filter(|path| path.exists())
    .map(|path| path.file_name().unwrap_or_default().to_string_lossy().to_string())
    .collect::<Vec<_>>();
    let processes = crate::process_tracking::describe_profile_process_candidates(profile_dir);
    let firefox_processes = crate::process_tracking::describe_firefox_family_processes();
    eprintln!(
        "[profile-launch][librewolf-diagnostics] stage={} profile_dir={} lock_files={:?} session_files={:?} restore_signals={:?} session_checkpoints={} times_json={} user_js={:?} prefs_js={:?} processes={:?} firefox_processes={:?}",
        stage,
        profile_dir.display(),
        lock_files,
        session_files,
        restore_signals,
        session_checkpoints,
        times_json,
        user_js_lines,
        prefs_js_lines,
        processes,
        firefox_processes
    );
}

fn firefox_pref_snapshot(path: PathBuf) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![format!("missing:{}", path.file_name().unwrap_or_default().to_string_lossy())];
    };
    content
        .lines()
        .filter(|line| {
            line.contains("browser.startup.")
                || line.contains("browser.newtab")
                || line.contains("startup.homepage")
                || line.contains("browser.sessionstore")
        })
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn json_file_snapshot(path: PathBuf, max_len: usize) -> String {
    let Ok(content) = std::fs::read_to_string(&path) else {
        return format!(
            "missing:{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
    };
    let compact = content.split_whitespace().collect::<String>();
    if compact.len() <= max_len {
        compact
    } else {
        format!("{}...", &compact[..max_len])
    }
}

fn librewolf_restore_signal_snapshot(profile_dir: &Path) -> Vec<String> {
    let mut signals = Vec::new();
    let prefs_path = profile_dir.join("prefs.js");
    if let Ok(content) = fs::read_to_string(&prefs_path) {
        for line in content.lines().map(str::trim) {
            if line.contains("browser.sessionstore.resume_session_once") && line.contains("true") {
                signals.push("prefs:resume_session_once".to_string());
            }
            if line.contains("browser.startup.page")
                && (line.contains(", 3)") || line.contains(",3);") || line.contains(",3)"))
            {
                signals.push("prefs:startup.page=3".to_string());
            }
            if line.contains("browser.startup.couldRestoreSession.count")
                && !line.contains(", 0)")
                && !line.contains(",0)")
                && !line.contains(", 0);")
            {
                signals.push("prefs:couldRestoreSession.count".to_string());
            }
            if line.contains("browser.startup.homepage") {
                signals.push("prefs:startup.homepage".to_string());
            }
        }
    }
    let checkpoints_path = profile_dir.join("sessionCheckpoints.json");
    if let Ok(content) = fs::read_to_string(&checkpoints_path) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
            if value
                .get("sessionstore-windows-restored")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                signals.push("checkpoints:sessionstore-windows-restored".to_string());
            }
            if value
                .get("sessionstore-final-state-write-complete")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                signals.push("checkpoints:final-state-write-complete".to_string());
            }
        }
    }
    signals.sort();
    signals.dedup();
    signals
}

fn librewolf_session_snapshot(profile_dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let root_candidates = ["sessionstore.jsonlz4", "times.json", "sessionCheckpoints.json"];
    for name in root_candidates {
        let path = profile_dir.join(name);
        if path.exists() {
            files.push(name.to_string());
        }
    }
    let backups_dir = profile_dir.join("sessionstore-backups");
    if backups_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&backups_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    files.push(format!(
                        "sessionstore-backups/{}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ));
                }
            }
        }
    }
    files.sort();
    files
}

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

#[allow(dead_code)]
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

pub(crate) fn normalize_start_page_url_impl(default_start_page: Option<&str>) -> String {
    normalize_optional_start_page_url_impl(default_start_page)
        .unwrap_or_else(|| "https://duckduckgo.com".to_string())
}

pub(crate) fn normalize_optional_start_page_url_impl(
    default_start_page: Option<&str>,
) -> Option<String> {
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

fn chromium_profile_has_session_state_impl(profile_dir: &Path) -> bool {
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
