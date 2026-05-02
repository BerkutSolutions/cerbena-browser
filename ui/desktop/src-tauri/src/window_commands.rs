use tauri::{AppHandle, Manager};

use crate::envelope::{ok, UiEnvelope};
use crate::process_tracking::stop_all_profile_processes;
use crate::network_sandbox_lifecycle::{
    cleanup_network_sandbox_janitor, stop_all_profile_network_stacks,
};

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
    stop_all_profile_processes(&app);
    stop_all_profile_network_stacks(&app);
    cleanup_network_sandbox_janitor(&app);
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.close().map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}
