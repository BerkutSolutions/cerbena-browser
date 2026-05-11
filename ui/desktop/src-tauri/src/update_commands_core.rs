use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State};

use crate::{
    envelope::{ok, UiEnvelope},
    launcher_commands::push_runtime_log,
    platform::release_security,
    state::{app_local_data_root, persist_app_update_store, AppState},
};

#[path = "update_commands_state.rs"]
mod updater_state;
#[path = "update_commands_discovery.rs"]
mod updater_discovery;
#[path = "update_commands_transfer.rs"]
mod updater_transfer;
#[path = "update_commands_lifecycle.rs"]
mod updater_lifecycle;
#[path = "update_commands_verification.rs"]
mod updater_verification;
#[path = "update_commands_apply.rs"]
mod updater_apply;
#[path = "update_commands_commands.rs"]
mod updater_commands;
#[path = "update_commands_reporting.rs"]
mod updater_reporting;
#[path = "update_commands_flow.rs"]
mod updater_flow;

#[path = "update_commands_core_types.rs"]
mod updater_core_types;
pub(crate) use updater_core_types::*;

#[tauri::command]
pub fn get_launcher_update_state(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    updater_commands::get_launcher_update_state_impl(state, correlation_id)
}

#[tauri::command]
pub fn set_launcher_auto_update(
    state: State<AppState>,
    enabled: bool,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    updater_commands::set_launcher_auto_update_impl(state, enabled, correlation_id)
}

#[tauri::command]
pub fn check_launcher_updates(
    state: State<AppState>,
    manual: bool,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    updater_commands::check_launcher_updates_impl(state, manual, correlation_id)
}

pub fn start_update_scheduler(app: AppHandle) {
    updater_lifecycle::start_update_scheduler_impl(app);
}

pub fn active_updater_launch_mode() -> UpdaterLaunchMode {
    updater_lifecycle::active_updater_launch_mode_impl()
}

pub fn configure_window_for_launch_mode(
    window: &tauri::WebviewWindow,
    mode: UpdaterLaunchMode,
) -> Result<(), String> {
    updater_lifecycle::configure_window_for_launch_mode_impl(window, mode)
}

pub fn ensure_updater_flow_started(state: &AppState) -> Result<(), String> {
    updater_lifecycle::ensure_updater_flow_started_impl(state)
}


#[path = "update_commands_core_runtime.rs"]
mod updater_core_runtime;
pub(crate) use updater_core_runtime::*;
