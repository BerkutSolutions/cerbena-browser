use super::*;

pub(crate) fn decode_amnezia_json_impl(value: &str) -> Result<Value, String> {
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
        inflate_zlib_to_string_impl(&decoded[4..]).or_else(|_| inflate_zlib_to_string_impl(&decoded))
    } else {
        inflate_zlib_to_string_impl(&decoded)
    }
    .map_err(|_| "amnezia key payload compression is invalid".to_string())?;

    serde_json::from_str::<Value>(&inflated).map_err(|_| "amnezia key JSON is invalid".to_string())
}

pub(crate) fn inflate_zlib_to_string_impl(bytes: &[u8]) -> Result<String, String> {
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

pub(crate) fn extract_awg_payload_impl(value: &Value) -> Option<Value> {
    let containers = value.get("containers")?.as_array()?;
    for container in containers {
        if let Some(awg) = container.get("awg") {
            return Some(awg.clone());
        }
    }
    None
}

pub(crate) fn extract_host_port_from_config_impl(
    config: &Value,
    root: &Value,
    awg: &Value,
) -> Result<(String, u16), String> {
    if let Some(endpoint) = extract_string_impl(config, &["endpoint", "server", "address"]) {
        if let Some(parsed) = parse_host_port_pair_impl(&endpoint) {
            return Ok(parsed);
        }
    }
    let host = extract_string_impl(config, &["hostName", "hostname", "host"])
        .or_else(|| extract_string_impl(root, &["hostName", "hostname", "host"]))
        .or_else(|| extract_string_impl(awg, &["hostName", "hostname", "host"]))
        .ok_or_else(|| "amnezia key does not contain endpoint host".to_string())?;
    let port = extract_u16_impl(config, &["port", "endpoint_port"])
        .or_else(|| extract_u16_impl(root, &["port", "endpoint_port"]))
        .or_else(|| extract_u16_impl(awg, &["port", "endpoint_port"]))
        .ok_or_else(|| "amnezia key does not contain endpoint port".to_string())?;
    Ok((host, port))
}

pub(crate) fn extract_string_impl(value: &Value, keys: &[&str]) -> Option<String> {
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

pub(crate) fn extract_u16_impl(value: &Value, keys: &[&str]) -> Option<u16> {
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

pub(crate) fn extract_string_array_impl(value: &Value, key: &str) -> Option<Vec<String>> {
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

pub(crate) fn extract_ini_value_impl(config: &Value, section: &str, key: &str) -> Option<String> {
    let raw = extract_string_impl(config, &["config"])?;
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

pub(crate) fn split_csv_values_impl(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>()
}

pub(crate) fn normalize_interface_address_impl(value: &str) -> String {
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

pub(crate) fn parse_host_port_pair_impl(raw: &str) -> Option<(String, u16)> {
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
