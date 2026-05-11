use super::*;

pub(crate) fn proxy_outbound_impl(node: &NormalizedNode, tag: &str) -> Result<Value, String> {
    let host = node
        .host
        .clone()
        .ok_or_else(|| "proxy host is required".to_string())?;
    let port = node
        .port
        .ok_or_else(|| "proxy port is required".to_string())?;
    match node.protocol.as_str() {
        "http" => {
            let mut out =
                json!({ "type": "http", "tag": tag, "server": host, "server_port": port });
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
                "version": version
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

