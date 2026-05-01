use browser_api_local::{LocalApi, LocalApiError, RequestContext};
use browser_network_policy::{NetworkPolicy, PolicyRequest};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolRequest {
    pub tool_name: String,
    pub token: String,
    pub profile_id: Uuid,
    pub target_domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResponse {
    pub ok: bool,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum McpServerError {
    #[error("api error: {0}")]
    Api(#[from] LocalApiError),
    #[error("unsupported tool: {0}")]
    UnsupportedTool(String),
}

#[derive(Debug, Default)]
pub struct McpServer {
    pub audit: Vec<String>,
}

impl McpServer {
    pub fn execute_tool(
        &mut self,
        api: &mut LocalApi,
        request: McpToolRequest,
        policy: Option<&NetworkPolicy>,
    ) -> Result<McpToolResponse, McpServerError> {
        let ctx = RequestContext {
            token: request.token.clone(),
            profile_id: request.profile_id,
        };
        let response = match request.tool_name.as_str() {
            "profile.launch" => {
                api.launch_profile(&ctx)?;
                McpToolResponse {
                    ok: true,
                    reason: "launch_accepted".to_string(),
                }
            }
            "policy.evaluate" => {
                let p = policy
                    .ok_or_else(|| McpServerError::UnsupportedTool("policy missing".to_string()))?;
                let reason = api.evaluate_policy(
                    &ctx,
                    p,
                    &PolicyRequest {
                        has_profile_context: true,
                        vpn_up: true,
                        target_domain: request
                            .target_domain
                            .unwrap_or_else(|| "example.com".to_string()),
                        target_service: None,
                        tor_up: true,
                        dns_over_tor: true,
                        active_route: p.route_mode,
                    },
                )?;
                McpToolResponse { ok: true, reason }
            }
            _ => return Err(McpServerError::UnsupportedTool(request.tool_name)),
        };
        self.audit.push(format!(
            "mcp.tool={} profile={}",
            request.tool_name, request.profile_id
        ));
        Ok(response)
    }
}
