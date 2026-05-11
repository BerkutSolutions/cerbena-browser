use super::*;

#[path = "network_commands_templates_parser.rs"]
mod parser;

pub(crate) fn validate_connection_template_request_impl(
    request: &SaveConnectionTemplateRequest,
) -> Result<(), String> {
    if request.name.trim().is_empty() {
        return Err("connection template name is required".to_string());
    }
    let mut template = ConnectionTemplate {
        id: "validation-template".to_string(),
        name: request.name.trim().to_string(),
        nodes: build_nodes_from_request_impl(request)?,
        connection_type: String::new(),
        protocol: String::new(),
        host: None,
        port: None,
        username: None,
        password: None,
        bridges: None,
        updated_at_epoch_ms: 0,
    };
    sync_legacy_primary_fields_impl(&mut template);
    validate_connection_template_impl(&template)
}

pub(crate) fn validate_connection_template_impl(template: &ConnectionTemplate) -> Result<(), String> {
    let nodes = normalize_template_nodes(template);
    if nodes.is_empty() {
        return Err("at least one connection node is required".to_string());
    }
    if nodes.len() > 3 {
        return Err("maximum three connection nodes are supported".to_string());
    }
    for node in &nodes {
        validate_connection_node(node)?;
    }
    Ok(())
}

pub(crate) fn build_template_id_impl(
    seed: &str,
    existing: &std::collections::BTreeMap<String, ConnectionTemplate>,
) -> String {
    let mut base = seed
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if base.is_empty() {
        base = "connection-template".to_string();
    }
    let mut candidate = base.clone();
    let mut index = 2u32;
    while existing.contains_key(&candidate) {
        candidate = format!("{base}-{index}");
        index += 1;
    }
    candidate
}

