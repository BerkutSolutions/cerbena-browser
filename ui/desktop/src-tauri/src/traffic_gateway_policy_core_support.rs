use super::*;

pub(crate) fn proxy_unavailable_reason(
    proxy: Option<&browser_network_policy::ProxyTransportAdapter>,
) -> Option<String> {
    let Some(proxy) = proxy else {
        return Some("Kill-switch: proxy route is not configured".to_string());
    };
    if endpoint_reachable(&proxy.host, proxy.port, ROUTE_HEALTH_TIMEOUT_MS) {
        None
    } else {
        Some(format!(
            "Kill-switch: {} proxy endpoint is unavailable",
            proxy_protocol_label(proxy.protocol)
        ))
    }
}

pub(crate) fn vpn_unavailable_reason(
    vpn: Option<&browser_network_policy::VpnTunnelAdapter>,
) -> Option<String> {
    let Some(vpn) = vpn else {
        return Some("Kill-switch: VPN route is not configured".to_string());
    };
    if vpn_endpoint_reachable(
        vpn.protocol,
        &vpn.endpoint_host,
        vpn.endpoint_port,
        ROUTE_HEALTH_TIMEOUT_MS,
    ) {
        None
    } else {
        Some(format!(
            "Kill-switch: {} tunnel endpoint is unavailable",
            vpn_protocol_label(vpn.protocol)
        ))
    }
}

pub(crate) fn tor_unavailable_reason(app_handle: &AppHandle, profile_id: Uuid) -> Option<String> {
    let state = app_handle.state::<AppState>();
    let store = match state.network_store.lock() {
        Ok(value) => value,
        Err(_) => return Some("Kill-switch: unable to read route state".to_string()),
    };
    let Some(template_id) = store
        .profile_template_selection
        .get(&profile_id.to_string())
    else {
        return Some("Kill-switch: TOR route template is not selected".to_string());
    };
    let Some(template) = store.connection_templates.get(template_id) else {
        return Some("Kill-switch: TOR route template is missing".to_string());
    };
    let node = template
        .nodes
        .iter()
        .find(|item| item.connection_type == "tor");
    let Some(node) = node else {
        return Some("Kill-switch: TOR node is not configured in the template".to_string());
    };
    match node.protocol.as_str() {
        "obfs4" => {
            let bridge = node
                .bridges
                .as_deref()
                .and_then(parse_first_bridge_endpoint);
            let Some((host, port)) = bridge else {
                return Some("Kill-switch: TOR obfs4 bridge is not configured".to_string());
            };
            if endpoint_reachable(&host, port, ROUTE_HEALTH_TIMEOUT_MS) {
                None
            } else {
                Some("Kill-switch: TOR obfs4 bridge endpoint is unavailable".to_string())
            }
        }
        "snowflake" | "meek" | "none" => None,
        _ => Some("Kill-switch: TOR transport is unsupported".to_string()),
    }
}

pub(crate) fn endpoint_reachable(host: &str, port: u16, timeout_ms: u64) -> bool {
    if host.trim().is_empty() || port == 0 {
        return false;
    }
    let mut addrs = match (host, port).to_socket_addrs() {
        Ok(value) => value,
        Err(_) => return false,
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms.max(1))).is_ok()
}

pub(crate) fn udp_endpoint_reachable(host: &str, port: u16, timeout_ms: u64) -> bool {
    if host.trim().is_empty() || port == 0 {
        return false;
    }
    let mut addrs = match (host, port).to_socket_addrs() {
        Ok(value) => value,
        Err(_) => return false,
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    let bind_addr = if addr.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let socket = match UdpSocket::bind(bind_addr) {
        Ok(value) => value,
        Err(_) => return false,
    };
    if socket
        .set_write_timeout(Some(Duration::from_millis(timeout_ms.max(1))))
        .is_err()
    {
        return false;
    }
    if socket.connect(addr).is_err() {
        return false;
    }
    socket.send(&[0u8]).is_ok()
}

pub(crate) fn vpn_endpoint_reachable(protocol: VpnProtocol, host: &str, port: u16, timeout_ms: u64) -> bool {
    match protocol {
        VpnProtocol::Wireguard | VpnProtocol::Amnezia => {
            udp_endpoint_reachable(host, port, timeout_ms)
        }
        VpnProtocol::Openvpn => {
            endpoint_reachable(host, port, timeout_ms)
                || udp_endpoint_reachable(host, port, timeout_ms)
        }
        _ => endpoint_reachable(host, port, timeout_ms),
    }
}

pub(crate) fn proxy_protocol_label(protocol: ProxyProtocol) -> &'static str {
    match protocol {
        ProxyProtocol::Http => "HTTP",
        ProxyProtocol::Socks4 => "SOCKS4",
        ProxyProtocol::Socks5 => "SOCKS5",
        ProxyProtocol::Shadowsocks => "SHADOWSOCKS",
        ProxyProtocol::Vmess => "VMESS",
        ProxyProtocol::Vless => "VLESS",
        ProxyProtocol::Trojan => "TROJAN",
    }
}

