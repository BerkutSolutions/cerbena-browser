use browser_network_policy::{
    dns::{DnsConfig, DnsMode, DnsResolverAdapter},
    dns_tab::{validate_dns_tab, DnsTabPayload},
    policy::{
        DecisionAction, DomainRule, NetworkPolicy, NetworkPolicyEngine, PolicyRequest,
        RouteConstraint, RouteMode, ServiceRule,
    },
    proxy::{ProxyProtocol, ProxyTransportAdapter},
    service_catalog::ServiceCatalog,
    tor::TorRouteGuard,
    updater::{BlocklistSource, DnsBlocklistUpdater, DnsListSnapshot},
    vpn::{VpnProtocol, VpnTunnelAdapter},
    vpn_proxy_tab::{test_connect, validate_vpn_proxy_tab, VpnProxyTabPayload},
};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::TcpListener,
    thread,
};
use tempfile::tempdir;

#[test]
fn policy_engine_enforces_kill_switch_first() {
    let engine = NetworkPolicyEngine;
    let policy = NetworkPolicy {
        deny_if_context_missing: true,
        kill_switch_enabled: true,
        vpn_required: true,
        tor_required: false,
        route_mode: RouteMode::Vpn,
        dns_config: DnsConfig {
            mode: DnsMode::System,
            servers: vec![],
            doh_url: None,
            dot_server_name: None,
        },
        domain_rules: vec![DomainRule {
            pattern: "example.com".to_string(),
            allow: true,
            route_constraint: None,
        }],
        service_rules: vec![],
        exceptions: vec![],
    };
    let decision = engine
        .evaluate(
            &policy,
            &PolicyRequest {
                has_profile_context: true,
                vpn_up: false,
                target_domain: "example.com".to_string(),
                target_service: None,
                tor_up: true,
                dns_over_tor: true,
                active_route: RouteMode::Vpn,
            },
        )
        .expect("evaluate");
    assert_eq!(decision.action, DecisionAction::Deny);
    assert_eq!(decision.reason_code, "vpn_required_down");
}

#[test]
fn policy_engine_uses_domain_rules_when_no_hard_constraints() {
    let engine = NetworkPolicyEngine;
    let policy = NetworkPolicy {
        deny_if_context_missing: true,
        kill_switch_enabled: false,
        vpn_required: false,
        tor_required: false,
        route_mode: RouteMode::Direct,
        dns_config: DnsConfig {
            mode: DnsMode::Custom,
            servers: vec!["1.1.9.1".to_string()],
            doh_url: None,
            dot_server_name: None,
        },
        domain_rules: vec![DomainRule {
            pattern: "blocked.example".to_string(),
            allow: false,
            route_constraint: None,
        }],
        service_rules: vec![ServiceRule {
            service: "chatgpt".to_string(),
            allow: true,
        }],
        exceptions: vec![],
    };
    let decision = engine
        .evaluate(
            &policy,
            &PolicyRequest {
                has_profile_context: true,
                vpn_up: true,
                target_domain: "blocked.example".to_string(),
                target_service: Some("chatgpt".to_string()),
                tor_up: true,
                dns_over_tor: true,
                active_route: RouteMode::Direct,
            },
        )
        .expect("evaluate");
    assert_eq!(decision.action, DecisionAction::Deny);
    assert_eq!(decision.reason_code, "domain_deny");
}

#[test]
fn proxy_adapter_validation_rejects_bad_input() {
    let proxy = ProxyTransportAdapter {
        protocol: ProxyProtocol::Socks5,
        host: "".to_string(),
        port: 1080,
        username: None,
        password: None,
    };
    assert!(proxy.validate().is_err());
}

#[test]
fn vpn_adapter_validation_and_health_check_shape() {
    let vpn = VpnTunnelAdapter {
        protocol: VpnProtocol::Wireguard,
        endpoint_host: "127.0.0.1".to_string(),
        endpoint_port: 51820,
        profile_ref: Some("wg-main".to_string()),
    };
    assert!(vpn.validate().is_ok());
    let health = vpn.health_check(10).expect("health");
    assert!(!health.message.is_empty());
}

#[test]
fn tor_guard_blocks_dns_leak_risk() {
    let guard = TorRouteGuard {
        tor_required: true,
        tor_up: true,
        dns_over_tor: false,
    };
    assert!(guard.evaluate().is_err());
}

#[test]
fn dns_adapter_validates_custom_servers() {
    let dns = DnsResolverAdapter {
        profile_id: "p1".to_string(),
        config: DnsConfig {
            mode: DnsMode::Custom,
            servers: vec!["1.1.9.1".to_string(), "8.8.8.8".to_string()],
            doh_url: Some("https://dns.example/dns-query".to_string()),
            dot_server_name: None,
        },
    };
    let resolvers = dns.effective_resolvers().expect("resolvers");
    assert_eq!(resolvers.len(), 2);
}

#[test]
fn domain_route_constraint_is_enforced() {
    let engine = NetworkPolicyEngine;
    let policy = NetworkPolicy {
        deny_if_context_missing: false,
        kill_switch_enabled: false,
        vpn_required: false,
        route_mode: RouteMode::Vpn,
        dns_config: DnsConfig {
            mode: DnsMode::System,
            servers: vec![],
            doh_url: None,
            dot_server_name: None,
        },
        tor_required: false,
        domain_rules: vec![DomainRule {
            pattern: "vpn-only.test".to_string(),
            allow: true,
            route_constraint: Some(RouteConstraint::OnlyVpn),
        }],
        service_rules: vec![],
        exceptions: vec![],
    };
    let decision = engine
        .evaluate(
            &policy,
            &PolicyRequest {
                has_profile_context: true,
                vpn_up: true,
                target_domain: "vpn-only.test".to_string(),
                target_service: None,
                tor_up: false,
                dns_over_tor: false,
                active_route: RouteMode::Proxy,
            },
        )
        .expect("evaluate");
    assert_eq!(decision.action, DecisionAction::Deny);
    assert_eq!(decision.reason_code, "route_constraint_mismatch");
}

