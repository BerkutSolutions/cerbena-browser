use super::*;

pub(super) fn normalized_nodes_impl(
    template: &crate::state::ConnectionTemplate,
) -> Vec<NormalizedNode> {
    if !template.nodes.is_empty() {
        return template
            .nodes
            .iter()
            .map(|node| NormalizedNode {
                connection_type: normalize_connection_type_impl(&node.connection_type),
                protocol: normalize_protocol_impl(&node.protocol),
                host: trim_option_impl(node.host.clone()),
                port: node.port,
                username: trim_option_impl(node.username.clone()),
                password: trim_option_impl(node.password.clone()),
                bridges: trim_option_impl(node.bridges.clone()),
                settings: normalize_settings_impl(node.settings.clone()),
            })
            .collect::<Vec<_>>();
    }
    let connection_type = normalize_connection_type_impl(&template.connection_type);
    let protocol = normalize_protocol_impl(&template.protocol);
    if connection_type.is_empty() || protocol.is_empty() {
        return Vec::new();
    }
    vec![NormalizedNode {
        connection_type,
        protocol,
        host: trim_option_impl(template.host.clone()),
        port: template.port,
        username: trim_option_impl(template.username.clone()),
        password: trim_option_impl(template.password.clone()),
        bridges: trim_option_impl(template.bridges.clone()),
        settings: BTreeMap::new(),
    }]
}

pub(super) fn trim_option_impl(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub(super) fn normalize_connection_type_impl(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "vpn" => "vpn".to_string(),
        "v2ray" | "xray" => "v2ray".to_string(),
        "proxy" => "proxy".to_string(),
        "tor" => "tor".to_string(),
        _ => value.trim().to_lowercase(),
    }
}

pub(super) fn normalize_protocol_impl(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "ss" => "shadowsocks".to_string(),
        protocol => protocol.to_string(),
    }
}

pub(super) fn normalize_settings_impl(raw: BTreeMap<String, String>) -> BTreeMap<String, String> {
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

pub(super) fn node_supported_by_runtime_impl(node: &NormalizedNode) -> bool {
    singbox::node_supported_by_runtime_impl(node)
}

pub(super) fn required_runtime_tools_impl(
    nodes: &[NormalizedNode],
    uses_openvpn: bool,
    uses_amnezia_native: bool,
    uses_amnezia_container: bool,
    uses_container_runtime: bool,
) -> BTreeSet<NetworkTool> {
    singbox::required_runtime_tools_impl(
        nodes,
        uses_openvpn,
        uses_amnezia_native,
        uses_amnezia_container,
        uses_container_runtime,
    )
}

pub(super) fn amnezia_node_requires_native_backend_impl(node: &NormalizedNode) -> Result<bool, String> {
    amnezia::amnezia_node_requires_native_backend_impl(node)
}

pub(super) fn amnezia_config_requires_native_backend_impl(value: &str) -> Result<bool, String> {
    amnezia::amnezia_config_requires_native_backend_impl(value)
}