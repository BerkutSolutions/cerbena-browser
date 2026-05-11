use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::TcpListener,
    path::PathBuf,
    process::{Command, Output},
    thread,
    time::Duration,
};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

#[path = "route_runtime_cleanup.rs"]
mod cleanup;
#[path = "route_runtime_diagnostics.rs"]
mod diagnostics;
#[path = "route_runtime_launch.rs"]
mod launch;
#[path = "route_runtime_launch_container.rs"]
mod launch_container;
#[path = "route_runtime_launch_host.rs"]
mod launch_host;
#[path = "route_runtime_openvpn.rs"]
mod openvpn;
#[path = "route_runtime_amnezia.rs"]
mod amnezia;
#[path = "route_runtime_parse.rs"]
mod parse;
#[path = "route_runtime_selection.rs"]
mod selection;
#[path = "route_runtime_session.rs"]
mod session;
#[path = "route_runtime_singbox.rs"]
mod singbox;
#[path = "route_runtime_support.rs"]
mod support;

use crate::{
    launcher_commands::push_runtime_log,
    network_runtime::{
        ensure_network_runtime_tools, resolve_amneziawg_binary_path, resolve_openvpn_binary_path,
        resolve_sing_box_binary_path, resolve_tor_binary_path, resolve_tor_pt_binary_path,
        NetworkTool,
    },
    network_sandbox::{resolve_profile_network_sandbox_mode, ResolvedNetworkSandboxMode},
    network_sandbox_container_runtime::{
        cleanup_stale_container_route_runtimes, launch_amnezia_container_runtime,
        launch_openvpn_container_runtime, launch_sing_box_container_runtime,
        stop_container_runtime, CONTAINER_PROXY_PORT,
    },
    process_tracking::is_process_running,
    profile_runtime_logs::append_profile_log,
    state::AppState,
};

pub(crate) use support::*;

fn hidden_command(program: &str) -> Command {
    #[cfg(target_os = "windows")]
    let mut command = Command::new(program);
    #[cfg(not(target_os = "windows"))]
    let command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

fn emit_profile_launch_progress(
    app_handle: &AppHandle,
    profile_id: Uuid,
    stage_key: &str,
    message_key: &str,
) {
    let _ = app_handle.emit(
        "profile-launch-progress",
        serde_json::json!({
            "profileId": profile_id.to_string(),
            "stageKey": stage_key,
            "messageKey": message_key,
            "done": false,
            "error": serde_json::Value::Null,
        }),
    );
}

#[derive(Debug, Default)]
pub struct RouteRuntimeState {
    pub sessions: BTreeMap<String, RouteRuntimeSession>,
}

#[derive(Debug, Clone)]
pub struct RouteRuntimeSession {
    pub signature: String,
    pub pid: Option<u32>,
    pub backend: RouteRuntimeBackend,
    pub listen_port: Option<u16>,
    pub config_path: PathBuf,
    pub cleanup_paths: Vec<PathBuf>,
    pub tunnel_name: Option<String>,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RouteRuntimeSessionSnapshot {
    pub backend: RouteRuntimeBackend,
    pub listen_port: Option<u16>,
    pub config_path: PathBuf,
    pub cleanup_paths: Vec<PathBuf>,
    pub tunnel_name: Option<String>,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteRuntimeBackend {
    SingBox,
    OpenVpn,
    AmneziaWg,
    ContainerSocks,
}

pub fn runtime_proxy_endpoint(app_handle: &AppHandle, profile_id: Uuid) -> Option<(String, u16)> {
    session::runtime_proxy_endpoint_impl(app_handle, profile_id)
}

pub fn runtime_session_active(app_handle: &AppHandle, profile_id: Uuid) -> bool {
    session::runtime_session_active_impl(app_handle, profile_id)
}

pub fn runtime_session_snapshot(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Option<RouteRuntimeSessionSnapshot> {
    session::runtime_session_snapshot_impl(app_handle, profile_id)
}

fn session_is_active(session: &RouteRuntimeSession) -> bool {
    session::session_is_active_impl(session)
}

pub fn route_runtime_required_for_profile(app_handle: &AppHandle, profile_id: Uuid) -> bool {
    session::route_runtime_required_for_profile_impl(app_handle, profile_id)
}

pub fn stop_profile_route_runtime(app_handle: &AppHandle, profile_id: Uuid) {
    session::stop_profile_route_runtime_impl(app_handle, profile_id);
}

pub fn stop_all_route_runtime(app_handle: &AppHandle) {
    session::stop_all_route_runtime_impl(app_handle);
}

pub fn cleanup_legacy_route_runtime(app_handle: &AppHandle) {
    cleanup::cleanup_legacy_route_runtime_impl(app_handle);
}

pub fn cleanup_stale_route_runtime_artifacts(
    app_handle: &AppHandle,
    active_profiles: &BTreeSet<Uuid>,
) {
    cleanup::cleanup_stale_route_runtime_artifacts_impl(app_handle, active_profiles);
}

pub fn ensure_profile_route_runtime(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Result<(), String> {
    launch::ensure_profile_route_runtime_impl(app_handle, profile_id)
}

fn resolve_effective_route_selection(
    store: &crate::state::NetworkStore,
    profile_key: &str,
) -> (String, Option<String>) {
    selection::resolve_effective_route_selection_impl(store, profile_key)
}

fn append_route_runtime_log(state: &AppState, entry: String) {
    push_runtime_log(state, entry);
}

fn reserve_local_port() -> Result<u16, String> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind local route runtime: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("route runtime local addr: {e}"))?
        .port();
    if port == 0 {
        return Err("route runtime local port is zero".to_string());
    }
    Ok(port)
}

pub(crate) fn amnezia_config_requires_native_backend(value: &str) -> Result<bool, String> {
    support::amnezia_config_requires_native_backend(value)
}

#[cfg(test)]
fn should_remove_runtime_artifact(file_name: &str) -> bool {
    cleanup::should_remove_runtime_artifact_impl(file_name)
}

#[cfg(test)]
fn container_tor_transport_binary(protocol: &str) -> Option<String> {
    match protocol {
        "obfs4" => Some("/usr/bin/obfs4proxy".to_string()),
        "snowflake" => Some("/usr/bin/snowflake-client".to_string()),
        "meek" => Some("/usr/bin/obfs4proxy".to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "route_runtime_tests.rs"]
mod route_runtime_tests;


