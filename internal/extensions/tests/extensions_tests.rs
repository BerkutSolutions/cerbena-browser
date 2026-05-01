use browser_extensions::{
    ExtensionImportState, ExtensionManager, ExtensionStatus, ExtensionUpdatePolicy,
    FirstLaunchInstaller, ImportSource, ImportSourceKind, OverrideGuardrails, SourceValidator,
};
use browser_network_policy::{DnsConfig, DnsMode, NetworkPolicy, PolicyRequest, RouteMode};
use uuid::Uuid;

#[test]
fn source_validator_accepts_supported_sources() {
    let validator = SourceValidator;
    validator
        .validate(&ImportSource {
            kind: ImportSourceKind::LocalFolder,
            value: "C:\\ext\\folder".to_string(),
        })
        .expect("folder source");
    validator
        .validate(&ImportSource {
            kind: ImportSourceKind::LocalArchive,
            value: "C:\\ext\\bundle.zip".to_string(),
        })
        .expect("archive source");
    validator
        .validate(&ImportSource {
            kind: ImportSourceKind::Url,
            value: "https://addons.mozilla.org/en-US/firefox/addon/ublock-origin/".to_string(),
        })
        .expect("url source");
}

#[test]
fn extension_manager_is_profile_scoped() {
    let mut manager = ExtensionManager::default();
    let p1 = Uuid::new_v4();
    let p2 = Uuid::new_v4();
    manager.create_profile_state(p1);
    manager.create_profile_state(p2);

    manager
        .install(
            p1,
            "ext-a",
            "Ext A",
            "1.0.2",
            ImportSource {
                kind: ImportSourceKind::LocalArchive,
                value: "C:\\ext\\a.crx".to_string(),
            },
            "profiles/p1/extensions/ext-a",
            ExtensionUpdatePolicy::ManualOnly,
        )
        .expect("install ext-a");

    assert_eq!(manager.profile_state(p1).expect("p1").extensions.len(), 1);
    assert_eq!(manager.profile_state(p2).expect("p2").extensions.len(), 0);
}

#[test]
fn first_launch_installer_handles_retry_and_failure() {
    let mut manager = ExtensionManager::default();
    let profile = Uuid::new_v4();
    manager.create_profile_state(profile);
    manager
        .install(
            profile,
            "ext-b",
            "Ext B",
            "2.1.0",
            ImportSource {
                kind: ImportSourceKind::LocalArchive,
                value: "C:\\ext\\b.xpi".to_string(),
            },
            "profiles/p/extensions/ext-b",
            ExtensionUpdatePolicy::FollowSource,
        )
        .expect("install ext-b");

    let installer = FirstLaunchInstaller { max_attempts: 2 };
    let state = manager.profile_state(profile).expect("state").clone();
    let mut mutable = state;

    installer.process(
        &mut mutable,
        &[browser_extensions::ExtensionInstallResult {
            extension_id: "ext-b".to_string(),
            installed: false,
            details: Some("network timeout".to_string()),
        }],
    );
    assert_eq!(
        mutable.extensions[0].status,
        ExtensionStatus::PendingFirstLaunchInstall
    );

    installer.process(
        &mut mutable,
        &[browser_extensions::ExtensionInstallResult {
            extension_id: "ext-b".to_string(),
            installed: false,
            details: Some("signature mismatch".to_string()),
        }],
    );
    assert_eq!(mutable.extensions[0].status, ExtensionStatus::Failed);
    assert_eq!(
        mutable.extensions[0].import_state,
        ExtensionImportState::Failed
    );
}

#[test]
fn extension_policy_enforcer_blocks_on_network_denial() {
    let enforcer = browser_extensions::ExtensionPolicyEnforcer::default();
    let policy = NetworkPolicy {
        deny_if_context_missing: true,
        kill_switch_enabled: true,
        vpn_required: true,
        route_mode: RouteMode::Vpn,
        dns_config: DnsConfig {
            mode: DnsMode::System,
            servers: vec![],
            doh_url: None,
            dot_server_name: None,
        },
        tor_required: false,
        domain_rules: vec![],
        service_rules: vec![],
        exceptions: vec![],
    };
    let request = PolicyRequest {
        has_profile_context: true,
        vpn_up: false,
        target_domain: "example.org".to_string(),
        target_service: None,
        tor_up: false,
        dns_over_tor: false,
        active_route: RouteMode::Vpn,
    };
    let (decision, code) = enforcer.evaluate(
        &policy,
        &request,
        false,
        &OverrideGuardrails {
            require_explicit_allow: true,
            allow_service_override: true,
        },
    );
    assert_eq!(decision, browser_extensions::ExtensionPolicyDecision::Deny);
    assert_eq!(code, "vpn_required_down");
}
