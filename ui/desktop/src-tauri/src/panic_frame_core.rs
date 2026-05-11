use std::{
    thread,
    time::{Duration, Instant},
};

use serde::Deserialize;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindowBuilder,
};
use uuid::Uuid;

use crate::envelope::{ok, UiEnvelope};
use crate::{
    process_tracking::{find_profile_main_window_pid_for_dir, is_process_running},
    state::AppState,
};

const FRAME_SIDE_BLEED: f64 = 0.0;
const FRAME_TOP_BLEED: f64 = 0.0;
const FRAME_BOTTOM_BLEED: f64 = 0.0;
const LABEL_HEIGHT: f64 = 30.0;
const LABEL_MIN_WIDTH: f64 = 220.0;
const LABEL_MAX_WIDTH: f64 = 340.0;
const LABEL_LIFT: f64 = 24.0;
const CONTROL_SIZE: f64 = 32.0;
const CONTROL_TOP_OFFSET: f64 = 7.0;
const CONTROL_RIGHT_GAP: f64 = 150.0;
const MENU_WIDTH: f64 = 360.0;
const MENU_HEIGHT: f64 = 520.0;
const MENU_OFFSET_Y: f64 = 8.0;
const POLL_MS: u64 = 8;
const PROCESS_CHECK_MS: u64 = 500;
const PID_REFRESH_MS: u64 = 1200;

#[cfg(target_os = "windows")]
use std::{ffi::c_void, mem::size_of, ptr};

#[derive(Debug, Clone, Copy)]
pub(crate) struct WindowBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    work_left: f64,
    work_top: f64,
    work_right: f64,
    work_bottom: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanicFrameMenuRequest {
    pub profile_id: Uuid,
}

pub fn maybe_start_panic_frame(app_handle: &AppHandle, profile_id: Uuid, pid: u32) {
    let state = app_handle.state::<AppState>();
    let enabled = state
        .manager
        .lock()
        .ok()
        .and_then(|manager| manager.get_profile(profile_id).ok())
        .map(|profile| profile.panic_frame_enabled)
        .unwrap_or(false);
    if !enabled {
        close_panic_frame(app_handle, profile_id);
        return;
    }

    {
        let mut active = match state.active_panic_frames.lock() {
            Ok(value) => value,
            Err(_) => return,
        };
        if !active.insert(profile_id) {
            return;
        }
    }

    let handle = app_handle.clone();
    thread::spawn(move || {
        let border_label = panic_frame_border_label(profile_id);
        let label_label = panic_frame_label_label(profile_id);
        let controls_label = panic_frame_controls_label(profile_id);
        let menu_label = panic_frame_menu_label(profile_id);
        let mut target_pid = pid;
        let mut last_process_check = Instant::now()
            .checked_sub(Duration::from_millis(PROCESS_CHECK_MS))
            .unwrap_or_else(Instant::now);
        let mut last_pid_refresh = Instant::now()
            .checked_sub(Duration::from_millis(PID_REFRESH_MS))
            .unwrap_or_else(Instant::now);
        let user_data_dir = handle
            .state::<AppState>()
            .profile_root
            .join(profile_id.to_string())
            .join("engine-profile");
        let _ = ensure_panic_frame_windows(&handle, profile_id);
        loop {
            let (should_continue, current_pid) = {
                let state = handle.state::<AppState>();
                let active = match state.active_panic_frames.lock() {
                    Ok(value) => value,
                    Err(_) => break,
                };
                let launched = match state.launched_processes.lock() {
                    Ok(value) => value,
                    Err(_) => break,
                };
                (
                    active.contains(&profile_id),
                    launched.get(&profile_id).copied().unwrap_or(pid),
                )
            };
            if !should_continue {
                break;
            }

            if current_pid != target_pid {
                target_pid = current_pid;
            }

            if last_process_check.elapsed() >= Duration::from_millis(PROCESS_CHECK_MS) {
                last_process_check = Instant::now();
                if !is_process_running(target_pid) {
                    break;
                }
            }

            if last_pid_refresh.elapsed() >= Duration::from_millis(PID_REFRESH_MS) {
                last_pid_refresh = Instant::now();
                if let Some(main_window_pid) = find_profile_main_window_pid_for_dir(&user_data_dir)
                {
                    if main_window_pid != target_pid {
                        target_pid = main_window_pid;
                        if let Ok(mut launched) =
                            handle.state::<AppState>().launched_processes.lock()
                        {
                            launched.insert(profile_id, target_pid);
                        }
                    }
                }
            }

            if let Some(bounds) = query_main_window_bounds(target_pid) {
                let _ = update_panic_frame_window(&handle, &border_label, bounds, "border");
                let _ = update_panic_frame_window(&handle, &label_label, bounds, "label");
                let _ = update_panic_frame_window(&handle, &controls_label, bounds, "controls");
                let _ = update_panic_frame_window(&handle, &menu_label, bounds, "menu");
            } else {
                hide_panic_frame_windows(&handle, profile_id);
            }
            thread::sleep(Duration::from_millis(POLL_MS));
        }
        close_panic_frame(&handle, profile_id);
    });
}

pub fn close_panic_frame(app_handle: &AppHandle, profile_id: Uuid) {
    if let Ok(mut active) = app_handle.state::<AppState>().active_panic_frames.lock() {
        active.remove(&profile_id);
    }
    if let Some(window) = app_handle.get_webview_window(&panic_frame_border_label(profile_id)) {
        hide_native_overlay_window(&window);
        let _ = window.hide();
        let _ = window.close();
    }
    if let Some(window) = app_handle.get_webview_window(&panic_frame_label_label(profile_id)) {
        hide_native_overlay_window(&window);
        let _ = window.hide();
        let _ = window.close();
    }
    if let Some(window) = app_handle.get_webview_window(&panic_frame_controls_label(profile_id)) {
        hide_native_overlay_window(&window);
        let _ = window.hide();
        let _ = window.close();
    }
    if let Some(window) = app_handle.get_webview_window(&panic_frame_menu_label(profile_id)) {
        hide_native_overlay_window(&window);
        let _ = window.hide();
        let _ = window.close();
    }
}


#[path = "panic_frame_core_ops.rs"]
mod ops;
pub(crate) use ops::*;