pub(crate) fn vpn_protocol_label(protocol: VpnProtocol) -> &'static str {
    match protocol {
        VpnProtocol::Wireguard => "WIREGUARD",
        VpnProtocol::Openvpn => "OPENVPN",
        VpnProtocol::Amnezia => "AMNEZIA",
        VpnProtocol::Vmess => "VMESS",
        VpnProtocol::Vless => "VLESS",
        VpnProtocol::Trojan => "TROJAN",
        VpnProtocol::Shadowsocks => "SHADOWSOCKS",
    }
}

pub(crate) fn dns_block_reason(state: &AppState, profile_id: Uuid, host: &str) -> Option<String> {
    let store = state.network_store.lock().ok()?;
    let dns = store.dns.get(&profile_id.to_string())?;
    for domain in &dns.domain_denylist {
        if host_matches(host, domain) {
            return Some("Profile domain denylist".to_string());
        }
    }
    for (_, service) in &dns.selected_services {
        if service_matches_host(host, service) {
            return Some(format!("Blocked service: {service}"));
        }
    }
    for list in &dns.selected_blocklists {
        for domain in &list.domains {
            if host_matches(host, domain) {
                return Some(format!("DNS blocklist: {}", list.list_id));
            }
        }
    }
    None
}

pub(crate) fn service_matches_host(host: &str, service: &str) -> bool {
    let normalized_host = normalize_domain(host);
    if normalized_host.is_empty() {
        return false;
    }
    service_domain_seeds(service)
        .into_iter()
        .any(|domain| host_matches(&normalized_host, &domain))
}

pub(crate) fn global_security_block_reason(state: &AppState, host: &str) -> Option<String> {
    let record = load_global_security_record(state).ok()?;
    for suffix in &record.blocked_domain_suffixes {
        if host_matches(host, suffix) {
            return Some("Global suffix blacklist".to_string());
        }
    }
    for item in record.blocklists {
        if !item.active {
            continue;
        }
        let list_name = item.name;
        for domain in item.domains {
            if host_matches(host, &domain) {
                return Some(format!("Global blocklist: {list_name}"));
            }
        }
    }
    None
}

#[allow(dead_code)]
pub(crate) fn current_route_policy(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Option<VpnProxyTabPayload> {
    let state = app_handle.state::<AppState>();
    let store = state.network_store.lock().ok()?;
    store.vpn_proxy.get(&profile_id.to_string()).cloned()
}

pub(crate) fn current_route_mode(
    app_handle: &AppHandle,
    profile_id: Uuid,
    policy: Option<&VpnProxyTabPayload>,
) -> String {
    let state = app_handle.state::<AppState>();
    if profile_route_mode(&state, profile_id) == "direct" {
        return "direct".to_string();
    }
    let global_vpn_enabled = state
        .network_store
        .lock()
        .ok()
        .map(|store| store.global_route_settings.global_vpn_enabled)
        .unwrap_or(false);
    let base_route = if global_vpn_enabled {
        "vpn".to_string()
    } else {
        policy
            .map(|value| value.route_mode.clone())
            .unwrap_or_else(|| "direct".to_string())
    };
    match resolved_route_strategy(app_handle, profile_id) {
        Some(ResolvedNetworkSandboxMode::CompatibilityNative) => {
            format!("{base_route}:compatibility-native")
        }
        Some(ResolvedNetworkSandboxMode::Container) => format!("{base_route}:container"),
        Some(ResolvedNetworkSandboxMode::Blocked) => format!("{base_route}:blocked"),
        _ => base_route,
    }
}

pub(crate) fn resolved_route_strategy(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Option<ResolvedNetworkSandboxMode> {
    let state = app_handle.state::<AppState>();
    let template = selected_route_template(state.inner(), profile_id)?;
    resolve_profile_network_sandbox_mode(state.inner(), profile_id, Some(&template))
        .ok()
        .map(|value| value.mode)
}

pub(crate) fn resolve_sandbox_strategy_reason(state: &AppState, profile_id: Uuid) -> Option<String> {
    let template = selected_route_template(state, profile_id)?;
    resolve_profile_network_sandbox_mode(state, profile_id, Some(&template))
        .ok()
        .map(|value| value.reason)
}

pub(crate) fn resolve_sandbox_adapter_reason(state: &AppState, profile_id: Uuid) -> Option<String> {
    resolve_profile_network_sandbox_view(state, profile_id)
        .ok()
        .map(|value| value.adapter.reason)
}

pub(crate) fn resolved_sandbox_adapter_available(app_handle: &AppHandle, profile_id: Uuid) -> bool {
    let state = app_handle.state::<AppState>();
    resolve_profile_network_sandbox_view(state.inner(), profile_id)
        .ok()
        .map(|value| value.adapter.available)
        .unwrap_or(false)
}


