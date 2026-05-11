#![allow(dead_code)]

use super::*;
#[path = "route_runtime_support_preflight.rs"]
mod preflight;
#[path = "route_runtime_support_amnezia_service.rs"]
mod amnezia_service;

#[derive(Debug)]
pub(super) struct OpenVpnLaunch {
    pub(super) pid: u32,
    pub(super) config_path: PathBuf,
    pub(super) cleanup_paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub(super) struct AmneziaWgLaunch {
    pub(super) config_path: PathBuf,
    pub(super) cleanup_paths: Vec<PathBuf>,
    pub(super) tunnel_name: String,
}

#[derive(Debug, Clone)]
pub(super) struct AmneziaServiceSnapshot {
    pub(super) exists: bool,
    pub(super) state_code: Option<u32>,
    pub(super) raw_output: String,
}

pub(super) use diagnostics::tail_lines_impl as tail_lines;

pub(super) fn amnezia_node_requires_native_backend(node: &NormalizedNode) -> Result<bool, String> { preflight::amnezia_node_requires_native_backend_impl(node) }
pub(crate) fn amnezia_config_requires_native_backend(value: &str) -> Result<bool, String> { preflight::amnezia_config_requires_native_backend_impl(value) }
pub(super) fn node_supported_by_runtime(node: &NormalizedNode) -> bool { preflight::node_supported_by_runtime_impl(node) }
pub(super) fn normalized_nodes(template: &crate::state::ConnectionTemplate) -> Vec<NormalizedNode> { preflight::normalized_nodes_impl(template) }
pub(super) fn normalize_connection_type(value: &str) -> String { preflight::normalize_connection_type_impl(value) }
pub(super) fn normalize_protocol(value: &str) -> String { preflight::normalize_protocol_impl(value) }
pub(super) fn normalize_settings(raw: BTreeMap<String, String>) -> BTreeMap<String, String> { preflight::normalize_settings_impl(raw) }
pub(super) fn required_runtime_tools(nodes: &[NormalizedNode], uses_openvpn: bool, uses_amnezia_native: bool, uses_amnezia_container: bool, uses_container_runtime: bool) -> BTreeSet<NetworkTool> { preflight::required_runtime_tools_impl(nodes, uses_openvpn, uses_amnezia_native, uses_amnezia_container, uses_container_runtime) }
pub(super) fn trim_option(value: Option<String>) -> Option<String> { preflight::trim_option_impl(value) }

pub(super) use parse::{
    amnezia_conf_contains_native_fields_impl as amnezia_conf_contains_native_fields,
    amnezia_json_contains_native_fields_impl as amnezia_json_contains_native_fields,
    decode_amnezia_json_impl as decode_amnezia_json,
    extract_awg_payload_impl as extract_awg_payload,
    extract_string_impl as extract_string,
    is_amnezia_native_only_key_impl as is_amnezia_native_only_key,
    looks_like_amnezia_conf_impl as looks_like_amnezia_conf,
    parse_amnezia_runtime_config_impl as parse_amnezia_runtime_config,
};

pub(super) fn amnezia_tunnel_service_exists(tunnel_name: &str) -> bool { amnezia_service::amnezia_tunnel_service_exists_impl(tunnel_name) }
pub(super) fn delete_amnezia_tunnel_service(tunnel_name: &str) -> Result<(), String> { amnezia_service::delete_amnezia_tunnel_service_impl(tunnel_name) }
pub(super) fn describe_amnezia_tunnel_status(tunnel_name: &str) -> String { amnezia_service::describe_amnezia_tunnel_status_impl(tunnel_name) }
pub(super) fn is_amnezia_access_denied(output: &Output) -> bool { amnezia_service::is_amnezia_access_denied_impl(output) }
pub(super) fn is_amnezia_tunnel_active(tunnel_name: &str) -> bool { amnezia_service::is_amnezia_tunnel_active_impl(tunnel_name) }
pub(super) fn is_uac_elevation_cancelled(output: &Output) -> bool { amnezia_service::is_uac_elevation_cancelled_impl(output) }
pub(super) fn run_amneziawg_command_elevated(binary: &PathBuf, args: &[String], action: &str) -> Result<Output, String> { amnezia_service::run_amneziawg_command_elevated_impl(binary, args, action) }
pub(super) fn run_amneziawg_command(binary: &PathBuf, args: &[String], action: &str) -> Result<Output, String> { amnezia_service::run_amneziawg_command_impl(binary, args, action) }
pub(super) fn start_amnezia_tunnel_service(tunnel_name: &str) -> Result<(), String> { amnezia_service::start_amnezia_tunnel_service_impl(tunnel_name) }
pub(super) fn stop_amnezia_tunnel_service(tunnel_name: &str) -> Result<(), String> { amnezia_service::stop_amnezia_tunnel_service_impl(tunnel_name) }
pub(super) fn uninstall_amnezia_tunnel(binary: &PathBuf, tunnel_name: &str) -> Result<(), String> { amnezia_service::uninstall_amnezia_tunnel_impl(binary, tunnel_name) }
pub(super) fn wait_amnezia_tunnel_state(tunnel_name: &str, should_be_active: bool, timeout_ms: u64) -> Result<(), String> { amnezia_service::wait_amnezia_tunnel_state_impl(tunnel_name, should_be_active, timeout_ms) }
#[cfg(target_os = "windows")]
pub(super) fn amnezia_service_name(tunnel_name: &str) -> String { amnezia_service::amnezia_service_name_impl(tunnel_name) }
#[cfg(target_os = "windows")]
pub(super) fn escape_powershell_single_quoted(value: &str) -> String { amnezia_service::escape_powershell_single_quoted_impl(value) }
#[cfg(target_os = "windows")]
pub(super) fn run_sc_command_elevated(args: &[String], action: &str) -> Result<Output, String> { amnezia_service::run_sc_command_elevated_impl(args, action) }

pub(super) fn describe_process_failure(output: &Output, label: &str) -> String {
    diagnostics::describe_process_failure_impl(output, label)
}

#[cfg(target_os = "windows")]
pub(super) fn parse_sc_state_code(raw: &str) -> Option<u32> {
    diagnostics::parse_sc_state_code_impl(raw)
}

pub(super) fn amnezia_tunnel_name(profile_id: Uuid) -> String {
    amnezia::amnezia_tunnel_name_impl(profile_id)
}

pub(super) fn build_amnezia_native_config_text(value: &str) -> Result<String, String> {
    amnezia::build_amnezia_native_config_text_impl(value)
}

pub(super) fn sanitize_amnezia_conf_text(value: &str) -> String {
    amnezia::sanitize_amnezia_conf_text_impl(value)
}

pub(super) fn extract_string_case_insensitive(value: &Value, expected_key: &str) -> Option<String> {
    let map = value.as_object()?;
    for (key, raw) in map {
        if !key.eq_ignore_ascii_case(expected_key) {
            continue;
        }
        if let Some(text) = raw.as_str() {
            return Some(text.to_string());
        }
        if let Some(number) = raw.as_i64() {
            return Some(number.to_string());
        }
        if let Some(number) = raw.as_u64() {
            return Some(number.to_string());
        }
        if let Some(number) = raw.as_f64() {
            return Some(number.to_string());
        }
    }
    None
}

pub(super) fn extract_amnezia_conf_text_from_payload(root: &Value, awg: &Value) -> Option<String> {
    let last_config = awg.get("last_config").cloned();
    let config_candidate = match last_config {
        Some(Value::String(raw)) => serde_json::from_str::<Value>(&raw).ok().and_then(|parsed| {
            extract_string(&parsed, &["config"]).or_else(|| extract_string(awg, &["config"]))
        }),
        Some(Value::Object(map)) => {
            let value = Value::Object(map);
            extract_string(&value, &["config"]).or_else(|| extract_string(awg, &["config"]))
        }
        _ => extract_string(awg, &["config"]),
    }?;
    let mut config = config_candidate.replace('\r', "");
    let primary_dns = extract_string(root, &["dns1", "primary_dns", "primaryDns"])
        .or_else(|| extract_string(awg, &["dns1", "primary_dns", "primaryDns"]))
        .unwrap_or_else(|| "1.2.3.1".to_string());
    let secondary_dns = extract_string(root, &["dns2", "secondary_dns", "secondaryDns"])
        .or_else(|| extract_string(awg, &["dns2", "secondary_dns", "secondaryDns"]))
        .unwrap_or_else(|| "1.0.0.1".to_string());
    config = config.replace("$PRIMARY_DNS", primary_dns.trim());
    config = config.replace("$SECONDARY_DNS", secondary_dns.trim());
    if !config.ends_with('\n') {
        config.push('\n');
    }
    Some(sanitize_amnezia_conf_text(&config))
}

pub(super) fn extract_amnezia_dns_pair(root: &Value, awg: &Value) -> Option<String> {
    let dns1 = extract_string(root, &["dns1", "primary_dns", "primaryDns"])
        .or_else(|| extract_string(awg, &["dns1", "primary_dns", "primaryDns"]))?;
    let dns2 = extract_string(root, &["dns2", "secondary_dns", "secondaryDns"])
        .or_else(|| extract_string(awg, &["dns2", "secondary_dns", "secondaryDns"]))
        .unwrap_or_default();
    let first = dns1.trim();
    if first.is_empty() {
        return None;
    }
    let second = dns2.trim();
    if second.is_empty() {
        Some(first.to_string())
    } else {
        Some(format!("{first}, {second}"))
    }
}

pub(super) fn build_openvpn_auth_file(
    node: &NormalizedNode,
    runtime_dir: &PathBuf,
    profile_id: Uuid,
) -> Result<Option<PathBuf>, String> {
    openvpn::build_openvpn_auth_file_impl(node, runtime_dir, profile_id)
}

pub(super) fn build_openvpn_config_text(
    node: &NormalizedNode,
    auth_path: Option<&PathBuf>,
    log_path: &PathBuf,
) -> Result<String, String> {
    openvpn::build_openvpn_config_text_impl(node, auth_path, log_path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeExecutionTarget {
    Host,
    Container,
}

pub(super) fn run_sing_box_check(
    binary: &str,
    config_path: &PathBuf,
    log_path: &PathBuf,
) -> Result<(), String> {
    singbox::run_sing_box_check_impl(binary, config_path, log_path)
}

pub(super) fn build_runtime_config(
    app_handle: &AppHandle,
    nodes: &[NormalizedNode],
    listen_port: u16,
    log_path: &PathBuf,
    target: RuntimeExecutionTarget,
) -> Result<Value, String> {
    singbox::build_runtime_config_impl(app_handle, nodes, listen_port, log_path, target)
}

pub(super) fn terminate_pid(pid: u32) {
    cleanup::terminate_pid_impl(pid)
}

pub(super) fn is_container_runtime_active(container_name: &str) -> bool {
    cleanup::is_container_runtime_active_impl(container_name)
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct NormalizedNode {
    pub(super) connection_type: String,
    pub(super) protocol: String,
    pub(super) host: Option<String>,
    pub(super) port: Option<u16>,
    pub(super) username: Option<String>,
    pub(super) password: Option<String>,
    pub(super) bridges: Option<String>,
    pub(super) settings: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub(super) struct AmneziaRuntimeConfig {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) client_private_key: String,
    pub(super) server_public_key: String,
    pub(super) pre_shared_key: Option<String>,
    pub(super) addresses: Vec<String>,
    pub(super) allowed_ips: Vec<String>,
    pub(super) mtu: Option<u16>,
    pub(super) transport: Option<String>,
}
