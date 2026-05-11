use super::*;

pub(crate) fn v2ray_outbound_impl(node: &NormalizedNode, tag: &str) -> Result<Value, String> {
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
                "type": "vmess", "tag": tag, "server": host, "server_port": port, "uuid": uuid, "alter_id": alter_id, "security": security
            });
            apply_v2ray_transport_and_tls_impl(&mut out, node)?;
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
            let mut out =
                json!({ "type": "vless", "tag": tag, "server": host, "server_port": port, "uuid": uuid });
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
            apply_v2ray_transport_and_tls_impl(&mut out, node)?;
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
                "type": "trojan", "tag": tag, "server": host, "server_port": port, "password": password
            });
            apply_v2ray_transport_and_tls_impl(&mut out, node)?;
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
                            json!(
                                alpn.split(',')
                                    .map(str::trim)
                                    .filter(|value| !value.is_empty())
                                    .collect::<Vec<_>>()
                            ),
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
                "type": "shadowsocks", "tag": tag, "server": host, "server_port": port, "method": method, "password": password
            }))
        }
        _ => Err("unsupported v2ray protocol for runtime".to_string()),
    }
}

pub(crate) fn apply_v2ray_transport_and_tls_impl(
    outbound: &mut Value,
    node: &NormalizedNode,
) -> Result<(), String> {
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
                        transport_map.insert(
                            "headers".to_string(),
                            json!({ "Host": ws_host.trim() }),
                        );
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
                    json!({ "type": "grpc", "service_name": service.trim().trim_start_matches('/') }),
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
                    .map(|value| {
                        value.eq_ignore_ascii_case("on") || value.eq_ignore_ascii_case("true")
                    })
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
                        json!({ "enabled": true, "fingerprint": fingerprint }),
                    );
                    tls_map.insert(
                        "reality".to_string(),
                        json!({ "enabled": true, "public_key": public_key, "short_id": short_id }),
                    );
                }
            }
            map.insert("tls".to_string(), tls);
        }
    }
    Ok(())
}

