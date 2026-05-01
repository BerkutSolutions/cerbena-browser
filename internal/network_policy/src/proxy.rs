use std::{
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyProtocol {
    Http,
    Socks4,
    Socks5,
    Shadowsocks,
    Vmess,
    Vless,
    Trojan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTransportAdapter {
    pub protocol: ProxyProtocol,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHealth {
    pub reachable: bool,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum ProxyAdapterError {
    #[error("invalid proxy config: {0}")]
    InvalidConfig(String),
    #[error("proxy health check failed: {0}")]
    HealthCheck(String),
}

impl ProxyTransportAdapter {
    pub fn validate(&self) -> Result<(), ProxyAdapterError> {
        if self.host.trim().is_empty() {
            return Err(ProxyAdapterError::InvalidConfig(
                "host must not be empty".to_string(),
            ));
        }
        if self.port == 0 {
            return Err(ProxyAdapterError::InvalidConfig(
                "port must be non-zero".to_string(),
            ));
        }
        if matches!(self.protocol, ProxyProtocol::Http | ProxyProtocol::Socks5)
            && self.username.is_some()
            && self.password.is_none()
        {
            return Err(ProxyAdapterError::InvalidConfig(
                "password is required when username is set".to_string(),
            ));
        }
        Ok(())
    }

    pub fn health_check(&self, timeout_ms: u64) -> Result<ProxyHealth, ProxyAdapterError> {
        self.validate()?;
        let mut addrs = (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|e| ProxyAdapterError::HealthCheck(format!("resolve failed: {e}")))?;
        let addr = addrs.next().ok_or_else(|| {
            ProxyAdapterError::HealthCheck("no address resolved for proxy".to_string())
        })?;
        let timeout = Duration::from_millis(timeout_ms.max(1));
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(_) => Ok(ProxyHealth {
                reachable: true,
                message: "proxy endpoint is reachable".to_string(),
            }),
            Err(e) => Ok(ProxyHealth {
                reachable: false,
                message: format!("proxy not reachable: {e}"),
            }),
        }
    }
}