pub(crate) fn build_nodes_from_request_impl(
    request: &SaveConnectionTemplateRequest,
) -> Result<Vec<ConnectionNode>, String> {
    let mut nodes = if !request.nodes.is_empty() {
        request
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let node_id = node
                    .node_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("node-{}", index + 1));
                Ok(ConnectionNode {
                    id: node_id,
                    connection_type: normalize_connection_type(&node.connection_type),
                    protocol: normalize_protocol(node.protocol.trim()),
                    host: trim_option(node.host.clone()),
                    port: node.port,
                    username: trim_option(node.username.clone()),
                    password: trim_option(node.password.clone()),
                    bridges: trim_option(node.bridges.clone()),
                    settings: normalize_settings(node.settings.clone()),
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        vec![ConnectionNode {
            id: "node-1".to_string(),
            connection_type: normalize_connection_type(&request.connection_type),
            protocol: normalize_protocol(request.protocol.trim()),
            host: trim_option(request.host.clone()),
            port: request.port,
            username: trim_option(request.username.clone()),
            password: trim_option(request.password.clone()),
            bridges: trim_option(request.bridges.clone()),
            settings: BTreeMap::new(),
        }]
    };
    if nodes.is_empty() {
        return Err("at least one connection node is required".to_string());
    }
    if nodes.len() > 3 {
        return Err("maximum three connection nodes are supported".to_string());
    }
    for (index, node) in nodes.iter_mut().enumerate() {
        if node.id.trim().is_empty() {
            node.id = format!("node-{}", index + 1);
        }
        hydrate_amnezia_endpoint(node)?;
    }
    Ok(nodes)
}

pub(crate) fn sync_legacy_primary_fields_impl(template: &mut ConnectionTemplate) {
    let nodes = normalize_template_nodes(template);
    if let Some(primary) = nodes.first() {
        template.connection_type = primary.connection_type.clone();
        template.protocol = primary.protocol.clone();
        template.host = primary.host.clone();
        template.port = primary.port;
        template.username = primary.username.clone();
        template.password = primary.password.clone();
        template.bridges = primary.bridges.clone();
        template.nodes = nodes;
    } else {
        template.connection_type.clear();
        template.protocol.clear();
        template.host = None;
        template.port = None;
        template.username = None;
        template.password = None;
        template.bridges = None;
        template.nodes.clear();
    }
}

pub(crate) fn now_epoch_ms_impl() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn validate_connection_node(node: &ConnectionNode) -> Result<(), String> {
    match node.connection_type.as_str() {
        "vpn" => {
            let valid = ["wireguard", "openvpn", "amnezia"];
            if !valid.contains(&node.protocol.as_str()) {
                return Err("unsupported VPN protocol".to_string());
            }
            if node.protocol == "amnezia" {
                let amnezia_key = node
                    .settings
                    .get("amneziaKey")
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "amnezia key is required".to_string())?;
                let _ = parser::parse_amnezia_key_endpoint_impl(&amnezia_key)?;
            } else {
                validate_host_port(node.host.as_deref(), node.port)?;
            }
        }
        "proxy" => {
            validate_host_port(node.host.as_deref(), node.port)?;
            let valid = ["http", "socks4", "socks5"];
            if !valid.contains(&node.protocol.as_str()) {
                return Err("unsupported proxy protocol".to_string());
            }
        }
        "v2ray" => {
            validate_host_port(node.host.as_deref(), node.port)?;
            let valid = ["vmess", "vless", "trojan", "shadowsocks"];
            if !valid.contains(&node.protocol.as_str()) {
                return Err("unsupported V2Ray/XRay protocol".to_string());
            }
            if matches!(node.protocol.as_str(), "vmess" | "vless") {
                let uuid = node
                    .settings
                    .get("uuid")
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "uuid is required for vmess/vless".to_string())?;
                if !looks_like_uuid(&uuid) {
                    return Err("uuid format is invalid".to_string());
                }
            }
            if node.protocol == "vless" {
                let security_mode = node
                    .settings
                    .get("securityMode")
                    .map(String::as_str)
                    .map(str::trim)
                    .map(str::to_ascii_lowercase)
                    .unwrap_or_default();
                if security_mode == "reality" {
                    let reality_public_key = node
                        .settings
                        .get("realityPublicKey")
                        .map(String::as_str)
                        .map(str::trim)
                        .unwrap_or_default();
                    if reality_public_key.is_empty() {
                        return Err("vless reality requires pbk/public key".to_string());
                    }
                }
            }
            if matches!(node.protocol.as_str(), "trojan" | "shadowsocks")
                && node.password.as_deref().map(str::trim).unwrap_or_default().is_empty()
            {
                return Err("password is required for trojan/shadowsocks".to_string());
            }
        }
        "tor" => {
            let valid = ["obfs4", "snowflake", "meek", "none"];
            if !valid.contains(&node.protocol.as_str()) {
                return Err("unsupported TOR transport".to_string());
            }
            if node.protocol == "obfs4" && trim_option(node.bridges.clone()).is_none() {
                return Err("TOR bridges are required for obfs4".to_string());
            }
        }
        _ => return Err("unsupported connection type".to_string()),
    }
    Ok(())
}

fn validate_host_port(host: Option<&str>, port: Option<u16>) -> Result<(), String> {
    if host.unwrap_or_default().trim().is_empty() {
        return Err("host is required".to_string());
    }
    if port.unwrap_or_default() == 0 {
        return Err("port must be non-zero".to_string());
    }
    Ok(())
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

fn looks_like_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (idx, byte) in bytes.iter().enumerate() {
        if [8, 13, 18, 23].contains(&idx) {
            if *byte != b'-' {
                return false;
            }
            continue;
        }
        if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

fn hydrate_amnezia_endpoint(node: &mut ConnectionNode) -> Result<(), String> {
    if node.connection_type != "vpn" || node.protocol != "amnezia" {
        return Ok(());
    }
    let amnezia_key = node
        .settings
        .get("amneziaKey")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "amnezia key is required".to_string())?;
    let (host, port) = parser::parse_amnezia_key_endpoint_impl(&amnezia_key)?;
    node.host = Some(host);
    node.port = Some(port);
    Ok(())
}
