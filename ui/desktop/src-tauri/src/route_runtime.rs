use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    net::TcpListener,
    path::PathBuf,
    process::{Command, Output},
    thread,
    time::Duration,
};

use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use flate2::read::ZlibDecoder;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    network_runtime::{
        ensure_network_runtime_tools, resolve_amneziawg_binary_path, resolve_openvpn_binary_path,
        resolve_sing_box_binary_path, resolve_tor_binary_path, resolve_tor_pt_binary_path,
        NetworkTool,
    },
    process_tracking::is_process_running,
    state::AppState,
};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteRuntimeBackend {
    SingBox,
    OpenVpn,
    AmneziaWg,
}

pub fn runtime_proxy_endpoint(app_handle: &AppHandle, profile_id: Uuid) -> Option<(String, u16)> {
    let state = app_handle.state::<AppState>();
    let runtime = state.route_runtime.lock().ok()?;
    let session = runtime.sessions.get(&profile_id.to_string())?;
    if !session_is_active(session) {
        return None;
    }
    match session.backend {
        RouteRuntimeBackend::SingBox => {
            let port = session.listen_port?;
            Some(("127.0.0.1".to_string(), port))
        }
        RouteRuntimeBackend::OpenVpn | RouteRuntimeBackend::AmneziaWg => None,
    }
}

pub fn runtime_session_active(app_handle: &AppHandle, profile_id: Uuid) -> bool {
    let state = app_handle.state::<AppState>();
    let runtime = match state.route_runtime.lock() {
        Ok(value) => value,
        Err(_) => return false,
    };
    runtime
        .sessions
        .get(&profile_id.to_string())
        .map(session_is_active)
        .unwrap_or(false)
}

fn session_is_active(session: &RouteRuntimeSession) -> bool {
    match session.backend {
        RouteRuntimeBackend::SingBox | RouteRuntimeBackend::OpenVpn => {
            session.pid.map(is_process_running).unwrap_or(false)
        }
        RouteRuntimeBackend::AmneziaWg => session
            .tunnel_name
            .as_deref()
            .map(is_amnezia_tunnel_active)
            .unwrap_or(false),
    }
}

pub fn route_runtime_required_for_profile(app_handle: &AppHandle, profile_id: Uuid) -> bool {
    let state = app_handle.state::<AppState>();
    let store = match state.network_store.lock() {
        Ok(value) => value,
        Err(_) => return false,
    };
    let profile_key = profile_id.to_string();
    let (route_mode, selected_template_id) = resolve_effective_route_selection(&store, &profile_key);
    if route_mode == "direct" {
        return false;
    }
    if store.global_route_settings.global_vpn_enabled {
        return true;
    }
    let Some(template_id) = selected_template_id.as_ref() else {
        return false;
    };
    let Some(template) = store.connection_templates.get(template_id) else {
        return false;
    };
    let nodes = normalized_nodes(template);
    if nodes.is_empty() {
        return false;
    }
    nodes.len() > 1
        || nodes
            .iter()
            .any(|node| !matches!(node.connection_type.as_str(), "proxy"))
}

pub fn stop_profile_route_runtime(app_handle: &AppHandle, profile_id: Uuid) {
    let state = app_handle.state::<AppState>();
    let key = profile_id.to_string();
    let session = {
        let mut runtime = match state.route_runtime.lock() {
            Ok(value) => value,
            Err(_) => return,
        };
        runtime.sessions.remove(&key)
    };
    if let Some(session) = session {
        if let Some(pid) = session.pid {
            terminate_pid(pid);
        }
        if session.backend == RouteRuntimeBackend::AmneziaWg {
            if let Some(tunnel_name) = session.tunnel_name.as_deref() {
                let _ = stop_amnezia_tunnel_service(tunnel_name);
                let _ = wait_amnezia_tunnel_state(tunnel_name, false, 8_000);
                if let Ok(binary) = resolve_amneziawg_binary_path(app_handle) {
                    let _ = uninstall_amnezia_tunnel(&binary, tunnel_name);
                }
            }
        }
        let _ = fs::remove_file(session.config_path);
        for path in session.cleanup_paths {
            let _ = fs::remove_file(path);
        }
    }
}

pub fn stop_all_route_runtime(app_handle: &AppHandle) {
    let sessions = {
        let state = app_handle.state::<AppState>();
        let runtime = match state.route_runtime.lock() {
            Ok(value) => value,
            Err(_) => return,
        };
        runtime
            .sessions
            .keys()
            .filter_map(|value| Uuid::parse_str(value).ok())
            .collect::<Vec<_>>()
    };
    for profile_id in sessions {
        stop_profile_route_runtime(app_handle, profile_id);
    }
}

pub fn ensure_profile_route_runtime(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let profile_key = profile_id.to_string();
    let (route_mode, template_id, template) = {
        let store = state
            .network_store
            .lock()
            .map_err(|_| "network store lock poisoned".to_string())?;
        let (route_mode, selected_id) = resolve_effective_route_selection(&store, &profile_key);
        let selected = selected_id
            .as_ref()
            .and_then(|id| store.connection_templates.get(id))
            .cloned();
        (route_mode, selected_id, selected)
    };

    if route_mode == "direct" {
        stop_profile_route_runtime(app_handle, profile_id);
        return Ok(());
    }

    let Some(template) = template else {
        stop_profile_route_runtime(app_handle, profile_id);
        return Ok(());
    };
    let template_id = template_id.unwrap_or_else(|| template.id.clone());
    let nodes = normalized_nodes(&template);
    if nodes.is_empty() {
        stop_profile_route_runtime(app_handle, profile_id);
        return Ok(());
    }

    let requires_runtime = nodes.len() > 1
        || nodes
            .iter()
            .any(|node| !matches!(node.connection_type.as_str(), "proxy"));
    if !requires_runtime {
        stop_profile_route_runtime(app_handle, profile_id);
        return Ok(());
    }

    let uses_amnezia_native = nodes.len() == 1
        && nodes[0].connection_type == "vpn"
        && nodes[0].protocol == "amnezia";
    let uses_openvpn = nodes
        .iter()
        .any(|node| node.connection_type == "vpn" && node.protocol == "openvpn");
    if uses_openvpn
        && !(nodes.len() == 1
            && nodes[0].connection_type == "vpn"
            && nodes[0].protocol == "openvpn")
    {
        return Err(
            "openvpn runtime currently supports only single-node VPN templates (without chain)"
                .to_string(),
        );
    }
    if !nodes.iter().all(node_supported_by_runtime) {
        let unsupported = nodes
            .iter()
            .filter(|node| !node_supported_by_runtime(node))
            .map(|node| format!("{}:{}", node.connection_type, node.protocol))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "route runtime does not support protocol chain yet: {unsupported}"
        ));
    }
    let required_tools = required_runtime_tools(&nodes, uses_openvpn, uses_amnezia_native);

    let signature_payload = json!({
        "route_mode": route_mode,
        "template_id": template_id,
        "nodes": nodes,
    });
    let signature = serde_json::to_string(&signature_payload).map_err(|e| e.to_string())?;
    {
        let runtime = state
            .route_runtime
            .lock()
            .map_err(|_| "route runtime lock poisoned".to_string())?;
        if let Some(current) = runtime.sessions.get(&profile_key) {
            if current.signature == signature && session_is_active(current) {
                return Ok(());
            }
        }
    }

    ensure_network_runtime_tools(app_handle, &required_tools)?;
    stop_profile_route_runtime(app_handle, profile_id);

    let runtime_dir = state.profile_root.join(profile_id.to_string()).join("runtime");
    fs::create_dir_all(&runtime_dir).map_err(|e| format!("create runtime dir: {e}"))?;
    let sing_box_log_path = runtime_dir.join("sing-box-route.log");

    if uses_amnezia_native {
        let node = nodes
            .first()
            .ok_or_else(|| "amnezia runtime requires one node".to_string())?;
        let amnezia_binary = resolve_amneziawg_binary_path(app_handle)?
            .to_string_lossy()
            .to_string();
        let launch = launch_amneziawg_runtime(node, &runtime_dir, profile_id, &amnezia_binary)?;
        let mut runtime = state
            .route_runtime
            .lock()
            .map_err(|_| "route runtime lock poisoned".to_string())?;
        runtime.sessions.insert(
            profile_key,
            RouteRuntimeSession {
                signature,
                pid: None,
                backend: RouteRuntimeBackend::AmneziaWg,
                listen_port: None,
                config_path: launch.config_path,
                cleanup_paths: launch.cleanup_paths,
                tunnel_name: Some(launch.tunnel_name),
            },
        );
        return Ok(());
    }
    if uses_openvpn {
        let node = nodes
            .first()
            .ok_or_else(|| "openvpn runtime requires one node".to_string())?;
        let openvpn_binary = resolve_openvpn_binary_path(app_handle)?
            .to_string_lossy()
            .to_string();
        let openvpn = launch_openvpn_runtime(node, &runtime_dir, profile_id, &openvpn_binary)?;
        let mut runtime = state
            .route_runtime
            .lock()
            .map_err(|_| "route runtime lock poisoned".to_string())?;
        runtime.sessions.insert(
            profile_key,
            RouteRuntimeSession {
                signature,
                pid: Some(openvpn.pid),
                backend: RouteRuntimeBackend::OpenVpn,
                listen_port: None,
                config_path: openvpn.config_path,
                cleanup_paths: openvpn.cleanup_paths,
                tunnel_name: None,
            },
        );
        return Ok(());
    }

    let local_port = reserve_local_port()?;
    let _ = fs::remove_file(&sing_box_log_path);
    let config = build_runtime_config(app_handle, &nodes, local_port, &sing_box_log_path)?;
    let config_path = runtime_dir.join("sing-box-route.json");
    let config_bytes = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&config_path, config_bytes)
        .map_err(|e| format!("write route runtime config: {e}"))?;

    let binary = resolve_sing_box_binary_path(app_handle)?
        .to_string_lossy()
        .to_string();
    run_sing_box_check(&binary, &config_path, &sing_box_log_path)?;
    let mut command = Command::new(&binary);
    command.arg("run").arg("-c").arg(&config_path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn sing-box route runtime failed: {e}"))?;
    let pid = child.id();
    if pid == 0 {
        return Err("route runtime spawn failed: empty pid".to_string());
    }
    thread::sleep(Duration::from_millis(300));
    if let Some(status) = child
        .try_wait()
        .map_err(|e| format!("route runtime process status check failed: {e}"))?
    {
        let code = status
            .code()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "terminated".to_string());
        let log = fs::read_to_string(&sing_box_log_path).unwrap_or_default();
        let summary = tail_lines(&log, 20);
        return Err(format!(
            "route runtime exited immediately (code {code}){}",
            if summary.is_empty() {
                String::new()
            } else {
                format!(": {summary}")
            }
        ));
    }

    let mut runtime = state
        .route_runtime
        .lock()
        .map_err(|_| "route runtime lock poisoned".to_string())?;
    runtime.sessions.insert(
        profile_key,
        RouteRuntimeSession {
            signature,
            pid: Some(pid),
            backend: RouteRuntimeBackend::SingBox,
            listen_port: Some(local_port),
            config_path,
            cleanup_paths: vec![sing_box_log_path],
            tunnel_name: None,
        },
    );

    Ok(())
}

