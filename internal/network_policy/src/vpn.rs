use std::{
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VpnProtocol {
    Wireguard,
    Openvpn,
    Amnezia,
    Vmess,
    Vless,
    Trojan,
    Shadowsocks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnTunnelAdapter {
    pub protocol: VpnProtocol,
    pub endpoint_host: String,
    pub endpoint_port: u16,
    pub profile_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnHealth {
    pub connected: bool,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum VpnAdapterError {
    #[error("invalid vpn config: {0}")]
    InvalidConfig(String),
    #[error("vpn health check failed: {0}")]
    HealthCheck(String),
}

impl VpnTunnelAdapter {
    pub fn validate(&self) -> Result<(), VpnAdapterError> {
        if self.endpoint_host.trim().is_empty() {
            return Err(VpnAdapterError::InvalidConfig(
                "endpoint_host must not be empty".to_string(),
            ));
        }
        if self.endpoint_port == 0 {
            return Err(VpnAdapterError::InvalidConfig(
                "endpoint_port must be non-zero".to_string(),
            ));
        }
        Ok(())
    }

    pub fn health_check(&self, timeout_ms: u64) -> Result<VpnHealth, VpnAdapterError> {
        self.validate()?;
        let mut addrs = (self.endpoint_host.as_str(), self.endpoint_port)
            .to_socket_addrs()
            .map_err(|e| VpnAdapterError::HealthCheck(format!("resolve failed: {e}")))?;
        let addr = addrs
            .next()
            .ok_or_else(|| VpnAdapterError::HealthCheck("no resolved endpoint".to_string()))?;
        let timeout = Duration::from_millis(timeout_ms.max(1));
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(_) => Ok(VpnHealth {
                connected: true,
                message: "vpn endpoint reachable".to_string(),
            }),
            Err(e) => Ok(VpnHealth {
                connected: false,
                message: format!("vpn endpoint not reachable: {e}"),
            }),
        }
    }
}
