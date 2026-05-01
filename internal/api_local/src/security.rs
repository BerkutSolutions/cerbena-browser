use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiRole {
    Viewer,
    Operator,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentGrant {
    pub profile_id: Uuid,
    pub operation: String,
    pub expires_at_unix_ms: u128,
}

#[derive(Debug, Error)]
pub enum GuardrailError {
    #[error("rbac denied")]
    RbacDenied,
    #[error("rate limit exceeded")]
    RateLimitExceeded,
    #[error("consent missing")]
    ConsentMissing,
    #[error("scope escalation blocked")]
    ScopeEscalationBlocked,
}

#[derive(Debug, Clone)]
pub struct RateLimitPolicy {
    pub max_requests: u32,
    pub window: Duration,
}

impl Default for RateLimitPolicy {
    fn default() -> Self {
        Self {
            max_requests: 30,
            window: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Default)]
pub struct SecurityGuardrails {
    pub rate_policy: RateLimitPolicy,
    counters: BTreeMap<String, (u32, Instant)>,
}

impl SecurityGuardrails {
    pub fn enforce_rate_limit(&mut self, token: &str) -> Result<(), GuardrailError> {
        let now = Instant::now();
        let entry = self
            .counters
            .entry(token.to_string())
            .or_insert((0, now + self.rate_policy.window));
        if now > entry.1 {
            *entry = (0, now + self.rate_policy.window);
        }
        if entry.0 >= self.rate_policy.max_requests {
            return Err(GuardrailError::RateLimitExceeded);
        }
        entry.0 += 1;
        Ok(())
    }

    pub fn enforce_rbac(&self, role: ApiRole, operation: &str) -> Result<(), GuardrailError> {
        let allowed = match role {
            ApiRole::Viewer => operation == "profile.list",
            ApiRole::Operator => {
                operation == "profile.list"
                    || operation == "profile.launch"
                    || operation == "policy.evaluate"
            }
            ApiRole::Admin => true,
        };
        if allowed {
            Ok(())
        } else {
            Err(GuardrailError::RbacDenied)
        }
    }

    pub fn enforce_consent(
        &self,
        grant: Option<&ConsentGrant>,
        profile_id: Uuid,
        operation: &str,
        now_unix_ms: u128,
    ) -> Result<(), GuardrailError> {
        let Some(grant) = grant else {
            return Err(GuardrailError::ConsentMissing);
        };
        if grant.profile_id != profile_id || grant.operation != operation {
            return Err(GuardrailError::ConsentMissing);
        }
        if now_unix_ms > grant.expires_at_unix_ms {
            return Err(GuardrailError::ConsentMissing);
        }
        Ok(())
    }

    pub fn enforce_no_scope_escalation(
        &self,
        requested_profile_id: Uuid,
        granted_profiles: &[Uuid],
    ) -> Result<(), GuardrailError> {
        if granted_profiles.contains(&requested_profile_id) {
            Ok(())
        } else {
            Err(GuardrailError::ScopeEscalationBlocked)
        }
    }
}