fn resolve_effective_route_selection(
    store: &crate::state::NetworkStore,
    profile_key: &str,
) -> (String, Option<String>) {
    let profile_route_mode = store
        .vpn_proxy
        .get(profile_key)
        .map(|value| value.route_mode.trim().to_lowercase())
        .unwrap_or_else(|| "direct".to_string());
    if profile_route_mode == "direct" {
        return ("direct".to_string(), None);
    }
    if store.global_route_settings.global_vpn_enabled {
        let template_id = store
            .global_route_settings
            .default_template_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        return ("vpn".to_string(), template_id);
    }
    let template_id = store.profile_template_selection.get(profile_key).cloned();
    (profile_route_mode, template_id)
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

#[derive(Debug)]
struct OpenVpnLaunch {
    pid: u32,
    config_path: PathBuf,
    cleanup_paths: Vec<PathBuf>,
}

#[derive(Debug)]
struct AmneziaWgLaunch {
    config_path: PathBuf,
    cleanup_paths: Vec<PathBuf>,
    tunnel_name: String,
}

fn launch_amneziawg_runtime(
    node: &NormalizedNode,
    runtime_dir: &PathBuf,
    profile_id: Uuid,
    binary: &str,
) -> Result<AmneziaWgLaunch, String> {
    let key = node
        .settings
        .get("amneziaKey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "amnezia key is required".to_string())?;
    let tunnel_name = amnezia_tunnel_name(profile_id);
    let config_path = runtime_dir.join(format!("{tunnel_name}.conf"));
    let config_text = build_amnezia_native_config_text(key)?;
    fs::write(&config_path, config_text).map_err(|e| format!("write amnezia config: {e}"))?;
    let binary_path = PathBuf::from(binary);

    if amnezia_tunnel_service_exists(&tunnel_name) {
        let _ = stop_amnezia_tunnel_service(&tunnel_name);
        let _ = wait_amnezia_tunnel_state(&tunnel_name, false, 8_000);
        uninstall_amnezia_tunnel(&binary_path, &tunnel_name)
            .map_err(|error| format!("failed to reset existing amneziawg tunnel service: {error}"))?;
    }

    install_amnezia_tunnel(&binary_path, &config_path, &tunnel_name)?;
    if let Err(error) = set_amnezia_tunnel_service_start_mode(&tunnel_name, "demand") {
        let cleanup_error = uninstall_amnezia_tunnel(&binary_path, &tunnel_name).err();
        return Err(match cleanup_error {
            Some(cleanup) => format!("{error}. Cleanup failed: {cleanup}"),
            None => error,
        });
    }
    if !is_amnezia_tunnel_active(&tunnel_name) {
        start_amnezia_tunnel_service(&tunnel_name)?;
    }

    if let Err(error) = wait_amnezia_tunnel_state(&tunnel_name, true, 45_000) {
        let status = describe_amnezia_tunnel_status(&tunnel_name);
        let cleanup_error = uninstall_amnezia_tunnel(&binary_path, &tunnel_name).err();
        return Err(match cleanup_error {
            Some(cleanup) => format!("{error}. {status}. Cleanup failed: {cleanup}"),
            None => format!("{error}. {status}"),
        });
    }

    Ok(AmneziaWgLaunch {
        config_path,
        cleanup_paths: Vec::new(),
        tunnel_name,
    })
}

fn install_amnezia_tunnel(
    binary: &PathBuf,
    config_path: &PathBuf,
    tunnel_name: &str,
) -> Result<(), String> {
    let args = vec![
        "/installtunnelservice".to_string(),
        config_path.to_string_lossy().to_string(),
    ];
    let output = run_amneziawg_command(binary, &args, "install tunnel")?;
    if output.status.success() {
        return Ok(());
    }
    if is_amnezia_access_denied(&output) {
        let elevated = run_amneziawg_command_elevated(binary, &args, "install tunnel");
        match elevated {
            Ok(out) if out.status.success() => return Ok(()),
            Ok(out) => {
                if is_uac_elevation_cancelled(&out) {
                    return Err(
                        "amneziawg tunnel install requires administrator approval (UAC was cancelled)"
                            .to_string(),
                    );
                }
                let reason = describe_process_failure(&out, "amneziawg elevated install");
                let _ = uninstall_amnezia_tunnel(binary, tunnel_name);
                return Err(format!(
                    "amneziawg tunnel install failed after elevation attempt: {reason}"
                ));
            }
            Err(error) => {
                let _ = uninstall_amnezia_tunnel(binary, tunnel_name);
                return Err(format!(
                    "amneziawg tunnel install requires administrator privileges: {error}"
                ));
            }
        }
    }
    let reason = describe_process_failure(&output, "amneziawg install");
    let _ = uninstall_amnezia_tunnel(binary, tunnel_name);
    Err(format!("amneziawg tunnel install failed: {reason}"))
}

fn amnezia_tunnel_name(profile_id: Uuid) -> String {
    let mut name = format!("awg-{}", profile_id.as_simple());
    if name.len() > 32 {
        name.truncate(32);
    }
    name
}

fn build_amnezia_native_config_text(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("amnezia key is required".to_string());
    }
    if looks_like_amnezia_conf(trimmed) {
        return Ok(sanitize_amnezia_conf_text(trimmed));
    }

    let root = decode_amnezia_json(trimmed)?;
    let awg = extract_awg_payload(&root)
        .ok_or_else(|| "amnezia key does not contain awg payload".to_string())?;
    if let Some(config) = extract_amnezia_conf_text_from_payload(&root, &awg) {
        return Ok(config);
    }

    let config = parse_amnezia_runtime_config(trimmed)?;
    let mut lines = Vec::new();
    lines.push("[Interface]".to_string());
    if !config.addresses.is_empty() {
        lines.push(format!("Address = {}", config.addresses.join(", ")));
    }
    if let Some(dns) = extract_amnezia_dns_pair(&root, &awg) {
        lines.push(format!("DNS = {dns}"));
    }
    lines.push(format!("PrivateKey = {}", config.client_private_key));
    if let Some(mtu) = config.mtu {
        lines.push(format!("MTU = {mtu}"));
    }

    for key in [
        "Jc", "Jmin", "Jmax", "S1", "S2", "S3", "S4", "H1", "H2", "H3", "H4", "I1", "I2", "I3",
        "I4", "I5",
    ] {
        if let Some(raw) = extract_string_case_insensitive(&awg, key) {
            lines.push(format!("{key} = {raw}"));
        }
    }

    lines.push(String::new());
    lines.push("[Peer]".to_string());
    lines.push(format!("PublicKey = {}", config.server_public_key));
    if let Some(psk) = config.pre_shared_key.filter(|item| !item.trim().is_empty()) {
        lines.push(format!("PresharedKey = {}", psk.trim()));
    }
    lines.push(format!("AllowedIPs = {}", config.allowed_ips.join(", ")));
    lines.push(format!("Endpoint = {}:{}", config.host, config.port));
    if let Some(keepalive) =
        extract_string_case_insensitive(&awg, "persistent_keep_alive").or_else(|| {
            extract_string_case_insensitive(&awg, "persistentKeepalive")
                .or_else(|| extract_string_case_insensitive(&awg, "persistent_keepalive"))
        })
    {
        let trimmed_keepalive = keepalive.trim();
        if !trimmed_keepalive.is_empty() {
            lines.push(format!("PersistentKeepalive = {trimmed_keepalive}"));
        }
    }
    Ok(lines.join("\n") + "\n")
}

fn sanitize_amnezia_conf_text(value: &str) -> String {
    let mut cleaned = Vec::new();
    for raw_line in value.replace('\r', "").lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            cleaned.push(String::new());
            continue;
        }
        let Some((left, right)) = line.split_once('=') else {
            cleaned.push(line.to_string());
            continue;
        };
        let key = left.trim();
        let val = right.trim();
        if ["I1", "I2", "I3", "I4", "I5"]
            .iter()
            .any(|item| key.eq_ignore_ascii_case(item))
            && val.is_empty()
        {
            continue;
        }
        cleaned.push(format!("{key} = {val}"));
    }
    let mut text = cleaned.join("\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn extract_amnezia_conf_text_from_payload(root: &Value, awg: &Value) -> Option<String> {
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
        .unwrap_or_else(|| "1.1.1.1".to_string());
    let secondary_dns = extract_string(root, &["dns2", "secondary_dns", "secondaryDns"])
        .or_else(|| extract_string(awg, &["dns2", "secondary_dns", "secondaryDns"]))
        .unwrap_or_else(|| "1.0.0.1".to_string());
    config = config.replace("$PRIMARY_DNS", primary_dns.trim());
    config = config.replace("$SECONDARY_DNS", secondary_dns.trim());
    if !config.ends_with('\n') {
        config.push('\n');
    }
    Some(config)
}

