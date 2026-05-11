use super::*;

pub(crate) fn backup_corrupt_traffic_log(path: &PathBuf, bytes: &[u8]) {
    let backup_path = path.with_extension(format!("corrupt-{}.json", now_epoch_ms()));
    if fs::rename(path, &backup_path).is_err() {
        let _ = fs::write(backup_path, bytes);
        let _ = fs::remove_file(path);
    }
}

pub(crate) fn persist_traffic_log(path: &PathBuf, entries: &[TrafficLogEntry]) -> Result<(), String> {
    let bytes =
        serde_json::to_vec_pretty(entries).map_err(|e| format!("serialize traffic log: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write traffic log: {e}"))
}

pub fn ensure_profile_gateway(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Result<GatewayLaunchConfig, String> {
    transport::ensure_profile_gateway_impl(app_handle, profile_id)
}

pub fn stop_profile_gateway(app_handle: &AppHandle, profile_id: Uuid) {
    transport::stop_profile_gateway_impl(app_handle, profile_id)
}

pub fn stop_all_profile_gateways(app_handle: &AppHandle) {
    transport::stop_all_profile_gateways_impl(app_handle)
}

pub fn list_traffic_log(state: &AppState) -> Result<Vec<TrafficLogEntry>, String> {
    telemetry::list_traffic_log_impl(state)
}

pub fn set_domain_block_rule(
    state: &AppState,
    profile_id: Option<String>,
    domain: &str,
    blocked: bool,
) -> Result<(), String> {
    let normalized = normalize_domain(domain);
    if normalized.is_empty() {
        return Err("domain is empty".to_string());
    }
    let mut gateway = state
        .traffic_gateway
        .lock()
        .map_err(|_| "traffic gateway lock poisoned".to_string())?;
    if let Some(profile_id) = profile_id {
        let entry = gateway
            .rules
            .profile_blocked_domains
            .entry(profile_id)
            .or_default();
        if blocked {
            entry.insert(normalized);
        } else {
            entry.remove(&normalized);
        }
    } else if blocked {
        gateway.rules.global_blocked_domains.insert(normalized);
    } else {
        gateway.rules.global_blocked_domains.remove(&normalized);
    }
    let path = state.traffic_gateway_rules_path(&state.app_handle)?;
    persist_rules_store(&path, &gateway.rules)
}

pub(crate) fn handle_client(
    app_handle: AppHandle,
    profile_id: Uuid,
    mut client: TcpStream,
) -> Result<(), String> {
    client
        .set_nonblocking(false)
        .map_err(|e| format!("client blocking mode: {e}"))?;
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    client
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;

    let started_at = Instant::now();
    let parsed = read_proxy_request(&mut client)?;
    let profile_name = profile_name(&app_handle, profile_id);
    let decision = evaluate_request(&app_handle, profile_id, &parsed.host);
    eprintln!(
        "[traffic-gateway] profile={} host={} kind={} blocked={} reason={}",
        profile_id, parsed.host, parsed.request_kind, decision.blocked, decision.reason
    );
    if decision.blocked {
        write_forbidden(&mut client, &parsed.host, &decision.reason).map_err(|e| e.to_string())?;
        append_traffic_log(
            &app_handle,
            TrafficLogEntry {
                id: format!("traffic-{}", now_epoch_ms()),
                timestamp_epoch_ms: now_epoch_ms(),
                profile_id: profile_id.to_string(),
                profile_name,
                request_host: parsed.host.clone(),
                request_kind: parsed.request_kind.clone(),
                status: "blocked".to_string(),
                reason: decision.reason,
                route: decision.route,
                latency_ms: started_at.elapsed().as_millis(),
                source_ip: client
                    .peer_addr()
                    .map(|addr| addr.ip().to_string())
                    .unwrap_or_else(|_| "127.0.0.1".to_string()),
                blocked_globally: decision.blocked_globally,
                blocked_for_profile: decision.blocked_for_profile,
            },
        );
        return Ok(());
    }

    let route_policy = current_route_policy(&app_handle, profile_id);
    let processing_result = if parsed.connect_tunnel {
        handle_connect_request(&app_handle, profile_id, &mut client, &parsed, &route_policy)
    } else {
        handle_http_request(&app_handle, profile_id, &mut client, &parsed, &route_policy)
    };
    let (status, reason) = match processing_result {
        Ok(_) => ("processed".to_string(), "allowed".to_string()),
        Err(error) => ("error".to_string(), error.to_string()),
    };
    append_traffic_log(
        &app_handle,
        TrafficLogEntry {
            id: format!("traffic-{}", now_epoch_ms()),
            timestamp_epoch_ms: now_epoch_ms(),
            profile_id: profile_id.to_string(),
            profile_name,
            request_host: parsed.host,
            request_kind: parsed.request_kind,
            status,
            reason: reason.clone(),
            route: decision.route,
            latency_ms: started_at.elapsed().as_millis(),
            source_ip: client
                .peer_addr()
                .map(|addr| addr.ip().to_string())
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            blocked_globally: false,
            blocked_for_profile: false,
        },
    );
    if reason == "allowed" {
        Ok(())
    } else {
        Err(reason)
    }
}

pub(super) fn read_proxy_request(client: &mut TcpStream) -> Result<ParsedProxyRequest, String> {
    traffic_gateway_tunnel::read_proxy_request(client)
}

pub(super) fn handle_connect_request(
    app_handle: &AppHandle,
    profile_id: Uuid,
    client: &mut TcpStream,
    parsed: &ParsedProxyRequest,
    route_policy: &Option<VpnProxyTabPayload>,
) -> std::io::Result<()> {
    traffic_gateway_tunnel::handle_connect_request(app_handle, profile_id, client, parsed, route_policy)
}

pub(super) fn handle_http_request(
    app_handle: &AppHandle,
    profile_id: Uuid,
    client: &mut TcpStream,
    parsed: &ParsedProxyRequest,
    route_policy: &Option<VpnProxyTabPayload>,
) -> std::io::Result<()> {
    traffic_gateway_tunnel::handle_http_request(app_handle, profile_id, client, parsed, route_policy)
}

pub(crate) fn write_forbidden(client: &mut TcpStream, host: &str, reason: &str) -> std::io::Result<()> {
    let body = format!("Blocked by Cerbena gateway: {host}\nReason: {reason}\n");
    let response = format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    client.write_all(response.as_bytes())?;
    client.flush()
}

pub(crate) fn append_traffic_log(app_handle: &AppHandle, entry: TrafficLogEntry) {
    let state = app_handle.state::<AppState>();
    let mut gateway = match state.traffic_gateway.lock() {
        Ok(value) => value,
        Err(_) => return,
    };
    gateway.traffic_log.push(entry.clone());
    prune_traffic_log(&mut gateway.traffic_log);
    let snapshot = gateway.traffic_log.clone();
    drop(gateway);
    if let Ok(path) = state.traffic_gateway_log_path(app_handle) {
        let _ = persist_traffic_log(&path, &snapshot);
    }
    let _ = app_handle.emit("traffic-gateway-event", &entry);
}

pub(crate) fn prune_traffic_log(entries: &mut Vec<TrafficLogEntry>) {
    let cutoff = now_epoch_ms().saturating_sub(TRAFFIC_RETENTION_MS);
    entries.retain(|entry| entry.timestamp_epoch_ms >= cutoff);
}

pub(super) fn evaluate_request(app_handle: &AppHandle, profile_id: Uuid, host: &str) -> GatewayDecision {
    traffic_gateway_policy::evaluate_request(app_handle, profile_id, host)
}

pub(crate) fn current_route_policy(app_handle: &AppHandle, profile_id: Uuid) -> Option<VpnProxyTabPayload> {
    let state = app_handle.state::<AppState>();
    let store = state.network_store.lock().ok()?;
    store.vpn_proxy.get(&profile_id.to_string()).cloned()
}

pub(crate) fn selected_route_template(
    state: &AppState,
    profile_id: Uuid,
) -> Option<crate::state::ConnectionTemplate> {
    let store = state.network_store.lock().ok()?;
    let profile_key = profile_id.to_string();
    let profile_route_mode = store
        .vpn_proxy
        .get(&profile_key)
        .map(|value| value.route_mode.trim().to_lowercase())
        .unwrap_or_else(|| "direct".to_string());
    if profile_route_mode == "direct" {
        return None;
    }
    let template_id = if store.global_route_settings.global_vpn_enabled {
        store
            .global_route_settings
            .default_template_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    } else {
        store.profile_template_selection.get(&profile_key).cloned()
    };
    template_id.and_then(|id| store.connection_templates.get(&id).cloned())
}

pub(crate) fn profile_route_mode(state: &AppState, profile_id: Uuid) -> String {
    state
        .network_store
        .lock()
        .ok()
        .and_then(|store| store.vpn_proxy.get(&profile_id.to_string()).cloned())
        .map(|payload| payload.route_mode.trim().to_lowercase())
        .unwrap_or_else(|| "direct".to_string())
}

pub(crate) fn profile_name(app_handle: &AppHandle, profile_id: Uuid) -> String {
    let state = app_handle.state::<AppState>();
    state
        .manager
        .lock()
        .ok()
        .and_then(|manager| manager.get_profile(profile_id).ok())
        .map(|profile| profile.name)
        .unwrap_or_else(|| profile_id.to_string())
}

pub(crate) fn normalize_domain(domain: &str) -> String {
    domain
        .trim()
        .trim_start_matches("*.")
        .trim_start_matches('.')
        .trim_end_matches('.')
        .to_lowercase()
}

pub(crate) fn host_matches(host: &str, rule: &str) -> bool {
    let normalized_rule = normalize_domain(rule);
    if normalized_rule.is_empty() {
        return false;
    }
    host == normalized_rule || host.ends_with(&format!(".{normalized_rule}"))
}

pub(crate) fn split_host_port(value: &str, default_port: u16) -> (String, u16) {
    let raw = value.trim();
    if let Some((host, port)) = raw.rsplit_once(':') {
        if let Ok(port) = port.parse::<u16>() {
            return (normalize_domain(host), port);
        }
    }
    (normalize_domain(raw), default_port)
}

pub(crate) fn parse_first_bridge_endpoint(bridges: &str) -> Option<(String, u16)> {
    for line in bridges
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        if let Some((host, port)) = parts[1].rsplit_once(':') {
            if let Ok(port) = port.parse::<u16>() {
                return Some((host.to_string(), port));
            }
        }
    }
    None
}

