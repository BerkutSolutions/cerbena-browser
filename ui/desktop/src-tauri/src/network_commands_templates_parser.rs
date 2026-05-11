use super::*;
use flate2::read::ZlibDecoder;

#[allow(dead_code)]
pub(crate) fn parse_amnezia_key_endpoint_impl(value: &str) -> Result<(String, u16), String> {
    let (host, port, _) = parse_amnezia_key_details_impl(value)?;
    Ok((host, port))
}

pub(crate) fn parse_amnezia_key_details_impl(value: &str) -> Result<(String, u16, Option<String>), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("amnezia key is required".to_string());
    }
    if looks_like_amnezia_conf_impl(trimmed) {
        return parse_amnezia_conf_details_impl(trimmed);
    }
    let encoded = match trimmed.get(0..6) {
        Some(prefix) if prefix.eq_ignore_ascii_case("vpn://") => trimmed.get(6..).unwrap_or_default().trim(),
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

    let json: serde_json::Value =
        serde_json::from_str(&inflated).map_err(|_| "amnezia key JSON is invalid".to_string())?;
    let endpoint = if let Some(endpoint) = extract_endpoint_from_json_impl(&json) {
        endpoint
    } else if let (Some(host), Some(port)) =
        (extract_host_hint_from_json_impl(&json), extract_port_hint_from_json_impl(&json))
    {
        (host, port)
    } else {
        return Err("amnezia key does not contain endpoint".to_string());
    };
    let transport = extract_transport_hint_from_json_impl(&json);
    Ok((endpoint.0, endpoint.1, transport))
}

pub(crate) fn looks_like_amnezia_conf_impl(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("[interface]") && lower.contains("[peer]")
}

pub(crate) fn parse_amnezia_conf_details_impl(value: &str) -> Result<(String, u16, Option<String>), String> {
    let sections = parse_ini_sections_impl(value);
    let endpoint = sections
        .get("peer")
        .and_then(|peer| peer.get("endpoint"))
        .map(String::as_str)
        .and_then(parse_host_port_pair_impl)
        .ok_or_else(|| "amnezia config does not contain endpoint".to_string())?;
    let transport = sections
        .get("interface")
        .and_then(|iface| iface.get("protocol").or_else(|| iface.get("transport")).or_else(|| iface.get("transport_proto")))
        .map(String::as_str)
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .filter(|value| value == "udp" || value == "tcp")
        .or_else(|| Some("udp".to_string()));
    Ok((endpoint.0, endpoint.1, transport))
}

pub(crate) fn parse_ini_sections_impl(value: &str) -> BTreeMap<String, BTreeMap<String, String>> {
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

pub(crate) fn extract_endpoint_from_json_impl(value: &serde_json::Value) -> Option<(String, u16)> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(endpoint) = map.get("endpoint").and_then(serde_json::Value::as_str) {
                if let Some(parsed) = parse_host_port_pair_impl(endpoint) {
                    return Some(parsed);
                }
            }
            let host = object_host_hint_impl(map);
            let port = map.get("port").and_then(parse_json_port_value_impl);
            if let (Some(host), Some(port)) = (host, port) {
                return Some((host, port));
            }
            if let Some(server) = map.get("server").and_then(serde_json::Value::as_str) {
                if let Some(parsed) = parse_host_port_pair_impl(server) {
                    return Some(parsed);
                }
            }
            for nested in map.values() {
                if let Some(parsed) = extract_endpoint_from_json_impl(nested) {
                    return Some(parsed);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(parsed) = extract_endpoint_from_json_impl(item) {
                    return Some(parsed);
                }
            }
            None
        }
        _ => None,
    }
}

pub(crate) fn object_host_hint_impl(map: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    for key in ["host", "hostname", "host_name", "hostName", "server"] {
        if let Some(value) = map.get(key).and_then(serde_json::Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                if let Some((host, port)) = parse_host_port_pair_impl(trimmed) {
                    if port > 0 {
                        return Some(host);
                    }
                }
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub(crate) fn extract_host_hint_from_json_impl(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(host) = object_host_hint_impl(map) {
                return Some(host);
            }
            for nested in map.values() {
                if let Some(host) = extract_host_hint_from_json_impl(nested) {
                    return Some(host);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(host) = extract_host_hint_from_json_impl(item) {
                    return Some(host);
                }
            }
            None
        }
        _ => None,
    }
}

pub(crate) fn extract_port_hint_from_json_impl(value: &serde_json::Value) -> Option<u16> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(port) = map
                .get("endpoint_port")
                .and_then(parse_json_port_value_impl)
                .or_else(|| map.get("remote_port").and_then(parse_json_port_value_impl))
                .or_else(|| map.get("port").and_then(parse_json_port_value_impl))
            {
                return Some(port);
            }
            for nested in map.values() {
                if let Some(port) = extract_port_hint_from_json_impl(nested) {
                    return Some(port);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(port) = extract_port_hint_from_json_impl(item) {
                    return Some(port);
                }
            }
            None
        }
        _ => None,
    }
}

pub(crate) fn extract_transport_hint_from_json_impl(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            for (key, nested) in map {
                if matches!(key.as_str(), "transport_proto" | "transport" | "proto" | "protocol") {
                    if let Some(raw) = nested.as_str() {
                        let normalized = raw.trim().to_lowercase();
                        if normalized == "udp" || normalized == "tcp" {
                            return Some(normalized);
                        }
                    }
                }
            }
            for nested in map.values() {
                if let Some(transport) = extract_transport_hint_from_json_impl(nested) {
                    return Some(transport);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(transport) = extract_transport_hint_from_json_impl(item) {
                    return Some(transport);
                }
            }
            None
        }
        _ => None,
    }
}

pub(crate) fn parse_json_port_value_impl(value: &serde_json::Value) -> Option<u16> {
    match value {
        serde_json::Value::Number(number) => number
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value > 0),
        serde_json::Value::String(raw) => raw.trim().parse::<u16>().ok().filter(|value| *value > 0),
        _ => None,
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

#[cfg(test)]
mod tests {
#![allow(dead_code)]

use super::*;
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    fn build_amnezia_key(payload: &str) -> String {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(payload.as_bytes())
            .expect("write amnezia payload");
        let compressed = encoder.finish().expect("finish compression");

        let mut framed = Vec::with_capacity(compressed.len() + 4);
        let len = payload.len() as u32;
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(&compressed);

        format!("vpn://{}", URL_SAFE_NO_PAD.encode(framed))
    }

    #[test]
    fn parse_amnezia_key_endpoint_extracts_host_and_port() {
        let key = build_amnezia_key(r#"{"endpoint":"demo.example:443"}"#);
        let endpoint = parse_amnezia_key_endpoint_impl(&key).expect("parse endpoint");
        assert_eq!(endpoint.0, "demo.example");
        assert_eq!(endpoint.1, 443);
    }
}
