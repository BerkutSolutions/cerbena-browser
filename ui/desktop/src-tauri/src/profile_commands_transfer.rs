use super::*;

pub(crate) fn copy_profile_cookies_impl(
    state: State<AppState>,
    request: CopyCookiesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<CopyCookiesResponse>, String> {
    let source_id =
        Uuid::parse_str(&request.source_profile_id).map_err(|e| format!("profile id: {e}"))?;
    if request.target_profile_ids.is_empty() {
        return Err("target profile list is empty".to_string());
    }

    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    manager
        .ensure_unlocked(source_id)
        .map_err(|_| ERR_LOCKED_REQUIRES_UNLOCK.to_string())?;
    let source = manager.get_profile(source_id).map_err(|e| e.to_string())?;
    drop(manager);

    if is_profile_running_impl(&state, source_id)? {
        return Err("source profile must be stopped before copying cookies".to_string());
    }

    let mut copied_targets = 0usize;
    let mut skipped_targets = Vec::new();
    for raw_id in request.target_profile_ids {
        let target_id = Uuid::parse_str(&raw_id).map_err(|e| format!("profile id: {e}"))?;
        if target_id == source_id {
            skipped_targets.push(raw_id);
            continue;
        }

        let manager = state
            .manager
            .lock()
            .map_err(|_| "lock poisoned".to_string())?;
        manager
            .ensure_unlocked(target_id)
            .map_err(|_| ERR_LOCKED_REQUIRES_UNLOCK.to_string())?;
        let target = manager.get_profile(target_id).map_err(|e| e.to_string())?;
        drop(manager);
        if !cookies_copy_allowed(&source, &target) {
            return Err(ERR_COOKIES_COPY_BLOCKED.to_string());
        }

        if target.engine != source.engine {
            skipped_targets.push(target_id.to_string());
            continue;
        }
        if is_profile_running_impl(&state, target_id)? {
            skipped_targets.push(target_id.to_string());
            continue;
        }

        copy_engine_cookies_impl(
            source.engine.clone(),
            &state
                .profile_root
                .join(source_id.to_string())
                .join("engine-profile"),
            &state
                .profile_root
                .join(target_id.to_string())
                .join("engine-profile"),
        )?;
        copied_targets += 1;
    }

    Ok(ok(
        correlation_id,
        CopyCookiesResponse {
            copied_targets,
            skipped_targets,
        },
    ))
}

pub(crate) fn collect_profile_data_files_impl(
    root: &PathBuf,
    profile_id: Uuid,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let data_root = root.join(profile_id.to_string()).join("data");
    let mut files = Vec::new();
    if !data_root.exists() {
        return Ok(files);
    }
    collect_files_recursive_impl(&data_root, &data_root, &mut files)?;
    Ok(files)
}

pub(crate) fn collect_files_recursive_impl(
    base: &PathBuf,
    current: &PathBuf,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive_impl(base, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            out.push((rel, bytes));
        }
    }
    Ok(())
}

pub(crate) fn write_imported_files_impl(
    root: &PathBuf,
    profile_id: Uuid,
    files: Vec<browser_import_export::ExportFile>,
) -> Result<(), String> {
    let base = root.join(profile_id.to_string()).join("data");
    for file in files {
        let target = base.join(file.relative_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, file.content_b64)
                .map_err(|e| e.to_string())?;
        fs::write(target, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn is_profile_running_impl(state: &AppState, profile_id: Uuid) -> Result<bool, String> {
    let launched = state
        .launched_processes
        .lock()
        .map_err(|_| "launch map lock poisoned".to_string())?;
    let pid = launched.get(&profile_id).copied();
    drop(launched);

    let Some(pid) = pid else {
        return Ok(false);
    };
    if is_pid_running(pid) {
        return Ok(true);
    }

    clear_profile_process(&state.app_handle, profile_id, pid, true);
    Ok(false)
}

fn copy_engine_cookies_impl(
    engine: Engine,
    source_root: &PathBuf,
    target_root: &PathBuf,
) -> Result<(), String> {
    fs::create_dir_all(target_root).map_err(|e| e.to_string())?;
    let copied = match engine {
        Engine::Chromium | Engine::UngoogledChromium => {
            copy_cookie_path_impl(source_root, target_root, "Default\\Network\\Cookies")?
                | copy_cookie_path_impl(
                    source_root,
                    target_root,
                    "Default\\Network\\Cookies-journal",
                )?
                | copy_cookie_path_impl(source_root, target_root, "Default\\Cookies")?
                | copy_cookie_path_impl(source_root, target_root, "Default\\Cookies-journal")?
        }
        Engine::FirefoxEsr | Engine::Librewolf => {
            copy_cookie_path_impl(source_root, target_root, "cookies.sqlite")?
                | copy_cookie_path_impl(source_root, target_root, "cookies.sqlite-wal")?
                | copy_cookie_path_impl(source_root, target_root, "cookies.sqlite-shm")?
        }
    };

    if !copied {
        return Err("source profile does not contain cookie store files yet".to_string());
    }
    Ok(())
}

fn copy_cookie_path_impl(
    source_root: &PathBuf,
    target_root: &PathBuf,
    relative: &str,
) -> Result<bool, String> {
    let source = source_root.join(relative);
    if !source.exists() {
        return Ok(false);
    }
    let target = target_root.join(relative);
    if source.is_dir() {
        copy_dir_recursive_impl(&source, &target)?;
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(&source, &target).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

fn copy_dir_recursive_impl(source: &PathBuf, target: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive_impl(&source_path, &target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&source_path, &target_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
