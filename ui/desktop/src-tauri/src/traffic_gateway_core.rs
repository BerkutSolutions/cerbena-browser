use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs, UdpSocket},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use browser_network_policy::{ProxyProtocol, VpnProtocol, VpnProxyTabPayload};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::launcher_commands::load_global_security_record;
use crate::network_sandbox::{
    resolve_profile_network_sandbox_mode, resolve_profile_network_sandbox_view,
    ResolvedNetworkSandboxMode,
};
use crate::route_runtime::{
    route_runtime_required_for_profile, runtime_proxy_endpoint, runtime_session_active,
};
use crate::service_domains::service_domain_seeds;
use crate::state::AppState;
#[path = "traffic_gateway_telemetry.rs"]
mod telemetry;
#[path = "traffic_gateway_transport.rs"]
mod transport;
#[path = "traffic_gateway_policy.rs"]
mod traffic_gateway_policy;
#[path = "traffic_gateway_tunnel.rs"]
mod traffic_gateway_tunnel;

const TRAFFIC_RETENTION_MS: u128 = 24 * 60 * 60 * 1000;
const ROUTE_HEALTH_TTL_MS: u128 = 30_000;
const ROUTE_HEALTH_TIMEOUT_MS: u64 = 1_500;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrafficRulesStore {
    pub global_blocked_domains: BTreeSet<String>,
    pub profile_blocked_domains: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficLogEntry {
    pub id: String,
    pub timestamp_epoch_ms: u128,
    pub profile_id: String,
    pub profile_name: String,
    pub request_host: String,
    pub request_kind: String,
    pub status: String,
    pub reason: String,
    pub route: String,
    pub latency_ms: u128,
    pub source_ip: String,
    pub blocked_globally: bool,
    pub blocked_for_profile: bool,
}

#[derive(Debug, Default)]
pub struct TrafficGatewayState {
    pub listeners: BTreeMap<String, GatewayListenerSession>,
    pub traffic_log: Vec<TrafficLogEntry>,
    pub rules: TrafficRulesStore,
    pub(crate) route_health_cache: BTreeMap<String, RouteHealthCacheEntry>,
}

#[derive(Debug, Clone)]
pub struct GatewayListenerSession {
    pub port: u16,
    pub shutdown: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct GatewayLaunchConfig {
    pub port: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct GatewayDecision {
    blocked: bool,
    reason: String,
    route: String,
    blocked_globally: bool,
    blocked_for_profile: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RouteHealthCacheEntry {
    checked_at_ms: u128,
    blocked_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedProxyRequest {
    request_kind: String,
    host: String,
    port: u16,
    connect_tunnel: bool,
    header_bytes: Vec<u8>,
    passthrough_bytes: Vec<u8>,
}

pub fn load_rules_store(path: &PathBuf) -> Result<TrafficRulesStore, String> {
    telemetry::load_rules_store_impl(path)
}

pub fn persist_rules_store(path: &PathBuf, rules: &TrafficRulesStore) -> Result<(), String> {
    telemetry::persist_rules_store_impl(path, rules)
}

pub fn load_traffic_log(path: &PathBuf) -> Result<Vec<TrafficLogEntry>, String> {
    telemetry::load_traffic_log_impl(path)
}

#[allow(dead_code)]

#[path = "traffic_gateway_core_runtime.rs"]
mod runtime;
pub use runtime::*;
