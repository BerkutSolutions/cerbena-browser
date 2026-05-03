use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

use crate::envelope::{ok, UiEnvelope};
use crate::instance_handoff;
use crate::network_sandbox_lifecycle::{
    cleanup_network_sandbox_janitor, stop_all_profile_network_stacks,
};
use crate::panic_frame::close_panic_frame;
use crate::process_tracking::stop_all_profile_processes;
use crate::state::AppState;
use crate::update_commands;

pub fn emit_app_lifecycle_progress(
    app: &AppHandle,
    phase: &str,
    stage_key: &str,
    message_key: &str,
    done: bool,
) {
    let _ = app.emit(
        "app-lifecycle-progress",
        serde_json::json!({
            "phase": phase,
            "stageKey": stage_key,
            "messageKey": message_key,
            "done": done,
        }),
    );
}

fn has_active_session_state(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let has_launched_processes = state
        .launched_processes
        .lock()
        .map(|guard| !guard.is_empty())
        .unwrap_or(true);
    let has_active_network_stacks = state
        .network_sandbox_lifecycle
        .lock()
        .map(|guard| !guard.active_profiles.is_empty())
        .unwrap_or(true);
    has_launched_processes || has_active_network_stacks
}

pub fn perform_shutdown_cleanup(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.shutdown_cleanup_started.swap(true, Ordering::SeqCst) {
        return;
    }
    instance_handoff::cleanup_primary_instance(app);

    emit_app_lifecycle_progress(
        app,
        "shutdown",
        "handoff",
        "app.lifecycle.shutdown.handoff",
        false,
    );
    update_commands::launch_pending_update_on_exit(app);

    if !has_active_session_state(app) {
        emit_app_lifecycle_progress(app, "shutdown", "done", "app.lifecycle.shutdown.done", true);
        return;
    }

    emit_app_lifecycle_progress(
        app,
        "shutdown",
        "processes",
        "app.lifecycle.shutdown.processes",
        false,
    );
    if let Ok(launched) = app
        .state::<AppState>()
        .launched_processes
        .lock()
        .map(|guard| guard.keys().copied().collect::<Vec<_>>())
    {
        for profile_id in launched {
            close_panic_frame(app, profile_id);
        }
    }
    stop_all_profile_processes(app);

    emit_app_lifecycle_progress(
        app,
        "shutdown",
        "network",
        "app.lifecycle.shutdown.network",
        false,
    );
    stop_all_profile_network_stacks(app);

    emit_app_lifecycle_progress(
        app,
        "shutdown",
        "cleanup",
        "app.lifecycle.shutdown.cleanup",
        false,
    );
    cleanup_network_sandbox_janitor(app);

    emit_app_lifecycle_progress(app, "shutdown", "done", "app.lifecycle.shutdown.done", true);
}

#[tauri::command]
pub fn window_minimize(app: AppHandle, correlation_id: String) -> Result<UiEnvelope<bool>, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.minimize().map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn window_toggle_maximize(
    app: AppHandle,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let is_maximized = window.is_maximized().map_err(|e| e.to_string())?;
    if is_maximized {
        window.unmaximize().map_err(|e| e.to_string())?;
    } else {
        window.maximize().map_err(|e| e.to_string())?;
    }
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn window_close(app: AppHandle, correlation_id: String) -> Result<UiEnvelope<bool>, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.close().map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}
