use serde::{Deserialize, Serialize};

use browser_network_policy::{NetworkPolicy, NetworkPolicyEngine, PolicyRequest, RouteMode};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionPolicyDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideGuardrails {
    pub require_explicit_allow: bool,
    pub allow_service_override: bool,
}

#[derive(Debug, Default, Clone)]
pub struct ExtensionPolicyEnforcer {
    engine: NetworkPolicyEngine,
}

impl ExtensionPolicyEnforcer {
    pub fn evaluate(
        &self,
        policy: &NetworkPolicy,
        request: &PolicyRequest,
        extension_override_allowed: bool,
        guardrails: &OverrideGuardrails,
    ) -> (ExtensionPolicyDecision, String) {
        if extension_override_allowed && !guardrails.require_explicit_allow {
            return (
                ExtensionPolicyDecision::Deny,
                "guardrail_violation.missing_explicit_allow".to_string(),
            );
        }

        if !guardrails.allow_service_override && request.target_service.is_some() {
            return (
                ExtensionPolicyDecision::Deny,
                "guardrail_violation.service_override_forbidden".to_string(),
            );
        }

        let route = request.active_route;
        if extension_override_allowed && matches!(route, RouteMode::Direct) {
            return (
                ExtensionPolicyDecision::Deny,
                "guardrail_violation.direct_route_override_forbidden".to_string(),
            );
        }

        match self.engine.evaluate(policy, request) {
            Ok(decision) if decision.action == browser_network_policy::DecisionAction::Allow => {
                (ExtensionPolicyDecision::Allow, decision.reason_code)
            }
            Ok(decision) => (ExtensionPolicyDecision::Deny, decision.reason_code),
            Err(err) => (
                ExtensionPolicyDecision::Deny,
                format!("policy_error:{}", err),
            ),
        }
    }
}
