use browser_api_local::{
    ApiRole, ConsentGrant, HomeDashboardModel, LaunchHookPolicy, PanicMode, SearchProvider,
};
use browser_network_policy::{BlocklistSource, DnsBlocklistUpdater};
use browser_profile::ProfileState;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path, time::Duration};
use tauri::{Emitter, State};
use uuid::Uuid;

use crate::{
    certificate_runtime::{display_certificate_issuer, load_certificate_metadata},
    device_posture::{get_or_refresh_device_posture, refresh_device_posture, DevicePostureReport},
    envelope::{ok, UiEnvelope},
    launch_sessions::revoke_launch_session,
    network_sandbox_lifecycle::stop_profile_network_stack,
    process_tracking::{
        clear_profile_process, terminate_process_tree, terminate_profile_processes,
    },
    state::{persist_link_routing_store_with_secret, AppState},
};
#[path = "launcher_commands_operator.rs"]
mod operator;
#[path = "launcher_commands_os.rs"]
mod os;
#[path = "launcher_commands_security.rs"]
mod security;
#[path = "launcher_commands_security_store.rs"]
mod security_store;
#[path = "launcher_commands_links.rs"]
mod links;
#[path = "launcher_commands_models.rs"]
mod models;
pub(crate) use models::*;
pub(crate) fn detect_link_type(raw_url: &str) -> Result<String, String> {
    links::detect_link_type_impl(raw_url)
}

pub(crate) fn push_runtime_log(state: &AppState, entry: impl Into<String>) {
    let line = entry.into();
    if let Ok(mut logs) = state.runtime_logs.lock() {
        logs.push(line.clone());
        if logs.len() > RUNTIME_LOG_LIMIT {
            let overflow = logs.len() - RUNTIME_LOG_LIMIT;
            logs.drain(0..overflow);
        }
    }
    append_runtime_log_file(state, &line);
    let _ = state.app_handle.emit(RUNTIME_LOG_EVENT_NAME, line);
}



#[tauri::command]
pub fn get_device_posture_report(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<DevicePostureReport>, String> {
    os::get_device_posture_report(state, correlation_id)
}

#[tauri::command]
pub fn refresh_device_posture_report(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<DevicePostureReport>, String> {
    os::refresh_device_posture_report(state, correlation_id)
}

#[tauri::command]
pub fn build_home_dashboard(
    state: State<AppState>,
    request: BuildHomeRequest,
    correlation_id: String,
) -> Result<UiEnvelope<HomeDashboardModel>, String> {
    operator::build_home_dashboard(state, request, correlation_id)
}

#[tauri::command]
pub fn panic_wipe_profile(
    state: State<AppState>,
    request: PanicRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    operator::panic_wipe_profile(state, request, correlation_id)
}

#[tauri::command]
pub fn set_default_profile_for_links(
    state: State<AppState>,
    request: DefaultProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    os::set_default_profile_for_links(state, request, correlation_id)
}

#[tauri::command]
pub fn clear_default_profile_for_links(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    os::clear_default_profile_for_links(state, correlation_id)
}

#[tauri::command]
pub fn get_link_routing_overview(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<LinkRoutingOverview>, String> {
    os::get_link_routing_overview(state, correlation_id)
}

#[tauri::command]
pub fn save_link_type_profile_binding(
    state: State<AppState>,
    request: LinkTypeBindingRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    os::save_link_type_profile_binding(state, request, correlation_id)
}

#[tauri::command]
pub fn remove_link_type_profile_binding(
    state: State<AppState>,
    request: LinkTypeRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    os::remove_link_type_profile_binding(state, request, correlation_id)
}

#[tauri::command]
pub fn dispatch_external_link(
    state: State<AppState>,
    request: DispatchLinkRequest,
    correlation_id: String,
) -> Result<UiEnvelope<DispatchLinkResolution>, String> {
    os::dispatch_external_link(state, request, correlation_id)
}

#[tauri::command]
pub fn consume_pending_external_link(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Option<String>>, String> {
    os::consume_pending_external_link(state, correlation_id)
}

#[tauri::command]
pub fn execute_launch_hook(
    state: State<AppState>,
    request: ExecuteHookRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    operator::execute_launch_hook(state, request, correlation_id)
}

#[tauri::command]
pub fn resolve_pip_policy(
    state: State<AppState>,
    request: ResolvePipRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    operator::resolve_pip_policy(state, request, correlation_id)
}

#[tauri::command]
pub fn import_search_providers(
    state: State<AppState>,
    request: ImportSearchRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    operator::import_search_providers(state, request, correlation_id)
}

#[tauri::command]
pub fn set_default_search_provider(
    state: State<AppState>,
    request: DefaultSearchRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    operator::set_default_search_provider(state, request, correlation_id)
}

#[tauri::command]
pub fn run_guardrail_check(
    state: State<AppState>,
    request: GuardrailCheckRequest,
    correlation_id: String,
) -> Result<UiEnvelope<GuardrailCheckResult>, String> {
    operator::run_guardrail_check(state, request, correlation_id)
}

#[tauri::command]
pub fn append_runtime_log(
    state: State<AppState>,
    entry: String,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    operator::append_runtime_log(state, entry, correlation_id)
}

#[tauri::command]
pub fn read_runtime_logs(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<String>>, String> {
    operator::read_runtime_logs(state, correlation_id)
}

#[tauri::command]
pub fn get_global_security_settings(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    security::get_global_security_settings(state, correlation_id)
}

#[tauri::command]
pub fn save_global_security_settings(
    state: State<AppState>,
    request: GlobalSecuritySettingsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    security::save_global_security_settings(state, request, correlation_id)
}


pub(crate) fn load_global_security_record(
    state: &AppState,
) -> Result<GlobalSecuritySettingsRecord, String> {
    security_store::load_global_security_record_impl(state)
}

#[cfg(test)]
fn load_global_security_record_from_paths(
    path: &Path,
    legacy_path: &Path,
    secret_material: &str,
) -> Result<GlobalSecuritySettingsRecord, String> {
    security_store::load_global_security_record_from_paths_impl(path, legacy_path, secret_material)
}

pub(crate) fn persist_global_security_record(
    state: &AppState,
    payload: &GlobalSecuritySettingsRecord,
) -> Result<(), String> {
    security_store::persist_global_security_record_impl(state, payload)
}

#[cfg(test)]
fn persist_global_security_record_to_paths(
    path: &Path,
    legacy_path: &Path,
    secret_material: &str,
    payload: &GlobalSecuritySettingsRecord,
) -> Result<(), String> {
    security_store::persist_global_security_record_to_paths_impl(
        path,
        legacy_path,
        secret_material,
        payload,
    )
}

fn now_unix_ms() -> u128 {
    security_store::now_unix_ms_impl()
}

#[cfg(test)]
#[path = "launcher_commands_tests.rs"]
mod tests;
