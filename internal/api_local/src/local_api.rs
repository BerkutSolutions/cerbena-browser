use std::collections::{BTreeMap, BTreeSet};

use browser_network_policy::{NetworkPolicy, NetworkPolicyEngine, PolicyRequest};
use browser_profile::ProfileManager;
use thiserror::Error;
use uuid::Uuid;

use crate::security::{ApiRole, ConsentGrant, GuardrailError, SecurityGuardrails};

#[derive(Debug, Clone)]
pub struct ApiSession {
    pub token: String,
    pub role: ApiRole,
    pub grants: Vec<ProfileScopeGrant>,
    pub consent_grants: Vec<ConsentGrant>,
}

#[derive(Debug, Clone)]
pub struct ProfileScopeGrant {
    pub profile_id: Uuid,
    pub allow_launch: bool,
    pub allow_policy_eval: bool,
}

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub token: String,
    pub profile_id: Uuid,
}

#[derive(Debug, Error)]
pub enum LocalApiError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("profile error: {0}")]
    Profile(String),
    #[error("guardrails: {0}")]
    Guardrails(#[from] GuardrailError),
}

#[derive(Debug)]
pub struct LocalApi {
    sessions: BTreeMap<String, BTreeMap<Uuid, ProfileScopeGrant>>,
    roles: BTreeMap<String, ApiRole>,
    consent: BTreeMap<String, Vec<ConsentGrant>>,
    audit: Vec<String>,
    network_engine: NetworkPolicyEngine,
    guardrails: SecurityGuardrails,
}

impl LocalApi {
    pub fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
            roles: BTreeMap::new(),
            consent: BTreeMap::new(),
            audit: Vec::new(),
            network_engine: NetworkPolicyEngine,
            guardrails: SecurityGuardrails::default(),
        }
    }

    pub fn register_session(&mut self, session: ApiSession) {
        self.roles.insert(session.token.clone(), session.role);
        self.consent
            .insert(session.token.clone(), session.consent_grants);
        let mut map = BTreeMap::new();
        for grant in session.grants {
            map.insert(grant.profile_id, grant);
        }
        self.sessions.insert(session.token, map);
    }

    pub fn list_profiles(
        &mut self,
        token: &str,
        manager: &ProfileManager,
    ) -> Result<Vec<Uuid>, LocalApiError> {
        self.guardrails.enforce_rate_limit(token)?;
        let role = *self.roles.get(token).ok_or(LocalApiError::Unauthorized)?;
        self.guardrails.enforce_rbac(role, "profile.list")?;
        if !self.sessions.contains_key(token) {
            return Err(LocalApiError::Unauthorized);
        }
        let allowed = self
            .sessions
            .get(token)
            .ok_or(LocalApiError::Unauthorized)?
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut ids = Vec::new();
        for profile in manager
            .list_profiles()
            .map_err(|e| LocalApiError::Profile(e.to_string()))?
        {
            if allowed.contains(&profile.id) {
                ids.push(profile.id);
            }
        }
        self.audit
            .push(format!("api.list_profiles token={}", token));
        Ok(ids)
    }

    pub fn launch_profile(&mut self, ctx: &RequestContext) -> Result<(), LocalApiError> {
        self.guardrails.enforce_rate_limit(&ctx.token)?;
        let role = *self
            .roles
            .get(&ctx.token)
            .ok_or(LocalApiError::Unauthorized)?;
        self.guardrails.enforce_rbac(role, "profile.launch")?;
        let grant = self.grant(ctx)?;
        self.guardrails
            .enforce_no_scope_escalation(ctx.profile_id, &self.granted_profile_ids(&ctx.token))?;
        self.guardrails.enforce_consent(
            self.find_consent(&ctx.token, ctx.profile_id, "profile.launch"),
            ctx.profile_id,
            "profile.launch",
            unix_ms_now(),
        )?;
        if !grant.allow_launch {
            return Err(LocalApiError::Forbidden);
        }
        self.audit
            .push(format!("api.launch_profile profile={}", ctx.profile_id));
        Ok(())
    }

    pub fn evaluate_policy(
        &mut self,
        ctx: &RequestContext,
        policy: &NetworkPolicy,
        request: &PolicyRequest,
    ) -> Result<String, LocalApiError> {
        self.guardrails.enforce_rate_limit(&ctx.token)?;
        let role = *self
            .roles
            .get(&ctx.token)
            .ok_or(LocalApiError::Unauthorized)?;
        self.guardrails.enforce_rbac(role, "policy.evaluate")?;
        let grant = self.grant(ctx)?;
        self.guardrails
            .enforce_no_scope_escalation(ctx.profile_id, &self.granted_profile_ids(&ctx.token))?;
        if !grant.allow_policy_eval {
            return Err(LocalApiError::Forbidden);
        }
        let decision = self
            .network_engine
            .evaluate(policy, request)
            .map_err(|e| LocalApiError::Profile(e.to_string()))?;
        self.audit.push(format!(
            "api.evaluate_policy profile={} action={:?}",
            ctx.profile_id, decision.action
        ));
        Ok(decision.reason_code)
    }

    pub fn audit_entries(&self) -> &[String] {
        &self.audit
    }

    fn grant(&self, ctx: &RequestContext) -> Result<&ProfileScopeGrant, LocalApiError> {
        self.sessions
            .get(&ctx.token)
            .ok_or(LocalApiError::Unauthorized)?
            .get(&ctx.profile_id)
            .ok_or(LocalApiError::Forbidden)
    }

    fn granted_profile_ids(&self, token: &str) -> Vec<Uuid> {
        self.sessions
            .get(token)
            .map(|m| m.keys().copied().collect::<Vec<_>>())
            .unwrap_or_default()
    }

    fn find_consent(
        &self,
        token: &str,
        profile_id: Uuid,
        operation: &str,
    ) -> Option<&ConsentGrant> {
        self.consent.get(token).and_then(|items| {
            items
                .iter()
                .find(|v| v.profile_id == profile_id && v.operation == operation)
        })
    }
}

impl Default for LocalApi {
    fn default() -> Self {
        Self::new()
    }
}

fn unix_ms_now() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
