use browser_api_local::{ApiRole, ApiSession, ConsentGrant, LocalApi, ProfileScopeGrant};
use browser_api_mcp::{McpServer, McpToolRequest};
use browser_network_policy::{DnsConfig, DnsMode, NetworkPolicy, RouteMode};
use uuid::Uuid;

#[test]
fn mcp_server_executes_scoped_tools() {
    let profile_id = Uuid::new_v4();
    let mut api = LocalApi::default();
    api.register_session(ApiSession {
        token: "mcp-token".to_string(),
        role: ApiRole::Operator,
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
    let mut mcp = McpServer::default();
    let launch = mcp
        .execute_tool(
            &mut api,
            McpToolRequest {
                tool_name: "profile.launch".to_string(),
                token: "mcp-token".to_string(),
                profile_id,
                target_domain: None,
            },
            None,
        )
        .expect("launch");
    assert!(launch.ok);

    let policy = NetworkPolicy {
        deny_if_context_missing: true,
        kill_switch_enabled: false,
        vpn_required: false,
        route_mode: RouteMode::Proxy,
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
    let eval = mcp
        .execute_tool(
            &mut api,
            McpToolRequest {
                tool_name: "policy.evaluate".to_string(),
                token: "mcp-token".to_string(),
                profile_id,
                target_domain: Some("example.org".to_string()),
            },
            Some(&policy),
        )
        .expect("policy eval");
    assert!(eval.ok);
}
