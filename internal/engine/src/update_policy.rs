use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    DisabledByDefault,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineUpdatePolicy {
    pub mode: UpdateMode,
    pub allow_user_enable: bool,
}

impl Default for EngineUpdatePolicy {
    fn default() -> Self {
        Self {
            mode: UpdateMode::DisabledByDefault,
            allow_user_enable: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineUpdateArtifact {
    pub version: String,
    pub signature: String,
}

#[derive(Debug, Error)]
pub enum UpdatePolicyError {
    #[error("updates disabled by policy")]
    Disabled,
    #[error("signature verification failed")]
    SignatureInvalid,
}

#[derive(Debug, Default, Clone)]
pub struct EngineUpdateService;

impl EngineUpdateService {
    pub fn can_check_updates(&self, policy: &EngineUpdatePolicy) -> bool {
        matches!(policy.mode, UpdateMode::Manual)
    }

    pub fn verify_and_apply(
        &self,
        policy: &EngineUpdatePolicy,
        artifact: &EngineUpdateArtifact,
        expected_signature: &str,
    ) -> Result<String, UpdatePolicyError> {
        if !self.can_check_updates(policy) {
            return Err(UpdatePolicyError::Disabled);
        }
        if artifact.signature != expected_signature {
            return Err(UpdatePolicyError::SignatureInvalid);
        }
        Ok(format!("updated_to_{}", artifact.version))
    }
}
