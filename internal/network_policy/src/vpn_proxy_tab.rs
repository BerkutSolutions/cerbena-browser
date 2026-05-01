use serde::{Deserialize, Serialize};

use crate::{
    proxy::{ProxyHealth, ProxyTransportAdapter},
    vpn::{VpnHealth, VpnTunnelAdapter},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnProxyTabPayload {
    pub route_mode: String,
    pub proxy: Option<ProxyTransportAdapter>,
    pub vpn: Option<VpnTunnelAdapter>,
    pub kill_switch_enabled: bool,
}

pub fn validate_vpn_proxy_tab(payload: &VpnProxyTabPayload) -> Result<(), String> {
    let mode = payload.route_mode.as_str();
    let valid = ["direct", "proxy", "vpn", "tor", "hybrid"];
    if !valid.contains(&mode) {
        return Err("invalid route_mode".to_string());
    }
    if matches!(mode, "proxy" | "hybrid") && payload.proxy.is_none() {
        return Err("proxy config is required for proxy/hybrid mode".to_string());
    }
    if matches!(mode, "vpn" | "hybrid") && payload.vpn.is_none() {
        return Err("vpn config is required for vpn/hybrid mode".to_string());
    }
    if let Some(proxy) = &payload.proxy {
        proxy.validate().map_err(|e| e.to_string())?;
    }
    if let Some(vpn) = &payload.vpn {
        vpn.validate().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn test_connect(
    payload: &VpnProxyTabPayload,
    timeout_ms: u64,
) -> Result<(Option<ProxyHealth>, Option<VpnHealth>), String> {
    validate_vpn_proxy_tab(payload)?;
    let proxy = match &payload.proxy {
        Some(p) => Some(p.health_check(timeout_ms).map_err(|e| e.to_string())?),
        None => None,
    };
    let vpn = match &payload.vpn {
        Some(v) => Some(v.health_check(timeout_ms).map_err(|e| e.to_string())?),
        None => None,
    };
    Ok((proxy, vpn))
}
