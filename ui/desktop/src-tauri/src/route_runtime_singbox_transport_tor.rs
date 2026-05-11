use super::*;

pub(crate) fn tor_outbound_impl(
    app_handle: &AppHandle,
    node: &NormalizedNode,
    tag: &str,
    target: RuntimeExecutionTarget,
) -> Result<Value, String> {
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
            let plugin_binary = if target == RuntimeExecutionTarget::Container {
                container_tor_transport_binary_impl(&node.protocol).ok_or_else(|| {
                    format!(
                        "tor {} pluggable transport is not available in container runtime yet",
                        node.protocol
                    )
                })?
            } else {
                resolve_tor_pt_binary_path(app_handle, &node.protocol)
                    .ok_or_else(|| {
                        format!(
                            "tor {} requires pluggable transport binary, but none is available",
                            node.protocol
                        )
                    })?
                    .to_string_lossy()
                    .to_string()
            };
            let transport = match node.protocol.as_str() {
                "obfs4" => "obfs4",
                "snowflake" => "snowflake",
                "meek" => "meek_lite",
                _ => "",
            };
            torrc.insert(
                "ClientTransportPlugin".to_string(),
                format!("{transport} exec {}", plugin_binary),
            );
        }
        _ => return Err("unsupported tor transport for runtime".to_string()),
    }

    let mut out = json!({
        "type": "tor",
        "tag": tag,
        "torrc": torrc,
    });
    if let Some(map) = out.as_object_mut() {
        if target == RuntimeExecutionTarget::Container {
            map.insert("executable_path".to_string(), json!("/usr/bin/tor"));
        } else if let Some(binary) = resolve_tor_binary_path(app_handle) {
            map.insert(
                "executable_path".to_string(),
                json!(binary.to_string_lossy().to_string()),
            );
        }
    }
    Ok(out)
}

pub(crate) fn container_tor_transport_binary_impl(protocol: &str) -> Option<String> {
    match protocol {
        "obfs4" => Some("/usr/bin/obfs4proxy".to_string()),
        "snowflake" => Some("/usr/bin/snowflake-client".to_string()),
        "meek" => Some("/usr/bin/obfs4proxy".to_string()),
        _ => None,
    }
}

