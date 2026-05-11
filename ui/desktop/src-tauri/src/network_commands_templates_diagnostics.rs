use super::*;

#[path = "network_commands_templates_parser.rs"]
mod parser;

pub(crate) fn test_connection_template_impl_impl(
    template: &ConnectionTemplate,
) -> Result<ConnectionHealthView, String> {
    let nodes = normalize_template_nodes(template);
    if nodes.is_empty() {
        return Err("connection template does not contain nodes".to_string());
    }
    let mut reachable = true;
    let mut latency_sum = 0u128;
    let mut latency_count = 0u128;
    let mut messages = Vec::new();
    for node in &nodes {
        let health = test_connection_node(node)?;
        if let Some(latency) = health.latency_ms {
            latency_sum += latency;
            latency_count += 1;
        }
        reachable = reachable && health.reachable;
        messages.push(format!(
            "[{}:{}] {}",
            node.connection_type, node.protocol, health.message
        ));
    }
    Ok(ConnectionHealthView {
        reachable,
        status: if reachable { "ok" } else { "unavailable" }.to_string(),
        latency_ms: if latency_count > 0 { Some(latency_sum / latency_count) } else { None },
        message: messages.join(" | "),
    })
}

fn normalize_template_nodes(template: &ConnectionTemplate) -> Vec<ConnectionNode> {
    if !template.nodes.is_empty() {
        return template
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| ConnectionNode {
                id: if node.id.trim().is_empty() { format!("node-{}", index + 1) } else { node.id.clone() },
                connection_type: normalize_connection_type(&node.connection_type),
                protocol: normalize_protocol(&node.protocol),
                host: trim_option(node.host.clone()),
                port: node.port,
                username: trim_option(node.username.clone()),
                password: trim_option(node.password.clone()),
                bridges: trim_option(node.bridges.clone()),
                settings: normalize_settings(node.settings.clone()),
            })
            .collect();
    }
    let connection_type = normalize_connection_type(&template.connection_type);
    let protocol = normalize_protocol(&template.protocol);
    if connection_type.is_empty() || protocol.is_empty() {
        return Vec::new();
    }
    vec![ConnectionNode {
        id: "node-1".to_string(),
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
        if trimmed.is_empty() { None } else { Some(trimmed) }
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

fn test_connection_node(node: &ConnectionNode) -> Result<ConnectionHealthView, String> {
    match node.connection_type.as_str() {
        "tor" => {
            if node.protocol != "obfs4" {
                return Ok(ConnectionHealthView {
                    reachable: true,
                    status: "ok".to_string(),
                    latency_ms: None,
                    message: "TOR transport does not require static bridge endpoint".to_string(),
                });
            }
            let Some(first_bridge) = node.bridges.as_deref().and_then(parse_first_bridge_endpoint) else {
                return Ok(ConnectionHealthView {
                    reachable: false,
                    status: "unavailable".to_string(),
                    latency_ms: None,
                    message: "TOR obfs4 bridge endpoint not found".to_string(),
                });
            };
            test_tcp_endpoint(first_bridge.0.as_str(), first_bridge.1, "TOR obfs4 bridge".to_string())
        }
        "vpn" => {
            if node.protocol == "amnezia" {
                let amnezia_key = node.settings.get("amneziaKey").map(String::as_str).unwrap_or_default();
                let (host, port, transport) = parser::parse_amnezia_key_details_impl(amnezia_key)?;
                if matches!(transport.as_deref(), Some("tcp")) {
                    return test_tcp_endpoint(host.as_str(), port, "AMNEZIA TCP endpoint".to_string());
                }
                return test_udp_endpoint(host.as_str(), port, "AMNEZIA UDP endpoint".to_string());
            }
            if node.protocol == "openvpn" {
                let host = node.host.clone().unwrap_or_default();
                let port = node.port.unwrap_or_default();
                let transport = node
                    .settings
                    .get("transport")
                    .map(String::as_str)
                    .map(str::trim)
                    .unwrap_or("udp");
                if transport.eq_ignore_ascii_case("udp") {
                    return test_udp_endpoint(host.as_str(), port, "OPENVPN UDP endpoint".to_string());
                }
                return test_tcp_endpoint(host.as_str(), port, "OPENVPN TCP endpoint".to_string());
            }
            let host = node.host.clone().unwrap_or_default();
            let port = node.port.unwrap_or_default();
            test_tcp_endpoint(host.as_str(), port, format!("{} endpoint", node.protocol.to_uppercase()))
        }
        "proxy" | "v2ray" => {
            let host = node.host.clone().unwrap_or_default();
            let port = node.port.unwrap_or_default();
            test_tcp_endpoint(host.as_str(), port, format!("{} endpoint", node.protocol.to_uppercase()))
        }
        _ => Err("unsupported connection node type".to_string()),
    }
}

fn test_udp_endpoint(host: &str, port: u16, label: String) -> Result<ConnectionHealthView, String> {
    if host.trim().is_empty() || port == 0 {
        return Err("host and port are required for connectivity check".to_string());
    }
    let started = std::time::Instant::now();
    let mut addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve endpoint failed: {e}"))?;
    let Some(addr) = addrs.next() else {
        return Err("no endpoint address resolved".to_string());
    };
    let bind_addr = if addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
    let socket = UdpSocket::bind(bind_addr).map_err(|e| format!("udp bind failed: {e}"))?;
    socket
        .set_write_timeout(Some(Duration::from_millis(3_000)))
        .map_err(|e| format!("udp timeout setup failed: {e}"))?;
    socket.connect(addr).map_err(|e| format!("udp connect failed: {e}"))?;
    let elapsed_ms = started.elapsed().as_millis().max(1);
    match socket.send(&[0u8]) {
        Ok(_) => Ok(ConnectionHealthView {
            reachable: true,
            status: "ok".to_string(),
            latency_ms: Some(elapsed_ms),
            message: format!("{label} probe sent"),
        }),
        Err(error) => Ok(ConnectionHealthView {
            reachable: false,
            status: "unavailable".to_string(),
            latency_ms: Some(elapsed_ms),
            message: format!("{label} probe failed: {error}"),
        }),
    }
}

fn test_tcp_endpoint(host: &str, port: u16, label: String) -> Result<ConnectionHealthView, String> {
    if host.trim().is_empty() || port == 0 {
        return Err("host and port are required for connectivity check".to_string());
    }
    let started = std::time::Instant::now();
    let mut addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve endpoint failed: {e}"))?;
    let Some(addr) = addrs.next() else {
        return Err("no endpoint address resolved".to_string());
    };
    let timeout = Duration::from_millis(3_000);
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(_) => Ok(ConnectionHealthView {
            reachable: true,
            status: "ok".to_string(),
            latency_ms: Some(started.elapsed().as_millis().max(1)),
            message: format!("{label} is reachable"),
        }),
        Err(error) => Ok(ConnectionHealthView {
            reachable: false,
            status: "unavailable".to_string(),
            latency_ms: Some(started.elapsed().as_millis().max(1)),
            message: format!("{label} is not reachable: {error}"),
        }),
    }
}

fn parse_first_bridge_endpoint(bridges: &str) -> Option<(String, u16)> {
    for line in bridges.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        if let Some((host, port)) = parts[1].rsplit_once(':') {
            if let Ok(port) = port.parse::<u16>() {
                return Some((host.to_string(), port));
            }
        }
    }
    None
}
