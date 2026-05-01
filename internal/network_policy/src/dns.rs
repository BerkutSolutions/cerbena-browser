use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DnsMode {
    System,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub mode: DnsMode,
    pub servers: Vec<String>,
    pub doh_url: Option<String>,
    pub dot_server_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsResolverAdapter {
    pub profile_id: String,
    pub config: DnsConfig,
}

#[derive(Debug, Error)]
pub enum DnsError {
    #[error("invalid dns config: {0}")]
    InvalidConfig(String),
}

impl DnsResolverAdapter {
    pub fn validate(&self) -> Result<(), DnsError> {
        match self.config.mode {
            DnsMode::System => Ok(()),
            DnsMode::Custom => {
                if self.config.servers.is_empty() {
                    return Err(DnsError::InvalidConfig(
                        "custom mode requires at least one DNS server".to_string(),
                    ));
                }
                for s in &self.config.servers {
                    if s.parse::<IpAddr>().is_err() {
                        return Err(DnsError::InvalidConfig(format!(
                            "invalid DNS server IP: {s}"
                        )));
                    }
                }
                Ok(())
            }
        }
    }

    pub fn effective_resolvers(&self) -> Result<Vec<String>, DnsError> {
        self.validate()?;
        Ok(match self.config.mode {
            DnsMode::System => vec!["system".to_string()],
            DnsMode::Custom => self.config.servers.clone(),
        })
    }
}
