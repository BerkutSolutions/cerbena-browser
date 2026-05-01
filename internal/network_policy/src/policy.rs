use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::dns::{DnsConfig, DnsMode};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteMode {
    Direct,
    Proxy,
    Vpn,
    Tor,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainRule {
    pub pattern: String,
    pub allow: bool,
    pub route_constraint: Option<RouteConstraint>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteConstraint {
    OnlyTor,
    OnlyVpn,
    OnlyProxy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceRule {
    pub service: String,
    pub allow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    pub deny_if_context_missing: bool,
    pub kill_switch_enabled: bool,
    pub vpn_required: bool,
    pub route_mode: RouteMode,
    pub dns_config: DnsConfig,
    pub tor_required: bool,
    pub domain_rules: Vec<DomainRule>,
    pub service_rules: Vec<ServiceRule>,
    pub exceptions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PolicyRequest {
    pub has_profile_context: bool,
    pub vpn_up: bool,
    pub target_domain: String,
    pub target_service: Option<String>,
    pub tor_up: bool,
    pub dns_over_tor: bool,
    pub active_route: RouteMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAction {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub action: DecisionAction,
    pub selected_route: RouteMode,
    pub effective_dns: String,
    pub matched_rules: Vec<String>,
    pub reason_code: String,
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("invalid policy: {0}")]
    InvalidPolicy(String),
}

#[derive(Debug, Default, Clone)]
pub struct NetworkPolicyEngine;

impl NetworkPolicyEngine {
    pub fn evaluate(
        &self,
        policy: &NetworkPolicy,
        request: &PolicyRequest,
    ) -> Result<Decision, PolicyError> {
        if policy.deny_if_context_missing && !request.has_profile_context {
            return Ok(Decision {
                action: DecisionAction::Deny,
                selected_route: policy.route_mode,
                effective_dns: dns_label(&policy.dns_config),
                matched_rules: vec!["hard_constraint.context_missing".to_string()],
                reason_code: "context_missing".to_string(),
            });
        }
        if policy.kill_switch_enabled && policy.vpn_required && !request.vpn_up {
            return Ok(Decision {
                action: DecisionAction::Deny,
                selected_route: policy.route_mode,
                effective_dns: dns_label(&policy.dns_config),
                matched_rules: vec!["hard_constraint.vpn_required_down".to_string()],
                reason_code: "vpn_required_down".to_string(),
            });
        }
        if policy.tor_required && !request.tor_up {
            return Ok(Decision {
                action: DecisionAction::Deny,
                selected_route: policy.route_mode,
                effective_dns: dns_label(&policy.dns_config),
                matched_rules: vec!["hard_constraint.tor_required_down".to_string()],
                reason_code: "tor_required_down".to_string(),
            });
        }
        if policy.tor_required && !request.dns_over_tor {
            return Ok(Decision {
                action: DecisionAction::Deny,
                selected_route: policy.route_mode,
                effective_dns: dns_label(&policy.dns_config),
                matched_rules: vec!["hard_constraint.tor_dns_leak_risk".to_string()],
                reason_code: "tor_dns_leak_risk".to_string(),
            });
        }

        let mut matched = Vec::new();
        let domain = request.target_domain.to_lowercase();

        if policy
            .exceptions
            .iter()
            .any(|v| v.eq_ignore_ascii_case(&domain))
        {
            return Ok(Decision {
                action: DecisionAction::Allow,
                selected_route: policy.route_mode,
                effective_dns: dns_label(&policy.dns_config),
                matched_rules: vec!["exception.domain".to_string()],
                reason_code: "exception_match".to_string(),
            });
        }

        for rule in &policy.domain_rules {
            if domain_contains(&domain, &rule.pattern) {
                matched.push(format!("domain_rule:{}", rule.pattern));
                if let Some(constraint) = rule.route_constraint {
                    if !route_satisfies_constraint(request.active_route, constraint) {
                        return Ok(Decision {
                            action: DecisionAction::Deny,
                            selected_route: policy.route_mode,
                            effective_dns: dns_label(&policy.dns_config),
                            matched_rules: vec![format!(
                                "route_constraint_mismatch:{:?}",
                                constraint
                            )],
                            reason_code: "route_constraint_mismatch".to_string(),
                        });
                    }
                }
                return Ok(Decision {
                    action: if rule.allow {
                        DecisionAction::Allow
                    } else {
                        DecisionAction::Deny
                    },
                    selected_route: policy.route_mode,
                    effective_dns: dns_label(&policy.dns_config),
                    matched_rules: matched,
                    reason_code: if rule.allow {
                        "domain_allow".to_string()
                    } else {
                        "domain_deny".to_string()
                    },
                });
            }
        }

        if let Some(service) = &request.target_service {
            for rule in &policy.service_rules {
                if rule.service.eq_ignore_ascii_case(service) {
                    matched.push(format!("service_rule:{}", rule.service));
                    return Ok(Decision {
                        action: if rule.allow {
                            DecisionAction::Allow
                        } else {
                            DecisionAction::Deny
                        },
                        selected_route: policy.route_mode,
                        effective_dns: dns_label(&policy.dns_config),
                        matched_rules: matched,
                        reason_code: if rule.allow {
                            "service_allow".to_string()
                        } else {
                            "service_deny".to_string()
                        },
                    });
                }
            }
        }

        Ok(Decision {
            action: DecisionAction::Allow,
            selected_route: policy.route_mode,
            effective_dns: dns_label(&policy.dns_config),
            matched_rules: vec!["default_allow".to_string()],
            reason_code: "default_allow".to_string(),
        })
    }
}

fn domain_contains(domain: &str, pattern: &str) -> bool {
    if pattern.eq_ignore_ascii_case(domain) {
        return true;
    }
    domain.ends_with(&format!(".{}", pattern.to_lowercase()))
}

fn route_satisfies_constraint(active: RouteMode, constraint: RouteConstraint) -> bool {
    match constraint {
        RouteConstraint::OnlyTor => matches!(active, RouteMode::Tor),
        RouteConstraint::OnlyVpn => matches!(active, RouteMode::Vpn),
        RouteConstraint::OnlyProxy => matches!(active, RouteMode::Proxy),
    }
}

fn dns_label(config: &DnsConfig) -> String {
    match config.mode {
        DnsMode::System => "system".to_string(),
        DnsMode::Custom => config.servers.join(","),
    }
}
