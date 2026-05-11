use crate::network_sandbox::{
    migrate_network_sandbox_store, normalize_mode, NetworkSandboxGlobalSettings,
    NetworkSandboxProfileSettings, NetworkSandboxStore, ResolvedNetworkSandboxMode, MODE_AUTO,
    MODE_COMPAT_NATIVE, MODE_CONTAINER, MODE_ISOLATED,
};
use super::resolution::{
    resolve_network_sandbox_strategy_for_modes, template_supports_container_mode,
};
use crate::state::{ConnectionNode, ConnectionTemplate, NetworkStore};
use browser_network_policy::VpnProxyTabPayload;
use std::collections::BTreeMap;

fn global_settings() -> NetworkSandboxGlobalSettings {
    NetworkSandboxGlobalSettings::default()
}

fn amnezia_template() -> ConnectionTemplate {
    ConnectionTemplate {
        id: "tpl-amnezia".to_string(),
        name: "Amnezia".to_string(),
        nodes: vec![ConnectionNode {
            id: "node-1".to_string(),
            connection_type: "vpn".to_string(),
            protocol: "amnezia".to_string(),
            host: None,
            port: None,
            username: None,
            password: None,
            bridges: None,
            settings: BTreeMap::from([(
                "amneziaKey".to_string(),
                "[Interface]\nAddress = 10.8.1.84/32\nPrivateKey = PRIVATE\nJc = 4\nJmin = 10\nJmax = 50\n\n[Peer]\nPublicKey = PUBLIC\nAllowedIPs = 0.0.0.0/0\nEndpoint = 5.129.225.48:32542\n".to_string(),
            )]),
        }],
        connection_type: String::new(),
        protocol: String::new(),
        host: None,
        port: None,
        username: None,
        password: None,
        bridges: None,
        updated_at_epoch_ms: 1,
    }
}

#[test]
fn migrate_marks_legacy_native_amnezia_profile() {
    let mut network_store = NetworkStore::default();
    network_store.vpn_proxy.insert(
        "profile-1".to_string(),
        VpnProxyTabPayload {
            route_mode: "vpn".to_string(),
            proxy: None,
            vpn: None,
            kill_switch_enabled: true,
        },
    );
    network_store
        .profile_template_selection
        .insert("profile-1".to_string(), "tpl-amnezia".to_string());
    network_store
        .connection_templates
        .insert("tpl-amnezia".to_string(), amnezia_template());

    let mut sandbox_store = NetworkSandboxStore::default();
    let changed =
        migrate_network_sandbox_store(&mut sandbox_store, &network_store).expect("migrate");
    assert!(changed);
    let profile = sandbox_store.profiles.get("profile-1").expect("profile");
    assert_eq!(profile.preferred_mode.as_deref(), Some(MODE_COMPAT_NATIVE));
    assert!(profile.migrated_legacy_native);
}

#[test]
fn normalize_mode_rejects_empty_and_normalizes_unknown_to_auto() {
    assert_eq!(normalize_mode(Some(String::new())), None);
    assert_eq!(
        normalize_mode(Some("compatibility-native".to_string())).as_deref(),
        Some(MODE_COMPAT_NATIVE)
    );
    assert_eq!(
        normalize_mode(Some("unexpected".to_string())).as_deref(),
        Some(MODE_AUTO)
    );
}

#[test]
fn container_mode_supports_tor_bridge_variants() {
    let build = |protocol: &str| ConnectionTemplate {
        id: format!("tpl-{protocol}"),
        name: format!("TOR {protocol}"),
        nodes: vec![ConnectionNode {
            id: "node-1".to_string(),
            connection_type: "tor".to_string(),
            protocol: protocol.to_string(),
            host: None,
            port: None,
            username: None,
            password: None,
            bridges: Some("Bridge example".to_string()),
            settings: BTreeMap::new(),
        }],
        connection_type: String::new(),
        protocol: String::new(),
        host: None,
        port: None,
        username: None,
        password: None,
        bridges: None,
        updated_at_epoch_ms: 1,
    };

    assert!(template_supports_container_mode(&build("obfs4")).expect("obfs4"));
    assert!(template_supports_container_mode(&build("snowflake")).expect("snowflake"));
    assert!(template_supports_container_mode(&build("meek")).expect("meek"));
}

#[test]
fn traffic_isolation_prefers_userspace_for_non_native_routes() {
    let strategy = resolve_network_sandbox_strategy_for_modes(
        &global_settings(),
        &NetworkSandboxProfileSettings::default(),
        MODE_AUTO.to_string(),
        false,
        true,
    );

    assert_eq!(strategy.mode, ResolvedNetworkSandboxMode::IsolatedUserspace);
    assert!(strategy.available);
    assert_eq!(strategy.effective_mode(), MODE_ISOLATED);
    assert!(!strategy.requires_native_backend);
}

#[test]
fn traffic_isolation_blocks_native_routes_without_explicit_fallback() {
    let strategy = resolve_network_sandbox_strategy_for_modes(
        &global_settings(),
        &NetworkSandboxProfileSettings::default(),
        MODE_AUTO.to_string(),
        true,
        true,
    );

    assert_eq!(strategy.mode, ResolvedNetworkSandboxMode::Blocked);
    assert!(!strategy.available);
    assert!(strategy.requires_native_backend);
    assert!(strategy
        .reason
        .contains("requires a machine-wide compatibility backend"));
}

#[test]
fn traffic_isolation_allows_native_container_mode_when_requested() {
    let strategy = resolve_network_sandbox_strategy_for_modes(
        &global_settings(),
        &NetworkSandboxProfileSettings::default(),
        MODE_CONTAINER.to_string(),
        true,
        true,
    );

    assert_eq!(strategy.mode, ResolvedNetworkSandboxMode::Container);
    assert!(strategy.available);
    assert!(strategy.requires_native_backend);
    assert_eq!(strategy.effective_mode(), MODE_CONTAINER);
}

#[test]
fn traffic_isolation_uses_global_native_fallback_for_migrated_profiles() {
    let strategy = resolve_network_sandbox_strategy_for_modes(
        &NetworkSandboxGlobalSettings {
            enabled: true,
            allow_native_compatibility_fallback: true,
            ..NetworkSandboxGlobalSettings::default()
        },
        &NetworkSandboxProfileSettings::default(),
        MODE_AUTO.to_string(),
        true,
        true,
    );

    assert_eq!(
        strategy.mode,
        ResolvedNetworkSandboxMode::CompatibilityNative
    );
    assert!(strategy.available);
    assert!(strategy.requires_native_backend);
    assert!(strategy.reason.contains("Global sandbox policy allows"));
}

#[test]
fn traffic_isolation_blocks_unsupported_container_requests() {
    let strategy = resolve_network_sandbox_strategy_for_modes(
        &global_settings(),
        &NetworkSandboxProfileSettings::default(),
        MODE_CONTAINER.to_string(),
        true,
        false,
    );

    assert_eq!(strategy.mode, ResolvedNetworkSandboxMode::Blocked);
    assert!(!strategy.available);
    assert!(strategy
        .reason
        .contains("not compatible with container isolation"));
}
