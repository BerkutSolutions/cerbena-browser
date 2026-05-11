use super::*;

pub(crate) fn start_update_scheduler_impl(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(SCHEDULER_TICK);
        let state = app.state::<AppState>();
        let updater_active = state
            .updater_runtime
            .lock()
            .map(|runtime| runtime.launch_mode.is_active())
            .unwrap_or(false);
        if updater_active {
            continue;
        }
        let should_run = match state.app_update_store.lock() {
            Ok(store) => should_run_auto_update_check_impl(&store),
            Err(_) => false,
        };
        if should_run {
            let _ = run_update_cycle(&state, false);
        }
    });
}

pub(crate) fn active_updater_launch_mode_impl() -> UpdaterLaunchMode {
    UpdaterLaunchMode::from_args(std::env::args().skip(1))
}

pub(crate) fn configure_window_for_launch_mode_impl(
    window: &tauri::WebviewWindow,
    mode: UpdaterLaunchMode,
) -> Result<(), String> {
    if !mode.is_active() {
        return Ok(());
    }
    window
        .set_title(if mode.is_preview() {
            "Cerbena Updater Preview"
        } else {
            "Cerbena Updater"
        })
        .map_err(|e| format!("set updater window title: {e}"))?;
    window
        .set_size(LogicalSize::new(760.0, 720.0))
        .map_err(|e| format!("set updater window size: {e}"))?;
    window
        .set_min_size(Some(LogicalSize::new(680.0, 640.0)))
        .map_err(|e| format!("set updater window min size: {e}"))?;
    let _ = window.center();
    window
        .eval(&format!(
            "window.location.replace('./updater.html?mode={}');",
            if mode.is_preview() { "preview" } else { "auto" }
        ))
        .map_err(|e| format!("redirect updater window: {e}"))?;
    Ok(())
}

pub(crate) fn ensure_updater_flow_started_impl(state: &AppState) -> Result<(), String> {
    let runtime = state.updater_runtime.clone();
    let launch_mode = {
        let mut guard = runtime
            .lock()
            .map_err(|e| format!("lock updater runtime: {e}"))?;
        if guard.flow_started {
            return Ok(());
        }
        guard.flow_started = true;
        guard.running = true;
        guard.overview.status = "running".to_string();
        guard.overview.started_at = Some(now_iso());
        guard.overview.finished_at = None;
        guard.overview.can_close = false;
        guard.overview.close_label_key = "updater.running".to_string();
        guard.launch_mode
    };

    let app_handle = state.app_handle.clone();
    thread::spawn(move || {
        let app_state = app_handle.state::<AppState>();
        let result = run_updater_flow(&app_state, launch_mode);
        if let Err(error) = result {
            let _ = finalize_updater_failure(&app_state, &error);
        }
    });
    Ok(())
}

pub(crate) fn updater_launch_mode_from_state_impl(
    state: &AppState,
) -> Result<UpdaterLaunchMode, String> {
    state
        .updater_runtime
        .lock()
        .map(|runtime| runtime.launch_mode)
        .map_err(|e| format!("lock updater runtime: {e}"))
}

pub(crate) fn schedule_updater_window_close_for_apply_impl(state: &AppState) {
    push_runtime_log(
        state,
        format!(
            "[updater] scheduling updater window close to trigger pending apply delayMs={}",
            UPDATER_AUTO_CLOSE_AFTER_READY_DELAY.as_millis()
        ),
    );
    let app_handle = state.app_handle.clone();
    thread::spawn(move || {
        thread::sleep(UPDATER_AUTO_CLOSE_AFTER_READY_DELAY);
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.close();
        }
    });
}

pub(crate) fn should_auto_close_updater_after_ready_to_restart_impl(
    launch_mode: UpdaterLaunchMode,
) -> bool {
    matches!(launch_mode, UpdaterLaunchMode::Auto)
}

pub(crate) fn run_updater_flow_impl(
    state: &AppState,
    launch_mode: UpdaterLaunchMode,
) -> Result<(), String> {
    if launch_mode.is_preview() {
        return run_preview_updater_flow(state);
    }
    run_live_updater_flow(state)
}

pub(crate) fn should_launch_external_updater_impl(
    store: &AppUpdateStore,
    candidate: &ReleaseCandidate,
) -> bool {
    store.updater_handoff_version.as_deref() != Some(candidate.version.as_str())
}

pub(crate) fn should_run_auto_update_check_impl(store: &AppUpdateStore) -> bool {
    updater_state::should_run_auto_update_check_impl(store)
}

pub(crate) fn spawn_updater_process_impl(
    app: &AppHandle,
    mode: UpdaterLaunchMode,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let exe = resolve_updater_executable_path(app)?;
    let mut command = Command::new(exe);
    if mode.is_preview() {
        command.arg("--updater-preview");
    } else {
        command.arg("--updater");
    }
    if let Some(dir) = app_local_data_root(app)
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
    {
        command.current_dir(dir);
    }
    if let Ok(log_path) = state.runtime_log_path(app) {
        command.env(UPDATER_HELPER_LOG_ENV, log_path);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x00000010);
    }
    push_runtime_log(
        &state,
        format!(
            "[updater] spawning standalone updater mode={} exe={}",
            if mode.is_preview() { "preview" } else { "auto" },
            command.get_program().to_string_lossy()
        ),
    );
    command
        .spawn()
        .map_err(|e| format!("spawn standalone updater: {e}"))?;
    Ok(())
}

