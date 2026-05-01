use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorRouteGuard {
    pub tor_required: bool,
    pub tor_up: bool,
    pub dns_over_tor: bool,
}

impl TorRouteGuard {
    pub fn evaluate(&self) -> Result<(), String> {
        if !self.tor_required {
            return Ok(());
        }
        if !self.tor_up {
            return Err("tor_required_but_unavailable".to_string());
        }
        if !self.dns_over_tor {
            return Err("tor_dns_leak_risk".to_string());
        }
        Ok(())
    }
}
