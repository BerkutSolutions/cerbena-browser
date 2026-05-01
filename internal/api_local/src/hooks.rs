use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchHookPolicy {
    pub url: String,
    pub timeout_ms: u64,
    pub allow_insecure_http: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookExecutionResult {
    pub accepted: bool,
    pub executed: bool,
    pub message_key: String,
}

#[derive(Debug, Default, Clone)]
pub struct LaunchHookService;

impl LaunchHookService {
    pub fn validate(&self, policy: &LaunchHookPolicy) -> Result<(), String> {
        if policy.url.trim().is_empty() {
            return Err("launch_hook.url.empty".to_string());
        }
        if !(policy.url.starts_with("https://")
            || (policy.allow_insecure_http && policy.url.starts_with("http://")))
        {
            return Err("launch_hook.url.scheme_invalid".to_string());
        }
        if policy.timeout_ms == 0 || policy.timeout_ms > 30_000 {
            return Err("launch_hook.timeout.invalid".to_string());
        }
        Ok(())
    }

    pub fn execute(
        &self,
        policy: &LaunchHookPolicy,
        simulated_latency_ms: u64,
    ) -> HookExecutionResult {
        if self.validate(policy).is_err() {
            return HookExecutionResult {
                accepted: false,
                executed: false,
                message_key: "launch_hook.validation_failed".to_string(),
            };
        }
        if simulated_latency_ms > policy.timeout_ms {
            return HookExecutionResult {
                accepted: true,
                executed: false,
                message_key: "launch_hook.timeout".to_string(),
            };
        }
        HookExecutionResult {
            accepted: true,
            executed: true,
            message_key: "launch_hook.executed".to_string(),
        }
    }
}
