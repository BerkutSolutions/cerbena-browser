use super::*;

pub(crate) fn launch_amneziawg_runtime_impl(
    node: &NormalizedNode,
    runtime_dir: &PathBuf,
    profile_id: Uuid,
    binary: &str,
) -> Result<AmneziaWgLaunch, String> {
    let key = node
        .settings
        .get("amneziaKey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "amnezia key is required".to_string())?;
    let tunnel_name = amnezia_tunnel_name_impl(profile_id);
    let config_path = runtime_dir.join(format!("{tunnel_name}.conf"));
    let config_text = build_amnezia_native_config_text_impl(key)?;
    fs::write(&config_path, config_text).map_err(|e| format!("write amnezia config: {e}"))?;
    let binary_path = PathBuf::from(binary);

    if amnezia_tunnel_service_exists_impl(&tunnel_name) {
        let _ = stop_amnezia_tunnel_service_impl(&tunnel_name);
        let _ = wait_amnezia_tunnel_state_impl(&tunnel_name, false, 8_000);
        uninstall_amnezia_tunnel_impl(&binary_path, &tunnel_name).map_err(|error| {
            format!("failed to reset existing amneziawg tunnel service: {error}")
        })?;
    }

    install_amnezia_tunnel_impl(&binary_path, &config_path, &tunnel_name)?;
    if !is_amnezia_tunnel_active_impl(&tunnel_name) {
        start_amnezia_tunnel_service_impl(&tunnel_name)?;
    }

    if let Err(error) = wait_amnezia_tunnel_state_impl(&tunnel_name, true, 45_000) {
        let status = describe_amnezia_tunnel_status_impl(&tunnel_name);
        let cleanup_error = uninstall_amnezia_tunnel_impl(&binary_path, &tunnel_name).err();
        return Err(match cleanup_error {
            Some(cleanup) => format!("{error}. {status}. Cleanup failed: {cleanup}"),
            None => format!("{error}. {status}"),
        });
    }

    Ok(AmneziaWgLaunch {
        config_path,
        cleanup_paths: Vec::new(),
        tunnel_name,
    })
}

fn install_amnezia_tunnel_impl(
    binary: &PathBuf,
    config_path: &PathBuf,
    tunnel_name: &str,
) -> Result<(), String> {
    let args = vec![
        "/installtunnelservice".to_string(),
        config_path.to_string_lossy().to_string(),
    ];
    let output = run_amneziawg_command(binary, &args, "install tunnel")?;
    if output.status.success() {
        return Ok(());
    }
    if is_amnezia_access_denied(&output) {
        let elevated = run_amneziawg_command_elevated(binary, &args, "install tunnel");
        match elevated {
            Ok(out) if out.status.success() => return Ok(()),
            Ok(out) => {
                if is_uac_elevation_cancelled(&out) {
                    return Err(
                        "amneziawg tunnel install requires administrator approval (UAC was cancelled)"
                            .to_string(),
                    );
                }
                let reason = describe_process_failure(&out, "amneziawg elevated install");
                let _ = uninstall_amnezia_tunnel_impl(binary, tunnel_name);
                return Err(format!(
                    "amneziawg tunnel install failed after elevation attempt: {reason}"
                ));
            }
            Err(error) => {
                let _ = uninstall_amnezia_tunnel_impl(binary, tunnel_name);
                return Err(format!(
                    "amneziawg tunnel install requires administrator privileges: {error}"
                ));
            }
        }
    }
    let reason = describe_process_failure(&output, "amneziawg install");
    let _ = uninstall_amnezia_tunnel_impl(binary, tunnel_name);
    Err(format!("amneziawg tunnel install failed: {reason}"))
}

pub(crate) fn amnezia_tunnel_name_impl(profile_id: Uuid) -> String {
    let mut name = format!("awg-{}", profile_id.as_simple());
    if name.len() > 32 {
        name.truncate(32);
    }
    name
}

pub(crate) fn build_amnezia_native_config_text_impl(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("amnezia key is required".to_string());
    }
    if looks_like_amnezia_conf(trimmed) {
        return Ok(sanitize_amnezia_conf_text_impl(trimmed));
    }

    let root = decode_amnezia_json(trimmed)?;
    let awg = extract_awg_payload(&root)
        .ok_or_else(|| "amnezia key does not contain awg payload".to_string())?;
    if let Some(config) = extract_amnezia_conf_text_from_payload(&root, &awg) {
        return Ok(config);
    }

    let config = parse_amnezia_runtime_config(trimmed)?;
    let mut lines = Vec::new();
    lines.push("[Interface]".to_string());
    if !config.addresses.is_empty() {
        lines.push(format!("Address = {}", config.addresses.join(", ")));
    }
    if let Some(dns) = extract_amnezia_dns_pair(&root, &awg) {
        lines.push(format!("DNS = {dns}"));
    }
    lines.push(format!("PrivateKey = {}", config.client_private_key));
    if let Some(mtu) = config.mtu {
        lines.push(format!("MTU = {mtu}"));
    }

    for key in [
        "Jc", "Jmin", "Jmax", "S1", "S2", "S3", "S4", "H1", "H2", "H3", "H4", "I1", "I2", "I3",
        "I4", "I5",
    ] {
        if let Some(raw) = extract_string_case_insensitive(&awg, key) {
            if let Some(sanitized) = sanitize_amnezia_native_field_value_impl(&raw) {
                lines.push(format!("{key} = {sanitized}"));
            }
        }
    }

    lines.push(String::new());
    lines.push("[Peer]".to_string());
    lines.push(format!("PublicKey = {}", config.server_public_key));
    if let Some(psk) = config.pre_shared_key.filter(|item| !item.trim().is_empty()) {
        lines.push(format!("PresharedKey = {}", psk.trim()));
    }
    lines.push(format!("AllowedIPs = {}", config.allowed_ips.join(", ")));
    lines.push(format!("Endpoint = {}:{}", config.host, config.port));
    if let Some(keepalive) =
        extract_string_case_insensitive(&awg, "persistent_keep_alive").or_else(|| {
            extract_string_case_insensitive(&awg, "persistentKeepalive")
                .or_else(|| extract_string_case_insensitive(&awg, "persistent_keepalive"))
        })
    {
        let trimmed_keepalive = keepalive.trim();
        if !trimmed_keepalive.is_empty() {
            lines.push(format!("PersistentKeepalive = {trimmed_keepalive}"));
        }
    }
    Ok(lines.join("\n") + "\n")
}

