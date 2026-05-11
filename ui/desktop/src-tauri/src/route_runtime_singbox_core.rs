use super::*;

pub(crate) fn run_sing_box_check_impl(
    binary: &str,
    config_path: &PathBuf,
    log_path: &PathBuf,
) -> Result<(), String> {
    let output = hidden_command(binary)
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

pub(crate) fn build_runtime_config_impl(
    app_handle: &AppHandle,
    nodes: &[NormalizedNode],
    listen_port: u16,
    log_path: &PathBuf,
    target: RuntimeExecutionTarget,
) -> Result<Value, String> {
    let mut outbounds = Vec::new();
    let mut endpoints = Vec::new();
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
        match node_to_sing_box_runtime_entry_impl(app_handle, node, &tags[idx], detour, target)? {
            SingBoxRuntimeEntry::Outbound(outbound) => outbounds.push(outbound),
            SingBoxRuntimeEntry::Endpoint(endpoint) => endpoints.push(endpoint),
        }
    }
    outbounds.push(json!({ "type": "direct", "tag": "direct" }));

    Ok(json!({
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
                "listen": if target == RuntimeExecutionTarget::Container { "0.0.0.0" } else { "127.0.0.1" },
                "listen_port": listen_port
            }
        ],
        "endpoints": endpoints,
        "outbounds": outbounds,
        "route": {
            "final": tags.first().cloned().unwrap_or_else(|| "direct".to_string())
        }
    }))
}

pub(crate) fn node_supported_by_runtime_impl(node: &NormalizedNode) -> bool {
    match node.connection_type.as_str() {
        "proxy" => matches!(node.protocol.as_str(), "http" | "socks4" | "socks5"),
        "v2ray" => matches!(
            node.protocol.as_str(),
            "vmess" | "vless" | "trojan" | "shadowsocks"
        ),
        "vpn" => matches!(node.protocol.as_str(), "wireguard" | "amnezia" | "openvpn"),
        "tor" => matches!(node.protocol.as_str(), "obfs4" | "snowflake" | "meek" | "none"),
        _ => false,
    }
}

pub(crate) fn required_runtime_tools_impl(
    nodes: &[NormalizedNode],
    uses_openvpn: bool,
    uses_amnezia_native: bool,
    uses_amnezia_container: bool,
    uses_container_runtime: bool,
) -> BTreeSet<NetworkTool> {
    let mut tools = BTreeSet::new();
    if uses_container_runtime {
    } else if uses_openvpn {
        tools.insert(NetworkTool::OpenVpn);
    } else if uses_amnezia_native {
        tools.insert(NetworkTool::AmneziaWg);
    } else if uses_amnezia_container {
    } else {
        tools.insert(NetworkTool::SingBox);
    }
    if nodes.iter().any(|node| node.connection_type == "tor") {
        tools.insert(NetworkTool::TorBundle);
    }
    tools
}

#[allow(dead_code)]
pub(crate) fn proxy_outbound_bridge_impl(
    node: &NormalizedNode,
    tag: &str,
) -> Result<Value, String> {
    transport::transport_proxy::proxy_outbound_impl(node, tag)
}

#[allow(dead_code)]
pub(crate) fn v2ray_outbound_bridge_impl(
    node: &NormalizedNode,
    tag: &str,
) -> Result<Value, String> {
    transport::transport_v2ray::v2ray_outbound_impl(node, tag)
}

#[allow(dead_code)]
pub(crate) fn apply_v2ray_transport_and_tls_bridge_impl(
    outbound: &mut Value,
    node: &NormalizedNode,
) -> Result<(), String> {
    transport::transport_v2ray::apply_v2ray_transport_and_tls_impl(outbound, node)
}

#[allow(dead_code)]
pub(crate) fn wireguard_endpoint_bridge_impl(
    node: &NormalizedNode,
    tag: &str,
) -> Result<Value, String> {
    transport::transport_vpn::wireguard_endpoint_impl(node, tag)
}

#[allow(dead_code)]
pub(crate) fn amnezia_endpoint_bridge_impl(
    node: &NormalizedNode,
    tag: &str,
) -> Result<Value, String> {
    transport::transport_vpn::amnezia_endpoint_impl(node, tag)
}

#[allow(dead_code)]
pub(crate) fn tor_outbound_bridge_impl(
    app_handle: &AppHandle,
    node: &NormalizedNode,
    tag: &str,
    target: RuntimeExecutionTarget,
) -> Result<Value, String> {
    transport::transport_tor::tor_outbound_impl(app_handle, node, tag, target)
}


#[path = "route_runtime_singbox_core_transport.rs"]
mod transport;
pub(crate) use transport::*;


