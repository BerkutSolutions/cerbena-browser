use browser_api_local::{
    ApiRole, ApiSession, ConsentGrant, LocalApi, ProfileScopeGrant, RequestContext,
};
use browser_network_policy::{DnsConfig, DnsMode, NetworkPolicy, PolicyRequest, RouteMode};
use browser_profile::{CreateProfileInput, Engine, ProfileManager};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn local_api_enforces_profile_scope_grants() {
    let temp = tempdir().expect("tempdir");
    let manager = ProfileManager::new(temp.path()).expect("manager");
    let p1 = manager
        .create_profile(CreateProfileInput {
            name: "P1".to_string(),
            description: None,
            tags: vec![],
            engine: Engine::Wayfern,
            default_start_page: None,
            default_search_provider: None,
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .expect("p1");
    let p2 = manager
        .create_profile(CreateProfileInput {
            name: "P2".to_string(),
            description: None,
            tags: vec![],
            engine: Engine::Librewolf,
            default_start_page: None,
            default_search_provider: None,
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .expect("p2");

    let mut api = LocalApi::default();
    api.register_session(ApiSession {
        token: "token-1".to_string(),
        role: ApiRole::Operator,
        grants: vec![ProfileScopeGrant {
            profile_id: p1.id,
            allow_launch: true,
            allow_policy_eval: true,
        }],
        consent_grants: vec![ConsentGrant {
            profile_id: p1.id,
            operation: "profile.launch".to_string(),
            expires_at_unix_ms: u128::MAX,
        }],
    });

    let ids = api.list_profiles("token-1", &manager).expect("list");
    assert_eq!(ids, vec![p1.id]);

    let forbidden = api.launch_profile(&RequestContext {
        token: "token-1".to_string(),
        profile_id: p2.id,
    });
    assert!(forbidden.is_err());
}

#[test]
fn local_api_policy_eval_returns_reason_code() {
    let mut api = LocalApi::default();
    let profile_id = Uuid::new_v4();
    api.register_session(ApiSession {
        token: "token-2".to_string(),
        role: ApiRole::Operator,
        grants: vec![ProfileScopeGrant {
            profile_id,
            allow_launch: false,
            allow_policy_eval: true,
        }],
        consent_grants: vec![],
    });
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
        target_domain: "example.com".to_string(),
        target_service: None,
        tor_up: false,
        dns_over_tor: false,
        active_route: RouteMode::Vpn,
    };
    let reason = api
        .evaluate_policy(
            &RequestContext {
                token: "token-2".to_string(),
                profile_id,
            },
            &policy,
            &request,
        )
        .expect("evaluate");
    assert_eq!(reason, "vpn_required_down");
}

#[test]
fn local_api_blocks_when_consent_missing() {
    let profile_id = Uuid::new_v4();
    let mut api = LocalApi::default();
    api.register_session(ApiSession {
        token: "token-3".to_string(),
        role: ApiRole::Operator,
        grants: vec![ProfileScopeGrant {
            profile_id,
            allow_launch: true,
            allow_policy_eval: false,
        }],
        consent_grants: vec![],
    });
    let denied = api.launch_profile(&RequestContext {
        token: "token-3".to_string(),
        profile_id,
    });
    assert!(denied.is_err());
}

#[test]
fn local_api_denies_viewer_launch_by_rbac() {
    let profile_id = Uuid::new_v4();
    let mut api = LocalApi::default();
    api.register_session(ApiSession {
        token: "token-4".to_string(),
        role: ApiRole::Viewer,
        grants: vec![ProfileScopeGrant {
            profile_id,
            allow_launch: true,
            allow_policy_eval: true,
        }],
        consent_grants: vec![ConsentGrant {
            profile_id,
            operation: "profile.launch".to_string(),
            expires_at_unix_ms: u128::MAX,
        }],
    });
    let denied = api.launch_profile(&RequestContext {
        token: "token-4".to_string(),
        profile_id,
    });
    assert!(denied.is_err());
}