pub(crate) fn sanitize_amnezia_conf_text_impl(value: &str) -> String {
    let mut cleaned = Vec::new();
    for raw_line in value.replace('\r', "").lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            cleaned.push(String::new());
            continue;
        }
        let Some((left, right)) = line.split_once('=') else {
            cleaned.push(line.to_string());
            continue;
        };
        let key = left.trim();
        let val = right.trim();
        if is_amnezia_native_only_key(key) {
            let Some(normalized) = sanitize_amnezia_native_field_value_impl(val) else {
                continue;
            };
            cleaned.push(format!("{key} = {normalized}"));
            continue;
        }
        cleaned.push(format!("{key} = {val}"));
    }
    let mut text = cleaned.join("\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn sanitize_amnezia_native_field_value_impl(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed
        .strip_prefix('"')
        .and_then(|item| item.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|item| item.strip_suffix('\''))
        })
        .unwrap_or(trimmed)
        .trim();

    (!normalized.is_empty()).then(|| normalized.to_string())
}

pub(crate) fn uninstall_amnezia_tunnel_impl(
    binary: &PathBuf,
    tunnel_name: &str,
) -> Result<(), String> {
    super::uninstall_amnezia_tunnel(binary, tunnel_name)
}

pub(crate) fn stop_amnezia_tunnel_service_impl(tunnel_name: &str) -> Result<(), String> {
    super::stop_amnezia_tunnel_service(tunnel_name)
}

pub(crate) fn delete_amnezia_tunnel_service_impl(tunnel_name: &str) -> Result<(), String> {
    super::delete_amnezia_tunnel_service(tunnel_name)
}

pub(crate) fn wait_amnezia_tunnel_state_impl(
    tunnel_name: &str,
    should_be_active: bool,
    timeout_ms: u64,
) -> Result<(), String> {
    super::wait_amnezia_tunnel_state(tunnel_name, should_be_active, timeout_ms)
}

pub(crate) fn is_amnezia_tunnel_active_impl(tunnel_name: &str) -> bool {
    super::is_amnezia_tunnel_active(tunnel_name)
}

pub(crate) fn amnezia_tunnel_service_exists_impl(tunnel_name: &str) -> bool {
    super::amnezia_tunnel_service_exists(tunnel_name)
}

fn start_amnezia_tunnel_service_impl(tunnel_name: &str) -> Result<(), String> {
    super::start_amnezia_tunnel_service(tunnel_name)
}

fn describe_amnezia_tunnel_status_impl(tunnel_name: &str) -> String {
    super::describe_amnezia_tunnel_status(tunnel_name)
}

pub(crate) fn amnezia_node_requires_native_backend_impl(
    node: &NormalizedNode,
) -> Result<bool, String> {
    if node.connection_type != "vpn" || node.protocol != "amnezia" {
        return Ok(false);
    }
    let key = node
        .settings
        .get("amneziaKey")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "amnezia key is required".to_string())?;
    amnezia_config_requires_native_backend_impl(key)
}

pub(crate) fn amnezia_config_requires_native_backend_impl(value: &str) -> Result<bool, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("amnezia key is required".to_string());
    }
    if looks_like_amnezia_conf(trimmed) {
        return Ok(amnezia_conf_contains_native_fields(trimmed));
    }

    let root = decode_amnezia_json(trimmed)?;
    let awg = extract_awg_payload(&root)
        .ok_or_else(|| "amnezia key does not contain awg payload".to_string())?;
    if amnezia_json_contains_native_fields(&awg) {
        return Ok(true);
    }
    if let Some(config) = extract_amnezia_conf_text_from_payload(&root, &awg) {
        if amnezia_conf_contains_native_fields(&config) {
            return Ok(true);
        }
    }
    let runtime = parse_amnezia_runtime_config(trimmed)?;
    Ok(runtime
        .transport
        .as_deref()
        .map(|value| !value.eq_ignore_ascii_case("udp"))
        .unwrap_or(false))
}