#[test]
fn blocklist_updater_refreshes_every_12h() {
    let updater = DnsBlocklistUpdater::new();
    let snap = DnsListSnapshot {
        list_id: "main".to_string(),
        domains: vec!["ads.test".to_string()],
        updated_at_epoch: 1000,
    };
    assert!(!updater.should_refresh(&snap, 1000 + 3600));
    assert!(updater.should_refresh(&snap, 1000 + 12 * 3600));
}

#[test]
fn blocklist_updater_reads_local_source() {
    let tmp = tempdir().expect("tempdir");
    let file = tmp.path().join("list.txt");
    fs::write(&file, "# comment\nads.test\ntracker.test\n").expect("write");
    let updater = DnsBlocklistUpdater::new();
    let snap = updater
        .update_from_source("local", &BlocklistSource::LocalFile { path: file })
        .expect("update");
    assert_eq!(snap.domains.len(), 2);
}

#[test]
fn blocklist_updater_reads_remote_hosts_source() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0u8; 1024];
        let _ = stream.read(&mut buffer);
        let body = "0.0.0.0 ads.test\n127.0.0.1 tracker.test\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).expect("write");
    });

    let updater = DnsBlocklistUpdater::new();
    let snap = updater
        .update_from_source(
            "remote",
            &BlocklistSource::RemoteUrl {
                url: format!("http://{addr}/list.txt"),
                require_https: false,
                expected_sha256: None,
            },
        )
        .expect("update");
    assert_eq!(
        snap.domains,
        vec!["ads.test".to_string(), "tracker.test".to_string()]
    );
}

#[test]
fn blocklist_updater_rejects_plain_http_when_https_required() {
    let updater = DnsBlocklistUpdater::new();
    let error = updater
        .update_from_source(
            "remote",
            &BlocklistSource::RemoteUrl {
                url: "http://example.test/list.txt".to_string(),
                require_https: true,
                expected_sha256: None,
            },
        )
        .expect_err("http source must be rejected");
    assert!(error.to_string().contains("https"));
}

#[test]
fn blocklist_updater_validates_remote_checksum_for_curated_sources() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0u8; 1024];
        let _ = stream.read(&mut buffer);
        let body = "0.0.0.0 ads.test\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).expect("write");
    });

    let updater = DnsBlocklistUpdater::new();
    let error = updater
        .update_from_source(
            "remote",
            &BlocklistSource::RemoteUrl {
                url: format!("http://{addr}/list.txt"),
                require_https: false,
                expected_sha256: Some("deadbeef".to_string()),
            },
        )
        .expect_err("checksum mismatch must fail");
    assert!(error.to_string().contains("checksum mismatch"));
}

#[test]
fn service_catalog_block_all_and_exception() {
    let mut seed = BTreeMap::new();
    seed.insert(
        "social".to_string(),
        vec!["reddit".to_string(), "x".to_string()],
    );
    let mut catalog = ServiceCatalog::from_seed(seed);
    catalog
        .set_category_block_all("social", true)
        .expect("block all");
    assert!(!catalog.is_allowed("social", "reddit"));
    catalog.add_exception("reddit");
    assert!(catalog.is_allowed("social", "reddit"));
}

#[test]
fn vpn_proxy_tab_validation_and_test_connect() {
    let payload = VpnProxyTabPayload {
        route_mode: "proxy".to_string(),
        proxy: Some(ProxyTransportAdapter {
            protocol: ProxyProtocol::Socks5,
            host: "127.0.0.1".to_string(),
            port: 9050,
            username: None,
            password: None,
        }),
        vpn: None,
        kill_switch_enabled: false,
    };
    validate_vpn_proxy_tab(&payload).expect("valid");
    let (proxy_health, vpn_health) = test_connect(&payload, 10).expect("connect");
    assert!(proxy_health.is_some());
    assert!(vpn_health.is_none());
}

#[test]
fn dns_tab_validation_checks_blocklist_and_catalog() {
    let mut seed = BTreeMap::new();
    seed.insert("ai".to_string(), vec!["chatgpt".to_string()]);
    let catalog = ServiceCatalog::from_seed(seed);

    let payload = DnsTabPayload {
        profile_id: "p1".to_string(),
        dns_config: DnsConfig {
            mode: DnsMode::Custom,
            servers: vec!["1.1.9.1".to_string()],
            doh_url: None,
            dot_server_name: None,
        },
        selected_blocklists: vec![DnsListSnapshot {
            list_id: "l1".to_string(),
            domains: vec!["ads.test".to_string()],
            updated_at_epoch: 0,
        }],
        selected_services: vec![("ai".to_string(), "chatgpt".to_string())],
        domain_allowlist: vec!["example.com".to_string()],
        domain_denylist: vec!["tracker.example".to_string()],
        domain_exceptions: vec![],
    };

    validate_dns_tab(&payload, Some(&catalog)).expect("dns tab valid");
}