pub(crate) fn normalize_http_first_line(first_line: &str, absolute_target: &str) -> (String, u16, String) {
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let version = parts.next_back().unwrap_or("HTTP/1.1");
    if let Some(rest) = absolute_target.strip_prefix("http://") {
        let (authority, path) = rest
            .split_once('/')
            .map(|(host, path)| (host, format!("/{path}")))
            .unwrap_or((rest, "/".to_string()));
        let (host, port) = split_host_port(authority, 80);
        return (host, port, format!("{method} {path} {version}"));
    }
    if let Some(rest) = absolute_target.strip_prefix("https://") {
        let (authority, path) = rest
            .split_once('/')
            .map(|(host, path)| (host, format!("/{path}")))
            .unwrap_or((rest, "/".to_string()));
        let (host, port) = split_host_port(authority, 443);
        return (host, port, format!("{method} {path} {version}"));
    }
    ("".to_string(), 80, first_line.to_string())
}

pub(crate) fn rebuild_header(
    original: &[u8],
    original_first_line: &str,
    rewritten_first_line: &str,
) -> Vec<u8> {
    let mut text = String::from_utf8_lossy(original).to_string();
    text = text.replacen(original_first_line, rewritten_first_line, 1);
    text.into_bytes()
}

pub(crate) fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::load_traffic_log;
    use super::super::traffic_gateway_policy::service_matches_host;
    use super::super::traffic_gateway_tunnel::is_expected_bridge_disconnect;
    use std::{
        fs, io,
        time::{SystemTime, UNIX_EPOCH},
    };

    pub(crate) fn temp_path(prefix: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cerbena-{prefix}-{unique}.json"))
    }

    #[test]
    pub(crate) fn vk_service_does_not_match_unrelated_com_domains() {
        assert!(!service_matches_host("duckduckgo.com", "vk_com"));
        assert!(!service_matches_host("myip.com", "vk_com"));
    }

    #[test]
    pub(crate) fn vk_service_matches_vk_domains() {
        assert!(service_matches_host("vk.com", "vk_com"));
        assert!(service_matches_host("m.vk.com", "vk_com"));
    }

    #[test]
    pub(crate) fn corrupt_traffic_log_is_ignored_instead_of_crashing() {
        let path = temp_path("traffic-log-corrupt");
        fs::write(&path, b"[]\ncorrupt").expect("write corrupt traffic log");

        let loaded = load_traffic_log(&path).expect("load traffic log");

        assert!(loaded.is_empty());
        let corrupt_backup = fs::read_dir(path.parent().expect("temp parent"))
            .expect("read temp dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|candidate| {
                candidate
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(|value| {
                        value.contains("traffic-log-corrupt") && value.contains(".corrupt-")
                    })
                    .unwrap_or(false)
            });
        assert!(corrupt_backup.is_some());

        let _ = fs::remove_file(path);
        if let Some(backup) = corrupt_backup {
            let _ = fs::remove_file(backup);
        }
    }

    #[test]
    pub(crate) fn expected_bridge_disconnects_are_suppressed() {
        assert!(is_expected_bridge_disconnect(&io::Error::new(
            io::ErrorKind::ConnectionReset,
            "reset"
        )));
        assert!(is_expected_bridge_disconnect(&io::Error::new(
            io::ErrorKind::BrokenPipe,
            "broken pipe"
        )));
    }

    #[test]
    pub(crate) fn unexpected_bridge_errors_are_not_suppressed() {
        assert!(!is_expected_bridge_disconnect(&io::Error::new(
            io::ErrorKind::PermissionDenied,
            "denied"
        )));
    }
}

