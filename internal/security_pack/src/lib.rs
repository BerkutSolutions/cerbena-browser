use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuiteStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteResult {
    pub suite: String,
    pub status: SuiteStatus,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPackReport {
    pub profile_id: Uuid,
    pub results: Vec<SuiteResult>,
}

impl SecurityPackReport {
    pub fn is_release_allowed(&self) -> bool {
        self.results.iter().all(|v| v.status == SuiteStatus::Passed)
    }
}

#[derive(Debug, Default, Clone)]
pub struct SecurityPackRunner;

impl SecurityPackRunner {
    pub fn run(
        &self,
        profile_id: Uuid,
        isolation_ok: bool,
        kill_switch_ok: bool,
        dns_leak_ok: bool,
        extension_policy_ok: bool,
        stress_ok: bool,
    ) -> SecurityPackReport {
        SecurityPackReport {
            profile_id,
            results: vec![
                result("isolation", isolation_ok),
                result("kill_switch", kill_switch_ok),
                result("dns_leak", dns_leak_ok),
                result("extension_abuse", extension_policy_ok),
                result("stress", stress_ok),
            ],
        }
    }
}

fn result(name: &str, ok: bool) -> SuiteResult {
    SuiteResult {
        suite: name.to_string(),
        status: if ok {
            SuiteStatus::Passed
        } else {
            SuiteStatus::Failed
        },
        details: if ok {
            "ok".to_string()
        } else {
            "failed".to_string()
        },
    }
}