pub(crate) fn resolve_updater_executable_path_impl(app: &AppHandle) -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("resolve current exe: {e}"))?;
    let adjacent = current_exe
        .parent()
        .map(|parent| parent.join("cerbena-updater.exe"))
        .ok_or_else(|| "resolve current exe parent for updater".to_string())?;
    if adjacent.is_file() {
        return Ok(adjacent);
    }
    let _ = app;
    Ok(current_exe)
}

pub(crate) fn current_windows_install_mode_impl() -> String {
    if !cfg!(target_os = "windows") {
        return "non_windows".to_string();
    }
    let Ok(current_exe) = std::env::current_exe() else {
        return "portable_zip".to_string();
    };
    let Some(install_root) = current_exe.parent() else {
        return "portable_zip".to_string();
    };
    let marker = install_root.join("cerbena-install-mode.txt");
    if let Ok(value) = fs::read_to_string(marker) {
        let normalized = value.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            return normalized;
        }
    }
    "portable_zip".to_string()
}

pub(crate) fn launch_pending_update_on_exit_impl(app: &AppHandle) {
    let state = app.state::<AppState>();

    let snapshot = match state.app_update_store.lock() {
        Ok(store) => store.clone(),
        Err(error) => {
            push_runtime_log(
                &state,
                format!("[updater] pending apply skipped: failed to lock store: {error}"),
            );
            return;
        }
    };

    if !snapshot.pending_apply_on_exit {
        push_runtime_log(
            &state,
            "[updater] pending apply skipped: pending_apply_on_exit=false",
        );
        return;
    }

    let Some(path) = snapshot.staged_asset_path.as_ref() else {
        push_runtime_log(
            &state,
            "[updater] pending apply skipped: staged_asset_path missing",
        );
        return;
    };
    let asset_path = PathBuf::from(path);
    if !asset_path.is_file() {
        push_runtime_log(
            &state,
            format!(
                "[updater] pending apply skipped: staged asset missing path={}",
                asset_path.display()
            ),
        );
        return;
    }

    let extension = asset_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let current_pid = std::process::id();
    let current_exe = match std::env::current_exe() {
        Ok(value) => value,
        Err(error) => {
            push_runtime_log(
                &state,
                format!("[updater] pending apply skipped: resolve current exe failed: {error}"),
            );
            return;
        }
    };
    let install_root = match current_exe.parent() {
        Some(value) => value.to_path_buf(),
        None => {
            push_runtime_log(
                &state,
                "[updater] pending apply skipped: install root missing",
            );
            return;
        }
    };
    let relaunch_executable = resolve_relaunch_executable_path(&install_root);
    let msi_target_install_root = app_local_data_root(&state.app_handle).ok();
    let runtime_log_path = state
        .runtime_log_path(&state.app_handle)
        .ok()
        .map(|value| value.to_string_lossy().to_string());
    push_runtime_log(
        &state,
        format!(
            "[updater] pending apply start asset={} extension={} install_root={} msi_target_root={} relaunch_exe={} target_version={}",
            asset_path.display(),
            extension,
            install_root.display(),
            msi_target_install_root
                .as_deref()
                .map(|value| value.display().to_string())
                .unwrap_or_else(|| "missing".to_string()),
            relaunch_executable
                .as_deref()
                .map(|value| value.display().to_string())
                .unwrap_or_else(|| "missing".to_string()),
            snapshot
                .staged_version
                .clone()
                .unwrap_or_else(|| "unknown".to_string())
        ),
    );

    let launched = match extension.as_str() {
        "zip" => launch_zip_apply_helper(
            current_pid,
            &asset_path,
            &install_root,
            relaunch_executable.as_deref(),
            runtime_log_path.as_deref(),
        )
        .is_ok(),
        "msi" => {
            let store_path = state
                .app_update_store_path(&state.app_handle)
                .ok()
                .map(|value| value.to_string_lossy().to_string());
            launch_msi_apply_helper(
                current_pid,
                &asset_path,
                msi_target_install_root.as_deref(),
                store_path.as_deref(),
                snapshot.staged_version.as_deref(),
                runtime_log_path.as_deref(),
            )
            .is_ok()
        }
        _ => false,
    };

    if !launched {
        push_runtime_log(
            &state,
            format!("[updater] pending apply failed: helper launch failed for {extension}"),
        );
        return;
    }

    if let Ok(mut store) = state.app_update_store.lock() {
        store.pending_apply_on_exit = false;
        store.status = "applying".to_string();
        let _ = persist_update_store_from_state(&state, &store);
        push_runtime_log(
            &state,
            format!(
                "[updater] pending apply helper launched asset={} status=applying",
                asset_path.display()
            ),
        );
    };
}
