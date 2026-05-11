use super::*;
use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use flate2::read::ZlibDecoder;
use std::io::Read;

pub(crate) fn amnezia_conf_contains_native_fields_impl(text: &str) -> bool {
    text.replace('\r', "")
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(left, _)| left.trim())
        .any(is_amnezia_native_only_key_impl)
}

pub(crate) fn amnezia_json_contains_native_fields_impl(value: &Value) -> bool {
    const KEYS: &[&str] = &[
        "Jc", "Jmin", "Jmax", "S1", "S2", "S3", "S4", "H1", "H2", "H3", "H4", "I1", "I2", "I3",
        "I4", "I5",
    ];
    KEYS.iter()
        .any(|key| extract_string_case_insensitive(value, key).is_some())
}

pub(crate) fn is_amnezia_native_only_key_impl(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "jc" | "jmin"
            | "jmax"
            | "s1"
            | "s2"
            | "s3"
            | "s4"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "i1"
            | "i2"
            | "i3"
            | "i4"
            | "i5"
    )
}

pub(crate) fn parse_amnezia_runtime_config_impl(value: &str) -> Result<AmneziaRuntimeConfig, String> {
    if looks_like_amnezia_conf_impl(value) {
        return parse_amnezia_conf_runtime_config_impl(value);
    }
    let json = decode_amnezia_json_impl(value)?;
    let awg = extract_awg_payload_impl(&json)
        .ok_or_else(|| "amnezia key does not contain awg payload".to_string())?;
    let mut config = awg.clone();
    if let Some(last) = awg.get("last_config") {
        match last {
            Value::String(raw) => {
                if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
                    config = parsed;
                }
            }
            Value::Object(_) => {
                config = last.clone();
            }
            _ => {}
        }
    }

    let (host, port) = extract_host_port_from_config_impl(&config, &json, &awg)?;
    let client_private_key = extract_string_impl(&config, &["client_priv_key", "private_key"])
        .or_else(|| extract_ini_value_impl(&config, "Interface", "PrivateKey"))
        .ok_or_else(|| "amnezia key does not contain private key".to_string())?;
    let server_public_key = extract_string_impl(
        &config,
        &["server_pub_key", "peer_public_key", "public_key"],
    )
    .or_else(|| extract_ini_value_impl(&config, "Peer", "PublicKey"))
    .ok_or_else(|| "amnezia key does not contain server public key".to_string())?;
    let pre_shared_key = extract_string_impl(&config, &["psk_key", "pre_shared_key"])
        .or_else(|| extract_ini_value_impl(&config, "Peer", "PresharedKey"));

    let allowed_ips = extract_string_array_impl(&config, "allowed_ips").or_else(|| {
        extract_ini_value_impl(&config, "Peer", "AllowedIPs").map(|value| split_csv_values_impl(&value))
    });
    let allowed_ips = allowed_ips
        .unwrap_or_else(|| vec!["0.0.0.0/0".to_string(), "::/0".to_string()])
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let addresses = extract_string_array_impl(&config, "client_ip")
        .or_else(|| extract_string_array_impl(&config, "address"))
        .or_else(|| {
            extract_ini_value_impl(&config, "Interface", "Address").map(|value| split_csv_values_impl(&value))
        })
        .unwrap_or_default()
        .into_iter()
        .map(|value| normalize_interface_address_impl(&value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let mtu = extract_u16_impl(&config, &["mtu"]);
    let transport = extract_string_impl(&awg, &["transport_proto", "transport"]);

    Ok(AmneziaRuntimeConfig {
        host,
        port,
        client_private_key,
        server_public_key,
        pre_shared_key,
        addresses,
        allowed_ips,
        mtu,
        transport,
    })
}

pub(crate) fn looks_like_amnezia_conf_impl(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("[interface]") && lower.contains("[peer]")
}

pub(crate) fn parse_amnezia_conf_runtime_config_impl(
    value: &str,
) -> Result<AmneziaRuntimeConfig, String> {
    let sections = parse_ini_sections_impl(value);
    let interface = sections
        .get("interface")
        .ok_or_else(|| "amnezia config does not contain [Interface] section".to_string())?;
    let peer = sections
        .get("peer")
        .ok_or_else(|| "amnezia config does not contain [Peer] section".to_string())?;

    let endpoint = peer
        .get("endpoint")
        .map(String::as_str)
        .and_then(parse_host_port_pair_impl)
        .ok_or_else(|| "amnezia config does not contain endpoint".to_string())?;
    let client_private_key = interface
        .get("privatekey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "amnezia config does not contain private key".to_string())?
        .to_string();
    let server_public_key = peer
        .get("publickey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "amnezia config does not contain server public key".to_string())?
        .to_string();
    let pre_shared_key = peer
        .get("presharedkey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let addresses = interface
        .get("address")
        .map(String::as_str)
        .map(split_csv_values_impl)
        .unwrap_or_default()
        .into_iter()
        .map(|value| normalize_interface_address_impl(&value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let allowed_ips = peer
        .get("allowedips")
        .map(String::as_str)
        .map(split_csv_values_impl)
        .unwrap_or_else(|| vec!["0.0.0.0/0".to_string(), "::/0".to_string()])
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let mtu = interface
        .get("mtu")
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|value| *value > 0);
    let transport = interface
        .get("transport")
        .or_else(|| interface.get("transport_proto"))
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .filter(|value| value == "udp" || value == "tcp");

    Ok(AmneziaRuntimeConfig {
        host: endpoint.0,
        port: endpoint.1,
        client_private_key,
        server_public_key,
        pre_shared_key,
        addresses,
        allowed_ips,
        mtu,
        transport,
    })
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
        sections.entry(current_section.clone()).or_default().insert(
            key.trim().to_ascii_lowercase(),
            raw_value.trim().to_string(),
        );
    }
    sections
}


#[path = "route_runtime_parse_core_extract.rs"]
mod support;
pub(crate) use self::support::*;