fn extract_amnezia_dns_pair(root: &Value, awg: &Value) -> Option<String> {
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

fn extract_string_case_insensitive(value: &Value, expected_key: &str) -> Option<String> {
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

fn uninstall_amnezia_tunnel(binary: &PathBuf, tunnel_name: &str) -> Result<(), String> {
    let args = vec![
        "/uninstalltunnelservice".to_string(),
        tunnel_name.to_string(),
    ];
    let output = run_amneziawg_command(binary, &args, "uninstall tunnel")?;
    if output.status.success() || !amnezia_tunnel_service_exists(tunnel_name) {
        return Ok(());
    }
    if is_amnezia_access_denied(&output) {
        let elevated = run_amneziawg_command_elevated(binary, &args, "uninstall tunnel");
        match elevated {
            Ok(out) if out.status.success() || !amnezia_tunnel_service_exists(tunnel_name) => {
                return Ok(());
            }
            Ok(out) => {
                if is_uac_elevation_cancelled(&out) {
                    return Err(
                        "amneziawg tunnel uninstall requires administrator approval (UAC was cancelled)"
                            .to_string(),
                    );
                }
                let reason = describe_process_failure(&out, "amneziawg elevated uninstall");
                return Err(format!("amneziawg tunnel uninstall failed: {reason}"));
            }
            Err(error) => {
                return Err(format!(
                    "amneziawg tunnel uninstall requires administrator privileges: {error}"
                ));
            }
        }
    }
    let reason = describe_process_failure(&output, "amneziawg uninstall");
    Err(format!("amneziawg tunnel uninstall failed: {reason}"))
}

fn start_amnezia_tunnel_service(tunnel_name: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let service_name = format!("AmneziaWGTunnel${tunnel_name}");
        let output = Command::new("sc.exe")
            .arg("start")
            .arg(&service_name)
            .output()
            .map_err(|e| format!("start amneziawg tunnel service failed: {e}"))?;
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .to_lowercase();
        if output.status.success()
            || text.contains("already running")
            || text.contains("service has already been started")
            || text.contains("service is already running")
        {
            return Ok(());
        }
        return Err(format!(
            "unable to start amneziawg tunnel service: {}",
            tail_lines(&text, 12)
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = tunnel_name;
        Err("amneziawg service start is only supported on Windows".to_string())
    }
}

fn stop_amnezia_tunnel_service(tunnel_name: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let service_name = format!("AmneziaWGTunnel${tunnel_name}");
        let output = Command::new("sc.exe")
            .arg("stop")
            .arg(&service_name)
            .output()
            .map_err(|e| format!("stop amneziawg tunnel service failed: {e}"))?;
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .to_lowercase();
        if output.status.success()
            || text.contains("service has not been started")
            || text.contains("service is not started")
            || text.contains("service was stopped")
            || text.contains("service cannot accept control messages")
        {
            return Ok(());
        }
        return Err(format!(
            "unable to stop amneziawg tunnel service: {}",
            tail_lines(&text, 12)
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = tunnel_name;
        Err("amneziawg service stop is only supported on Windows".to_string())
    }
}

fn set_amnezia_tunnel_service_start_mode(tunnel_name: &str, start_mode: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let service_name = amnezia_service_name(tunnel_name);
        let args = vec![
            "config".to_string(),
            service_name.clone(),
            "start=".to_string(),
            start_mode.to_string(),
        ];
        let output = Command::new("sc.exe")
            .arg("config")
            .arg(&service_name)
            .arg("start=")
            .arg(start_mode)
            .output()
            .map_err(|e| format!("set amneziawg tunnel service start mode failed: {e}"))?;
        if output.status.success() {
            return Ok(());
        }
        if is_amnezia_access_denied(&output) {
            let elevated = run_sc_command_elevated(&args, "configure amneziawg service start mode");
            match elevated {
                Ok(out) if out.status.success() => return Ok(()),
                Ok(out) if is_uac_elevation_cancelled(&out) => {
                    return Err(
                        "amneziawg service start mode update requires administrator approval (UAC was cancelled)"
                            .to_string(),
                    )
                }
                Ok(_) | Err(_) => {}
            }
        }
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(format!(
            "unable to set amneziawg tunnel service start mode to {start_mode}: {}",
            tail_lines(&text, 12)
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = tunnel_name;
        let _ = start_mode;
        Err("amneziawg service configuration is only supported on Windows".to_string())
    }
}

#[cfg(target_os = "windows")]
fn run_sc_command_elevated(args: &[String], action: &str) -> Result<Output, String> {
    let arg_list = args
        .iter()
        .map(|value| format!("'{}'", escape_powershell_single_quoted(value)))
        .collect::<Vec<_>>()
        .join(", ");
    let script = format!(
        "$p = Start-Process -FilePath 'sc.exe' -ArgumentList @({arg_list}) -Verb RunAs -WindowStyle Hidden -PassThru -Wait; exit $p.ExitCode"
    );
    let mut command = Command::new("powershell.exe");
    command
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script);
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .output()
        .map_err(|e| format!("spawn elevated sc.exe {action} failed: {e}"))
}

fn run_amneziawg_command(binary: &PathBuf, args: &[String], action: &str) -> Result<Output, String> {
    let mut command = Command::new(binary);
    for arg in args {
        command.arg(arg);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .output()
        .map_err(|e| format!("spawn amneziawg {action} failed: {e}"))
}

#[cfg(target_os = "windows")]
fn run_amneziawg_command_elevated(
    binary: &PathBuf,
    args: &[String],
    action: &str,
) -> Result<Output, String> {
    let file = escape_powershell_single_quoted(&binary.to_string_lossy());
    let arg_list = args
        .iter()
        .map(|value| format!("'{}'", escape_powershell_single_quoted(value)))
        .collect::<Vec<_>>()
        .join(", ");
    let script = format!(
        "$p = Start-Process -FilePath '{file}' -ArgumentList @({arg_list}) -Verb RunAs -WindowStyle Hidden -PassThru -Wait; exit $p.ExitCode"
    );
    let mut command = Command::new("powershell.exe");
    command
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script);
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .output()
        .map_err(|e| format!("spawn elevated amneziawg {action} failed: {e}"))
}

#[cfg(not(target_os = "windows"))]
fn run_amneziawg_command_elevated(
    _binary: &PathBuf,
    _args: &[String],
    _action: &str,
) -> Result<Output, String> {
    Err("amneziawg elevation is only supported on Windows".to_string())
}

fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

fn is_amnezia_access_denied(output: &Output) -> bool {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = text.to_lowercase();
    lower.contains("access is denied")
        || lower.contains("отказано в доступе")
        || lower.contains("error 5")
        || lower.contains("os error 5")
}

fn is_uac_elevation_cancelled(output: &Output) -> bool {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = text.to_lowercase();
    lower.contains("operation was canceled by the user")
        || lower.contains("the operation was canceled")
        || lower.contains("операция отменена пользователем")
}

fn describe_process_failure(output: &Output, label: &str) -> String {
    let tail = tail_lines(
        &format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        20,
    );
    if tail.is_empty() {
        format!("{label} exited with code {:?}", output.status.code())
    } else {
        tail
    }
}

#[derive(Debug, Clone)]
struct AmneziaServiceSnapshot {
    exists: bool,
    state_code: Option<u32>,
    raw_output: String,
}

fn amnezia_service_name(tunnel_name: &str) -> String {
    format!("AmneziaWGTunnel${tunnel_name}")
}

fn parse_sc_state_code(raw: &str) -> Option<u32> {
    for line in raw.lines() {
        let Some((_, right)) = line.split_once(':') else {
            continue;
        };
        let token = right.split_whitespace().next().unwrap_or_default();
        let Ok(code) = token.parse::<u32>() else {
            continue;
        };
        if (1..=7).contains(&code) {
            return Some(code);
        }
    }
    None
}

fn query_amnezia_tunnel_service(tunnel_name: &str) -> AmneziaServiceSnapshot {
    #[cfg(target_os = "windows")]
    {
        let service_name = amnezia_service_name(tunnel_name);
        let output = Command::new("sc.exe").arg("query").arg(&service_name).output();
        let Ok(output) = output else {
            return AmneziaServiceSnapshot {
                exists: false,
                state_code: None,
                raw_output: String::new(),
            };
        };
        let raw_output = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .trim()
        .to_string();
        if !output.status.success() {
            return AmneziaServiceSnapshot {
                exists: false,
                state_code: None,
                raw_output,
            };
        }
        let state_code = parse_sc_state_code(&raw_output);
        return AmneziaServiceSnapshot {
            exists: true,
            state_code,
            raw_output,
        };
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = tunnel_name;
        AmneziaServiceSnapshot {
            exists: false,
            state_code: None,
            raw_output: String::new(),
        }
    }
}

fn amnezia_tunnel_service_exists(tunnel_name: &str) -> bool {
    query_amnezia_tunnel_service(tunnel_name).exists
}

fn describe_amnezia_tunnel_status(tunnel_name: &str) -> String {
    let snapshot = query_amnezia_tunnel_service(tunnel_name);
    if !snapshot.exists {
        return "service is not installed".to_string();
    }
    let state = snapshot
        .state_code
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let details = tail_lines(&snapshot.raw_output, 12);
    if details.is_empty() {
        format!("service state code: {state}")
    } else {
        format!("service state code: {state}; details: {details}")
    }
}

fn wait_amnezia_tunnel_state(
    tunnel_name: &str,
    should_be_active: bool,
    timeout_ms: u64,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    loop {
        let snapshot = query_amnezia_tunnel_service(tunnel_name);
        let active = snapshot.state_code == Some(4);
        if active == should_be_active {
            return Ok(());
        }
        if should_be_active && snapshot.exists && snapshot.state_code == Some(1) {
            let details = tail_lines(&snapshot.raw_output, 12);
            if details.is_empty() {
                return Err(
                    "amneziawg tunnel service entered STOPPED state before RUNNING".to_string(),
                );
            }
            return Err(format!(
                "amneziawg tunnel service entered STOPPED state before RUNNING: {details}"
            ));
        }
        if started.elapsed() >= Duration::from_millis(timeout_ms.max(1)) {
            return Err(if should_be_active {
                format!("amneziawg tunnel service did not reach RUNNING within {timeout_ms} ms")
            } else {
                format!(
                    "amneziawg tunnel service did not stop/uninstall within {} ms",
                    timeout_ms
                )
            });
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn is_amnezia_tunnel_active(tunnel_name: &str) -> bool {
    query_amnezia_tunnel_service(tunnel_name).state_code == Some(4)
}

fn launch_openvpn_runtime(
    node: &NormalizedNode,
    runtime_dir: &PathBuf,
    profile_id: Uuid,
    binary: &str,
) -> Result<OpenVpnLaunch, String> {
    let config_path = runtime_dir.join("openvpn-route.ovpn");
    let log_path = runtime_dir.join("openvpn-route.log");
    let auth_path = build_openvpn_auth_file(node, runtime_dir, profile_id)?;
    let config_text = build_openvpn_config_text(node, auth_path.as_ref(), &log_path)?;
    fs::write(&config_path, config_text).map_err(|e| format!("write openvpn config: {e}"))?;
    let _ = fs::remove_file(&log_path);

    let mut command = Command::new(binary);
    command
        .arg("--config")
        .arg(&config_path)
        .arg("--verb")
        .arg("3")
        .arg("--log")
        .arg(&log_path)
        .arg("--suppress-timestamps");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn openvpn runtime failed: {e}"))?;
    let pid = child.id();
    if pid == 0 {
        return Err("openvpn runtime spawn failed: empty pid".to_string());
    }
    thread::sleep(Duration::from_millis(400));
    if let Some(status) = child
        .try_wait()
        .map_err(|e| format!("openvpn runtime process status check failed: {e}"))?
    {
        let code = status
            .code()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "terminated".to_string());
        let log = fs::read_to_string(&log_path).unwrap_or_default();
        let summary = tail_lines(&log, 16);
        return Err(format!(
            "openvpn runtime exited immediately (code {code}){}",
            if summary.is_empty() {
                String::new()
            } else {
                format!(": {summary}")
            }
        ));
    }
    wait_openvpn_connected(pid, &log_path, 20_000)?;

    let mut cleanup_paths = vec![log_path];
    if let Some(path) = auth_path {
        cleanup_paths.push(path);
    }
    Ok(OpenVpnLaunch {
        pid,
        config_path,
        cleanup_paths,
    })
}

fn build_openvpn_auth_file(
    node: &NormalizedNode,
    runtime_dir: &PathBuf,
    profile_id: Uuid,
) -> Result<Option<PathBuf>, String> {
    let username = node.username.as_deref().unwrap_or_default().trim();
    let password = node.password.as_deref().unwrap_or_default().trim();
    if username.is_empty() && password.is_empty() {
        return Ok(None);
    }
    if username.is_empty() {
        return Err("openvpn username is required when password is set".to_string());
    }
    let path = runtime_dir.join(format!("openvpn-auth-{}.txt", profile_id.as_simple()));
    fs::write(&path, format!("{username}\n{password}\n"))
        .map_err(|e| format!("write openvpn auth file: {e}"))?;
    Ok(Some(path))
}

fn build_openvpn_config_text(
    node: &NormalizedNode,
    auth_path: Option<&PathBuf>,
    log_path: &PathBuf,
) -> Result<String, String> {
    let host = node
        .host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "openvpn host is required".to_string())?;
    let port = node
        .port
        .filter(|value| *value > 0)
        .ok_or_else(|| "openvpn port is required".to_string())?;
    let transport = node
        .settings
        .get("transport")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| value.eq_ignore_ascii_case("udp") || value.eq_ignore_ascii_case("tcp"))
        .unwrap_or("udp")
        .to_ascii_lowercase();

    if let Some(raw) = node
        .settings
        .get("ovpnRaw")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let raw_lower = raw.to_ascii_lowercase();
        if auth_path.is_none()
            && (raw_lower.starts_with("auth-user-pass")
                || raw_lower.contains("\nauth-user-pass")
                || raw_lower.contains("\r\nauth-user-pass"))
            && !raw_lower.contains("<auth-user-pass>")
        {
            return Err(
                "openvpn profile requests auth-user-pass; set username/password fields".to_string(),
            );
        }
        let mut out = raw.replace('\r', "");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("client\n");
        out.push_str("nobind\n");
        out.push_str("persist-key\n");
        out.push_str("persist-tun\n");
        out.push_str(&format!("proto {transport}\n"));
        out.push_str(&format!("remote {host} {port}\n"));
        out.push_str("auth-retry nointeract\n");
        out.push_str("remote-cert-tls server\n");
        if let Some(path) = auth_path {
            out.push_str(&format!(
                "auth-user-pass \"{}\"\n",
                path.to_string_lossy().replace('\\', "\\\\")
            ));
        }
        out.push_str(&format!(
            "log \"{}\"\n",
            log_path.to_string_lossy().replace('\\', "\\\\")
        ));
        out.push_str("verb 3\n");
        return Ok(out);
    }

    let mut lines = vec![
        "client".to_string(),
        "dev tun".to_string(),
        format!("proto {transport}"),
        format!("remote {host} {port}"),
        "resolv-retry infinite".to_string(),
        "nobind".to_string(),
        "persist-key".to_string(),
        "persist-tun".to_string(),
        "auth-retry nointeract".to_string(),
        "remote-cert-tls server".to_string(),
        "verb 3".to_string(),
    ];

    if let Some(path) = auth_path {
        lines.push(format!(
            "auth-user-pass \"{}\"",
            path.to_string_lossy().replace('\\', "\\\\")
        ));
    }

    for (setting_key, inline_tag) in [
        ("caCert", "ca"),
        ("clientCert", "cert"),
        ("clientKey", "key"),
    ] {
        if let Some(value) = node
            .settings
            .get(setting_key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            lines.push(format!("<{inline_tag}>"));
            lines.extend(value.replace('\r', "").lines().map(|line| line.to_string()));
            lines.push(format!("</{inline_tag}>"));
        }
    }

    lines.push(format!(
        "log \"{}\"",
        log_path.to_string_lossy().replace('\\', "\\\\")
    ));
    Ok(lines.join("\n") + "\n")
}

fn wait_openvpn_connected(pid: u32, log_path: &PathBuf, timeout_ms: u64) -> Result<(), String> {
    let started = std::time::Instant::now();
    loop {
        if !is_process_running(pid) {
            let summary = fs::read_to_string(log_path)
                .ok()
                .map(|text| tail_lines(&text, 20))
                .unwrap_or_default();
            return Err(format!(
                "openvpn runtime terminated before connect{}",
                if summary.is_empty() {
                    String::new()
                } else {
                    format!(": {summary}")
                }
            ));
        }
        let log = fs::read_to_string(log_path).unwrap_or_default();
        if log.contains("Initialization Sequence Completed") {
            return Ok(());
        }
        if log.contains("AUTH_FAILED")
            || log.contains("Options error")
            || log.contains("TLS Error")
            || log.contains("Exiting due to fatal error")
        {
            let summary = tail_lines(&log, 20);
            return Err(format!("openvpn runtime failed to initialize: {summary}"));
        }
        if started.elapsed() >= Duration::from_millis(timeout_ms.max(1)) {
            let summary = tail_lines(&log, 20);
            return Err(format!(
                "openvpn runtime did not reach CONNECTED within {} ms{}",
                timeout_ms,
                if summary.is_empty() {
                    String::new()
                } else {
                    format!(": {summary}")
                }
            ));
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed
        .lines()
        .rev()
        .take(max_lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
}

fn run_sing_box_check(binary: &str, config_path: &PathBuf, log_path: &PathBuf) -> Result<(), String> {
    let output = Command::new(binary)
        .arg("check")
        .arg("-c")
        .arg(config_path)
        .output()
        .map_err(|e| format!("run sing-box config check failed: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    let summary = tail_lines(&combined, 20);
    let log = fs::read_to_string(log_path).unwrap_or_default();
    let log_summary = tail_lines(&log, 20);
    let message = if !summary.is_empty() {
        summary
    } else if !log_summary.is_empty() {
        log_summary
    } else {
        "unknown config error".to_string()
    };
    Err(format!("route runtime config check failed: {message}"))
}

fn build_runtime_config(
    app_handle: &AppHandle,
    nodes: &[NormalizedNode],
    listen_port: u16,
    log_path: &PathBuf,
) -> Result<Value, String> {
    let mut outbounds = Vec::new();
    let tags = nodes
        .iter()
        .enumerate()
        .map(|(idx, _)| format!("node-{}", idx + 1))
        .collect::<Vec<_>>();
    for (idx, node) in nodes.iter().enumerate() {
        let detour = if idx + 1 < tags.len() {
            Some(tags[idx + 1].clone())
        } else {
            None
        };
        let outbound = node_to_sing_box_outbound(app_handle, node, &tags[idx], detour)?;
        outbounds.push(outbound);
    }
    outbounds.push(json!({ "type": "direct", "tag": "direct" }));

    let config = json!({
        "log": {
            "disabled": false,
            "level": "info",
            "output": log_path.to_string_lossy().to_string(),
            "timestamp": true
        },
        "inbounds": [
            {
                "type": "mixed",
                "tag": "mixed-in",
                "listen": "127.0.0.1",
                "listen_port": listen_port
            }
        ],
        "outbounds": outbounds,
        "route": {
            "final": tags.first().cloned().unwrap_or_else(|| "direct".to_string())
        }
    });
    Ok(config)
}

fn node_to_sing_box_outbound(
    app_handle: &AppHandle,
    node: &NormalizedNode,
    tag: &str,
    detour: Option<String>,
) -> Result<Value, String> {
    let mut outbound = match node.connection_type.as_str() {
        "proxy" => proxy_outbound(node, tag)?,
        "v2ray" => v2ray_outbound(node, tag)?,
        "vpn" => vpn_outbound(node, tag)?,
        "tor" => tor_outbound(app_handle, node, tag)?,
        _ => return Err("unsupported node type for runtime".to_string()),
    };
    if let Some(detour_tag) = detour {
        if let Some(map) = outbound.as_object_mut() {
            map.insert("detour".to_string(), json!(detour_tag));
        }
    }
    Ok(outbound)
}

fn node_supported_by_runtime(node: &NormalizedNode) -> bool {
    match node.connection_type.as_str() {
        "proxy" => matches!(node.protocol.as_str(), "http" | "socks4" | "socks5"),
        "v2ray" => matches!(
            node.protocol.as_str(),
            "vmess" | "vless" | "trojan" | "shadowsocks"
        ),
        "vpn" => matches!(node.protocol.as_str(), "wireguard" | "amnezia" | "openvpn"),
        "tor" => matches!(
            node.protocol.as_str(),
            "obfs4" | "snowflake" | "meek" | "none"
        ),
        _ => false,
    }
}

fn proxy_outbound(node: &NormalizedNode, tag: &str) -> Result<Value, String> {
    let host = node
        .host
        .clone()
        .ok_or_else(|| "proxy host is required".to_string())?;
    let port = node
        .port
        .ok_or_else(|| "proxy port is required".to_string())?;
    match node.protocol.as_str() {
        "http" => {
            let mut out = json!({
                "type": "http",
                "tag": tag,
                "server": host,
                "server_port": port,
            });
            if let Some(map) = out.as_object_mut() {
                if let Some(user) = node.username.clone().filter(|value| !value.is_empty()) {
                    map.insert("username".to_string(), json!(user));
                }
                if let Some(pass) = node.password.clone().filter(|value| !value.is_empty()) {
                    map.insert("password".to_string(), json!(pass));
                }
            }
            Ok(out)
        }
        "socks4" | "socks5" => {
            let version = if node.protocol == "socks4" { "4" } else { "5" };
            let mut out = json!({
                "type": "socks",
                "tag": tag,
                "server": host,
                "server_port": port,
                "version": version,
            });
            if let Some(map) = out.as_object_mut() {
                if let Some(user) = node.username.clone().filter(|value| !value.is_empty()) {
                    map.insert("username".to_string(), json!(user));
                }
                if let Some(pass) = node.password.clone().filter(|value| !value.is_empty()) {
                    map.insert("password".to_string(), json!(pass));
                }
            }
            Ok(out)
        }
        _ => Err("unsupported proxy protocol for runtime".to_string()),
    }
}

fn v2ray_outbound(node: &NormalizedNode, tag: &str) -> Result<Value, String> {
    let host = node
        .host
        .clone()
        .ok_or_else(|| "v2ray host is required".to_string())?;
    let port = node
        .port
        .ok_or_else(|| "v2ray port is required".to_string())?;
    match node.protocol.as_str() {
        "vmess" => {
            let uuid = node
                .settings
                .get("uuid")
                .map(String::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if uuid.is_empty() {
                return Err("vmess uuid is required".to_string());
            }
            let alter_id = node
                .settings
                .get("alterId")
                .and_then(|value| value.trim().parse::<u32>().ok())
                .unwrap_or(0);
            let security = node
                .settings
                .get("security")
                .map(String::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("auto");
            let mut out = json!({
                "type": "vmess",
                "tag": tag,
                "server": host,
                "server_port": port,
                "uuid": uuid,
                "alter_id": alter_id,
                "security": security,
            });
            apply_v2ray_transport_and_tls(&mut out, node)?;
            Ok(out)
        }
        "vless" => {
            let uuid = node
                .settings
                .get("uuid")
                .map(String::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if uuid.is_empty() {
                return Err("vless uuid is required".to_string());
            }
            let mut out = json!({
                "type": "vless",
                "tag": tag,
                "server": host,
                "server_port": port,
                "uuid": uuid,
            });
            if let Some(flow) = node
                .settings
                .get("flow")
                .map(String::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                if let Some(map) = out.as_object_mut() {
                    map.insert("flow".to_string(), json!(flow));
                }
            }
            apply_v2ray_transport_and_tls(&mut out, node)?;
            Ok(out)
        }
        "trojan" => {
            let password = node
                .password
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            if password.is_empty() {
                return Err("trojan password is required".to_string());
            }
            let mut out = json!({
                "type": "trojan",
                "tag": tag,
                "server": host,
                "server_port": port,
                "password": password,
            });
            apply_v2ray_transport_and_tls(&mut out, node)?;
            if let Some(alpn) = node
                .settings
                .get("alpn")
                .map(String::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                if let Some(map) = out.as_object_mut() {
                    let tls = map
                        .entry("tls".to_string())
                        .or_insert_with(|| json!({ "enabled": true }));
                    if let Some(tls_map) = tls.as_object_mut() {
                        tls_map.insert(
                            "alpn".to_string(),
                            json!(alpn
                                .split(',')
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .collect::<Vec<_>>()),
                        );
                    }
                }
            }
            Ok(out)
        }
        "shadowsocks" => {
            let password = node
                .password
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            if password.is_empty() {
                return Err("shadowsocks password is required".to_string());
            }
            let method = node
                .settings
                .get("method")
                .map(String::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("aes-256-gcm");
            Ok(json!({
                "type": "shadowsocks",
                "tag": tag,
                "server": host,
                "server_port": port,
                "method": method,
                "password": password,
            }))
        }
        _ => Err("unsupported v2ray protocol for runtime".to_string()),
    }
}

fn apply_v2ray_transport_and_tls(outbound: &mut Value, node: &NormalizedNode) -> Result<(), String> {
    let network = node
        .settings
        .get("network")
        .map(String::as_str)
        .unwrap_or("tcp")
        .to_lowercase();
    if let Some(map) = outbound.as_object_mut() {
        map.insert("network".to_string(), json!(network.clone()));
        match network.as_str() {
            "ws" => {
                let path = node.settings.get("wsPath").cloned().unwrap_or_default();
                let ws_host = node.settings.get("wsHost").cloned().unwrap_or_default();
                let mut transport = json!({
                    "type": "ws",
                    "path": if path.trim().is_empty() { "/" } else { path.trim() },
                });
                if !ws_host.trim().is_empty() {
                    if let Some(transport_map) = transport.as_object_mut() {
                        transport_map
                            .insert("headers".to_string(), json!({ "Host": ws_host.trim() }));
                    }
                }
                map.insert("transport".to_string(), transport);
            }
            "grpc" => {
                let service = node
                    .settings
                    .get("wsPath")
                    .map(String::as_str)
                    .unwrap_or("TunService");
                map.insert(
                    "transport".to_string(),
                    json!({
                        "type": "grpc",
                        "service_name": service.trim().trim_start_matches('/'),
                    }),
                );
            }
            _ => {}
        }
        let security_mode = node
            .settings
            .get("securityMode")
            .map(String::as_str)
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                if node
                    .settings
                    .get("tls")
                    .map(|value| value.eq_ignore_ascii_case("on") || value.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
                {
                    "tls".to_string()
                } else {
                    "none".to_string()
                }
            });
        let tls_enabled = node
            .settings
            .get("tls")
            .map(|value| value.eq_ignore_ascii_case("on") || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if tls_enabled || matches!(security_mode.as_str(), "tls" | "reality") {
            let sni = node.settings.get("sni").cloned().unwrap_or_default();
            let mut tls = if sni.trim().is_empty() {
                json!({ "enabled": true })
            } else {
                json!({ "enabled": true, "server_name": sni.trim() })
            };
            if security_mode == "reality" {
                let public_key = node
                    .settings
                    .get("realityPublicKey")
                    .map(String::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "vless reality requires pbk/public key".to_string())?
                    .to_string();
                let short_id = node
                    .settings
                    .get("realityShortId")
                    .map(String::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or_default()
                    .to_string();
                let fingerprint = node
                    .settings
                    .get("realityFingerprint")
                    .or_else(|| node.settings.get("fp"))
                    .map(String::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("chrome")
                    .to_string();
                if let Some(tls_map) = tls.as_object_mut() {
                    tls_map.insert(
                        "utls".to_string(),
                        json!({
                            "enabled": true,
                            "fingerprint": fingerprint
                        }),
                    );
                    tls_map.insert(
                        "reality".to_string(),
                        json!({
                            "enabled": true,
                            "public_key": public_key,
                            "short_id": short_id
                        }),
                    );
                }
            }
            map.insert("tls".to_string(), tls);
        }
    }
    Ok(())
}

fn vpn_outbound(node: &NormalizedNode, tag: &str) -> Result<Value, String> {
    if node.protocol == "wireguard" {
        return wireguard_outbound(node, tag);
    }
    if node.protocol == "amnezia" {
        return amnezia_outbound(node, tag);
    }
    if node.protocol == "openvpn" {
        return Err("openvpn runtime requires native openvpn backend and is not yet available in sing-box mode".to_string());
    }
    Err("unsupported vpn protocol for runtime".to_string())
}

fn wireguard_outbound(node: &NormalizedNode, tag: &str) -> Result<Value, String> {
    let host = node
        .host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "wireguard host is required".to_string())?;
    let port = node
        .port
        .filter(|value| *value > 0)
        .ok_or_else(|| "wireguard port is required".to_string())?;
    let private_key = node
        .settings
        .get("privateKey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "wireguard private key is required".to_string())?;
    let public_key = node
        .settings
        .get("publicKey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "wireguard peer public key is required".to_string())?;
    let address = node
        .settings
        .get("address")
        .map(String::as_str)
        .unwrap_or("10.0.0.2/32")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let allowed_ips = node
        .settings
        .get("allowedIps")
        .map(String::as_str)
        .unwrap_or("0.0.0.0/0")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if address.is_empty() {
        return Err("wireguard address is required".to_string());
    }
    if allowed_ips.is_empty() {
        return Err("wireguard allowed IPs are required".to_string());
    }

    let peer = json!({
        "server": host,
        "server_port": port,
        "public_key": public_key,
        "allowed_ips": allowed_ips,
    });
    Ok(json!({
        "type": "wireguard",
        "tag": tag,
        "private_key": private_key,
        "local_address": address,
        "peers": [peer],
        "workers": 1,
        "mtu": 1408
    }))
}

fn amnezia_outbound(node: &NormalizedNode, tag: &str) -> Result<Value, String> {
    let key = node
        .settings
        .get("amneziaKey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "amnezia key is required".to_string())?;
    let config = parse_amnezia_runtime_config(key)?;
    if config.addresses.is_empty() {
        return Err("amnezia key does not contain interface address".to_string());
    }
    if config.allowed_ips.is_empty() {
        return Err("amnezia key does not contain allowed IPs".to_string());
    }
    let mut peer = json!({
        "server": config.host,
        "server_port": config.port,
        "public_key": config.server_public_key,
        "allowed_ips": config.allowed_ips,
    });
    if let Some(psk) = config.pre_shared_key.filter(|value| !value.is_empty()) {
        if let Some(peer_map) = peer.as_object_mut() {
            peer_map.insert("pre_shared_key".to_string(), json!(psk));
        }
    }
    let mut outbound = json!({
        "type": "wireguard",
        "tag": tag,
        "private_key": config.client_private_key,
        "local_address": config.addresses,
        "peers": [peer],
        "workers": 1,
        "mtu": config.mtu.unwrap_or(1408),
    });
    if let Some(network) = config.transport.filter(|value| !value.is_empty()) {
        if let Some(map) = outbound.as_object_mut() {
            map.insert("network".to_string(), json!(network));
        }
    }
    Ok(outbound)
}

fn tor_outbound(app_handle: &AppHandle, node: &NormalizedNode, tag: &str) -> Result<Value, String> {
    let mut torrc = BTreeMap::<String, String>::new();
    torrc.insert("ClientOnly".to_string(), "1".to_string());
    match node.protocol.as_str() {
        "none" => {}
        "obfs4" | "snowflake" | "meek" => {
            let bridges = node
                .bridges
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("tor {0} requires bridge line", node.protocol))?;
            let first_bridge = bridges
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .ok_or_else(|| format!("tor {0} bridge line is empty", node.protocol))?;
            let first_bridge = first_bridge
                .strip_prefix("Bridge ")
                .unwrap_or(first_bridge)
                .trim()
                .to_string();
            if first_bridge.is_empty() {
                return Err(format!("tor {} bridge line is invalid", node.protocol));
            }
            torrc.insert("UseBridges".to_string(), "1".to_string());
            torrc.insert("Bridge".to_string(), first_bridge);
            let plugin_binary =
                resolve_tor_pt_binary_path(app_handle, &node.protocol).ok_or_else(|| {
                    format!(
                        "tor {} requires pluggable transport binary, but none is available",
                        node.protocol
                    )
                })?;
            let transport = match node.protocol.as_str() {
                "obfs4" => "obfs4",
                "snowflake" => "snowflake",
                "meek" => "meek_lite",
                _ => "",
            };
            torrc.insert(
                "ClientTransportPlugin".to_string(),
                format!("{transport} exec {}", plugin_binary.to_string_lossy()),
            );
        }
        _ => return Err("unsupported tor transport for runtime".to_string()),
    }

    let mut out = json!({
        "type": "tor",
        "tag": tag,
        "torrc": torrc,
    });
    if let Some(binary) = resolve_tor_binary_path(app_handle) {
        if let Some(map) = out.as_object_mut() {
            map.insert(
                "executable_path".to_string(),
                json!(binary.to_string_lossy().to_string()),
            );
        }
    }
    Ok(out)
}

fn required_runtime_tools(
    nodes: &[NormalizedNode],
    uses_openvpn: bool,
    uses_amnezia_native: bool,
) -> BTreeSet<NetworkTool> {
    let mut tools = BTreeSet::new();
    if uses_openvpn {
        tools.insert(NetworkTool::OpenVpn);
    } else if uses_amnezia_native {
        tools.insert(NetworkTool::AmneziaWg);
    } else {
        tools.insert(NetworkTool::SingBox);
    }
    if nodes.iter().any(|node| node.connection_type == "tor") {
        tools.insert(NetworkTool::TorBundle);
    }
    tools
}

fn terminate_pid(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct NormalizedNode {
    connection_type: String,
    protocol: String,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    bridges: Option<String>,
    settings: BTreeMap<String, String>,
}

fn normalized_nodes(template: &crate::state::ConnectionTemplate) -> Vec<NormalizedNode> {
    if !template.nodes.is_empty() {
        return template
            .nodes
            .iter()
            .map(|node| NormalizedNode {
                connection_type: normalize_connection_type(&node.connection_type),
                protocol: normalize_protocol(&node.protocol),
                host: trim_option(node.host.clone()),
                port: node.port,
                username: trim_option(node.username.clone()),
                password: trim_option(node.password.clone()),
                bridges: trim_option(node.bridges.clone()),
                settings: normalize_settings(node.settings.clone()),
            })
            .collect::<Vec<_>>();
    }
    let connection_type = normalize_connection_type(&template.connection_type);
    let protocol = normalize_protocol(&template.protocol);
    if connection_type.is_empty() || protocol.is_empty() {
        return Vec::new();
    }
    vec![NormalizedNode {
        connection_type,
        protocol,
        host: trim_option(template.host.clone()),
        port: template.port,
        username: trim_option(template.username.clone()),
        password: trim_option(template.password.clone()),
        bridges: trim_option(template.bridges.clone()),
        settings: BTreeMap::new(),
    }]
}

fn trim_option(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn normalize_connection_type(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "vpn" => "vpn".to_string(),
        "v2ray" | "xray" => "v2ray".to_string(),
        "proxy" => "proxy".to_string(),
        "tor" => "tor".to_string(),
        _ => value.trim().to_lowercase(),
    }
}

fn normalize_protocol(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "ss" => "shadowsocks".to_string(),
        protocol => protocol.to_string(),
    }
}

fn normalize_settings(raw: BTreeMap<String, String>) -> BTreeMap<String, String> {
    raw.into_iter()
        .filter_map(|(key, value)| {
            let key = key.trim().to_string();
            if key.is_empty() {
                return None;
            }
            Some((key, value.trim().to_string()))
        })
        .collect()
}

#[derive(Debug, Clone)]
struct AmneziaRuntimeConfig {
    host: String,
    port: u16,
    client_private_key: String,
    server_public_key: String,
    pre_shared_key: Option<String>,
    addresses: Vec<String>,
    allowed_ips: Vec<String>,
    mtu: Option<u16>,
    transport: Option<String>,
}

fn parse_amnezia_runtime_config(value: &str) -> Result<AmneziaRuntimeConfig, String> {
    if looks_like_amnezia_conf(value) {
        return parse_amnezia_conf_runtime_config(value);
    }
    let json = decode_amnezia_json(value)?;
    let awg = extract_awg_payload(&json)
        .ok_or_else(|| "amnezia key does not contain awg payload".to_string())?;
    let mut config = awg.clone();
    if let Some(last) = awg.get("last_config") {
        match last {
            Value::String(raw) => {
                if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
                    config = parsed;
                }
            }
            Value::Object(_) => {
                config = last.clone();
            }
            _ => {}
        }
    }

    let (host, port) = extract_host_port_from_config(&config, &json, &awg)?;
    let client_private_key = extract_string(&config, &["client_priv_key", "private_key"])
        .or_else(|| extract_ini_value(&config, "Interface", "PrivateKey"))
        .ok_or_else(|| "amnezia key does not contain private key".to_string())?;
    let server_public_key = extract_string(
        &config,
        &["server_pub_key", "peer_public_key", "public_key"],
    )
    .or_else(|| extract_ini_value(&config, "Peer", "PublicKey"))
    .ok_or_else(|| "amnezia key does not contain server public key".to_string())?;
    let pre_shared_key = extract_string(&config, &["psk_key", "pre_shared_key"])
        .or_else(|| extract_ini_value(&config, "Peer", "PresharedKey"));

    let allowed_ips = extract_string_array(&config, "allowed_ips").or_else(|| {
        extract_ini_value(&config, "Peer", "AllowedIPs").map(|value| split_csv_values(&value))
    });
    let allowed_ips = allowed_ips
        .unwrap_or_else(|| vec!["0.0.0.0/0".to_string(), "::/0".to_string()])
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let addresses = extract_string_array(&config, "client_ip")
        .or_else(|| extract_string_array(&config, "address"))
        .or_else(|| {
            extract_ini_value(&config, "Interface", "Address").map(|value| split_csv_values(&value))
        })
        .unwrap_or_default()
        .into_iter()
        .map(|value| normalize_interface_address(&value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let mtu = extract_u16(&config, &["mtu"]);
    let transport = extract_string(&awg, &["transport_proto", "transport"]);

    Ok(AmneziaRuntimeConfig {
        host,
        port,
        client_private_key,
        server_public_key,
        pre_shared_key,
        addresses,
        allowed_ips,
        mtu,
        transport,
    })
}

fn looks_like_amnezia_conf(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("[interface]") && lower.contains("[peer]")
}

fn parse_amnezia_conf_runtime_config(value: &str) -> Result<AmneziaRuntimeConfig, String> {
    let sections = parse_ini_sections(value);
    let interface = sections
        .get("interface")
        .ok_or_else(|| "amnezia config does not contain [Interface] section".to_string())?;
    let peer = sections
        .get("peer")
        .ok_or_else(|| "amnezia config does not contain [Peer] section".to_string())?;

    let endpoint = peer
        .get("endpoint")
        .map(String::as_str)
        .and_then(parse_host_port_pair)
        .ok_or_else(|| "amnezia config does not contain endpoint".to_string())?;
    let client_private_key = interface
        .get("privatekey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "amnezia config does not contain private key".to_string())?
        .to_string();
    let server_public_key = peer
        .get("publickey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "amnezia config does not contain server public key".to_string())?
        .to_string();
    let pre_shared_key = peer
        .get("presharedkey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let addresses = interface
        .get("address")
        .map(String::as_str)
        .map(split_csv_values)
        .unwrap_or_default()
        .into_iter()
        .map(|value| normalize_interface_address(&value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let allowed_ips = peer
        .get("allowedips")
        .map(String::as_str)
        .map(split_csv_values)
        .unwrap_or_else(|| vec!["0.0.0.0/0".to_string(), "::/0".to_string()])
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let mtu = interface
        .get("mtu")
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|value| *value > 0);
    let transport = interface
        .get("protocol")
        .or_else(|| interface.get("transport"))
        .or_else(|| interface.get("transport_proto"))
        .map(String::as_str)
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .filter(|value| value == "udp" || value == "tcp");

    Ok(AmneziaRuntimeConfig {
        host: endpoint.0,
        port: endpoint.1,
        client_private_key,
        server_public_key,
        pre_shared_key,
        addresses,
        allowed_ips,
        mtu,
        transport,
    })
}

fn parse_ini_sections(value: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut sections = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut current_section = String::new();
    for raw_line in value.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        if current_section.is_empty() {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        sections
            .entry(current_section.clone())
            .or_default()
            .insert(key.trim().to_ascii_lowercase(), raw_value.trim().to_string());
    }
    sections
}

fn decode_amnezia_json(value: &str) -> Result<Value, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("amnezia key is required".to_string());
    }
    let encoded = match trimmed.get(0..6) {
        Some(prefix) if prefix.eq_ignore_ascii_case("vpn://") => {
            trimmed.get(6..).unwrap_or_default().trim()
        }
        _ => trimmed,
    };
    if encoded.is_empty() {
        return Err("amnezia key payload is empty".to_string());
    }

    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| URL_SAFE.decode(encoded))
        .map_err(|_| "amnezia key payload encoding is invalid".to_string())?;
    let inflated = if decoded.len() > 4 {
        inflate_zlib_to_string(&decoded[4..]).or_else(|_| inflate_zlib_to_string(&decoded))
    } else {
        inflate_zlib_to_string(&decoded)
    }
    .map_err(|_| "amnezia key payload compression is invalid".to_string())?;

    serde_json::from_str::<Value>(&inflated).map_err(|_| "amnezia key JSON is invalid".to_string())
}

fn inflate_zlib_to_string(bytes: &[u8]) -> Result<String, String> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut output = String::new();
    decoder
        .read_to_string(&mut output)
        .map_err(|_| "failed to inflate".to_string())?;
    if output.trim().is_empty() {
        return Err("inflated payload is empty".to_string());
    }
    Ok(output)
}

fn extract_awg_payload(value: &Value) -> Option<Value> {
    let containers = value.get("containers")?.as_array()?;
    for container in containers {
        if let Some(awg) = container.get("awg") {
            return Some(awg.clone());
        }
    }
    None
}

fn extract_host_port_from_config(
    config: &Value,
    root: &Value,
    awg: &Value,
) -> Result<(String, u16), String> {
    if let Some(endpoint) = extract_string(config, &["endpoint", "server", "address"]) {
        if let Some(parsed) = parse_host_port_pair(&endpoint) {
            return Ok(parsed);
        }
    }
    let host = extract_string(config, &["hostName", "hostname", "host"])
        .or_else(|| extract_string(root, &["hostName", "hostname", "host"]))
        .or_else(|| extract_string(awg, &["hostName", "hostname", "host"]))
        .ok_or_else(|| "amnezia key does not contain endpoint host".to_string())?;
    let port = extract_u16(config, &["port", "endpoint_port"])
        .or_else(|| extract_u16(root, &["port", "endpoint_port"]))
        .or_else(|| extract_u16(awg, &["port", "endpoint_port"]))
        .ok_or_else(|| "amnezia key does not contain endpoint port".to_string())?;
    Ok((host, port))
}

fn extract_string(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(raw) = map.get(*key).and_then(Value::as_str) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn extract_u16(value: &Value, keys: &[&str]) -> Option<u16> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(raw) = map.get(*key) {
            match raw {
                Value::Number(number) => {
                    if let Some(port) = number.as_u64().and_then(|item| u16::try_from(item).ok()) {
                        if port > 0 {
                            return Some(port);
                        }
                    }
                }
                Value::String(text) => {
                    if let Ok(port) = text.trim().parse::<u16>() {
                        if port > 0 {
                            return Some(port);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn extract_string_array(value: &Value, key: &str) -> Option<Vec<String>> {
    let map = value.as_object()?;
    let raw = map.get(key)?;
    match raw {
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>(),
        ),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(vec![trimmed.to_string()])
            }
        }
        _ => None,
    }
}

fn extract_ini_value(config: &Value, section: &str, key: &str) -> Option<String> {
    let raw = extract_string(config, &["config"])?;
    let section_header = format!("[{section}]").to_ascii_lowercase();
    let key_lower = key.to_ascii_lowercase();
    let mut in_section = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed.to_ascii_lowercase() == section_header;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((left, right)) = trimmed.split_once('=') else {
            continue;
        };
        if left.trim().eq_ignore_ascii_case(&key_lower) || left.trim().eq_ignore_ascii_case(key) {
            let value = right.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn split_csv_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>()
}

fn normalize_interface_address(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.contains('/') {
        return trimmed.to_string();
    }
    if trimmed.contains(':') {
        format!("{trimmed}/128")
    } else {
        format!("{trimmed}/32")
    }
}

fn parse_host_port_pair(raw: &str) -> Option<(String, u16)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('[') {
        let end = trimmed.find(']')?;
        let host = trimmed[1..end].trim();
        let rest = trimmed[end + 1..].trim();
        let port = rest.strip_prefix(':')?.trim().parse::<u16>().ok()?;
        if !host.is_empty() && port > 0 {
            return Some((host.to_string(), port));
        }
    }

    let (host, port_raw) = trimmed.rsplit_once(':')?;
    let host = host.trim();
    let port = port_raw.trim().parse::<u16>().ok()?;
    if host.is_empty() || port == 0 {
        return None;
    }
    Some((host.to_string(), port))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{NetworkGlobalRouteSettings, NetworkStore};
    use browser_network_policy::VpnProxyTabPayload;
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    fn build_amnezia_key(payload: &str) -> String {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(payload.as_bytes())
            .expect("write amnezia payload");
        let compressed = encoder.finish().expect("finish amnezia compression");
        let mut framed = Vec::with_capacity(compressed.len() + 4);
        let len = payload.len() as u32;
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(&compressed);
        format!("vpn://{}", URL_SAFE_NO_PAD.encode(framed))
    }

    #[test]
    fn parse_amnezia_runtime_config_extracts_wireguard_settings() {
        let payload = r#"{
          "hostName":"91.186.212.196",
          "containers":[
            {
              "awg":{
                "transport_proto":"udp",
                "last_config":"{\"client_priv_key\":\"PRIVATE\",\"server_pub_key\":\"PUBLIC\",\"psk_key\":\"PSK\",\"client_ip\":\"10.8.1.6\",\"allowed_ips\":[\"0.0.0.0/0\",\"::/0\"],\"persistent_keep_alive\":\"25\",\"mtu\":\"1376\",\"port\":\"44017\",\"hostName\":\"91.186.212.196\"}"
              }
            }
          ]
        }"#;
        let key = build_amnezia_key(payload);
        let cfg = parse_amnezia_runtime_config(&key).expect("parse amnezia runtime config");
        assert_eq!(cfg.host, "91.186.212.196");
        assert_eq!(cfg.port, 44017);
        assert_eq!(cfg.client_private_key, "PRIVATE");
        assert_eq!(cfg.server_public_key, "PUBLIC");
        assert_eq!(cfg.pre_shared_key.as_deref(), Some("PSK"));
        assert_eq!(cfg.addresses, vec!["10.8.1.6/32".to_string()]);
        assert_eq!(
            cfg.allowed_ips,
            vec!["0.0.0.0/0".to_string(), "::/0".to_string()]
        );
        assert_eq!(cfg.mtu, Some(1376));
        assert_eq!(cfg.transport.as_deref(), Some("udp"));
    }

    #[test]
    fn parse_amnezia_runtime_config_supports_awg_conf() {
        let conf = r#"
[Interface]
Address = 10.8.1.84/32
DNS = 1.1.1.1, 1.0.0.1
PrivateKey = PRIVATE
Jc = 4
Jmin = 10
Jmax = 50

[Peer]
PublicKey = PUBLIC
PresharedKey = PSK
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = 5.129.225.48:32542
PersistentKeepalive = 25
"#;
        let cfg = parse_amnezia_runtime_config(conf).expect("parse amnezia conf runtime config");
        assert_eq!(cfg.host, "5.129.225.48");
        assert_eq!(cfg.port, 32542);
        assert_eq!(cfg.client_private_key, "PRIVATE");
        assert_eq!(cfg.server_public_key, "PUBLIC");
        assert_eq!(cfg.pre_shared_key.as_deref(), Some("PSK"));
        assert_eq!(cfg.addresses, vec!["10.8.1.84/32".to_string()]);
        assert_eq!(
            cfg.allowed_ips,
            vec!["0.0.0.0/0".to_string(), "::/0".to_string()]
        );
    }

    #[test]
    fn build_amnezia_native_config_text_from_key_replaces_dns_placeholders() {
        let last_cfg = serde_json::json!({
            "config": "[Interface]\nAddress = 10.8.1.84/32\nDNS = $PRIMARY_DNS, $SECONDARY_DNS\nPrivateKey = PRIVATE\nJc = 4\nJmin = 10\nJmax = 50\nS1 = 88\nS2 = 143\nH1 = 755270012\nH2 = 876050617\nH3 = 220715218\nH4 = 1577770230\nI1 = \nI2 = \nI3 = \nI4 = \nI5 = \n\n[Peer]\nPublicKey = PUBLIC\nPresharedKey = PSK\nAllowedIPs = 0.0.0.0/0, ::/0\nEndpoint = 5.129.225.48:32542\nPersistentKeepalive = 25\n"
        })
        .to_string();
        let payload = serde_json::json!({
            "dns1": "1.1.1.1",
            "dns2": "1.0.0.1",
            "containers": [
                {
                    "awg": {
                        "last_config": last_cfg
                    }
                }
            ]
        })
        .to_string();
        let key = build_amnezia_key(&payload);
        let conf = build_amnezia_native_config_text(&key).expect("materialize amnezia config");
        assert!(conf.contains("DNS = 1.1.1.1, 1.0.0.1"));
        assert!(conf.contains("Jc = 4"));
        assert!(conf.contains("Endpoint = 5.129.225.48:32542"));
    }

    #[test]
    fn amnezia_tunnel_name_is_stable_and_within_limit() {
        let profile_id = Uuid::parse_str("7aafb8b9-17f0-4c2b-8dac-b92e77629d44").expect("uuid");
        let name = amnezia_tunnel_name(profile_id);
        assert!(name.starts_with("awg-"));
        assert!(name.len() <= 32);
    }

    #[test]
    fn resolve_effective_route_selection_prioritizes_direct_profile_over_global_vpn() {
        let profile_key = "profile-direct-priority".to_string();
        let mut store = NetworkStore::default();
        store.vpn_proxy.insert(
            profile_key.clone(),
            VpnProxyTabPayload {
                route_mode: "direct".to_string(),
                proxy: None,
                vpn: None,
                kill_switch_enabled: false,
            },
        );
        store.profile_template_selection.insert(
            profile_key.clone(),
            "profile-template".to_string(),
        );
        store.global_route_settings = NetworkGlobalRouteSettings {
            global_vpn_enabled: true,
            block_without_vpn: true,
            default_template_id: Some("global-template".to_string()),
        };

        let (mode, template) = resolve_effective_route_selection(&store, &profile_key);
        assert_eq!(mode, "direct");
        assert_eq!(template, None);
    }

    #[test]
    fn resolve_effective_route_selection_uses_global_defaults_for_non_direct_profiles() {
        let profile_key = "profile-global-default".to_string();
        let mut store = NetworkStore::default();
        store.vpn_proxy.insert(
            profile_key.clone(),
            VpnProxyTabPayload {
                route_mode: "vpn".to_string(),
                proxy: None,
                vpn: None,
                kill_switch_enabled: true,
            },
        );
        store.profile_template_selection.insert(
            profile_key.clone(),
            "profile-template".to_string(),
        );
        store.global_route_settings = NetworkGlobalRouteSettings {
            global_vpn_enabled: true,
            block_without_vpn: true,
            default_template_id: Some("global-template".to_string()),
        };

        let (mode, template) = resolve_effective_route_selection(&store, &profile_key);
        assert_eq!(mode, "vpn");
        assert_eq!(template.as_deref(), Some("global-template"));
    }
}
