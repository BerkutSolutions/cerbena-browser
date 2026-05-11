use super::*;

pub(crate) fn vpn_runtime_entry_impl(
    node: &NormalizedNode,
    tag: &str,
    target: RuntimeExecutionTarget,
) -> Result<SingBoxRuntimeEntry, String> {
    if node.protocol == "wireguard" {
        return Ok(SingBoxRuntimeEntry::Endpoint(wireguard_endpoint_impl(node, tag)?));
    }
    if node.protocol == "amnezia" {
        return Ok(SingBoxRuntimeEntry::Endpoint(amnezia_endpoint_impl(node, tag)?));
    }
    if node.protocol == "openvpn" {
        return if target == RuntimeExecutionTarget::Container {
            Err("openvpn requires dedicated container-openvpn backend".to_string())
        } else {
            Err(
                "openvpn runtime requires native openvpn backend and is not yet available in sing-box mode"
                    .to_string(),
            )
        };
    }
    Err("unsupported vpn protocol for runtime".to_string())
}

pub(crate) fn wireguard_endpoint_impl(node: &NormalizedNode, tag: &str) -> Result<Value, String> {
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
    let mut peer =
        json!({ "address": host, "port": port, "public_key": public_key, "allowed_ips": allowed_ips });
    if let Some(psk) = node
        .settings
        .get("preSharedKey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(peer_map) = peer.as_object_mut() {
            peer_map.insert("pre_shared_key".to_string(), json!(psk));
        }
    }
    Ok(json!({
        "type": "wireguard", "tag": tag, "system": false, "private_key": private_key, "address": address,
        "peers": [peer], "workers": 1, "mtu": 1408
    }))
}

pub(crate) fn amnezia_endpoint_impl(node: &NormalizedNode, tag: &str) -> Result<Value, String> {
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
        "address": config.host, "port": config.port, "public_key": config.server_public_key, "allowed_ips": config.allowed_ips
    });
    if let Some(psk) = config.pre_shared_key.filter(|value| !value.is_empty()) {
        if let Some(peer_map) = peer.as_object_mut() {
            peer_map.insert("pre_shared_key".to_string(), json!(psk));
        }
    }
    Ok(json!({
        "type": "wireguard", "tag": tag, "system": false, "private_key": config.client_private_key,
        "address": config.addresses, "peers": [peer], "workers": 1, "mtu": config.mtu.unwrap_or(1408)
    }))
}

