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
struct GatewayDecision {
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
struct ParsedProxyRequest {
    request_kind: String,
    host: String,
    port: u16,
    connect_tunnel: bool,
    header_bytes: Vec<u8>,
    passthrough_bytes: Vec<u8>,
}

pub fn load_rules_store(path: &PathBuf) -> Result<TrafficRulesStore, String> {
    if !path.exists() {
        return Ok(TrafficRulesStore::default());
    }
    let bytes = fs::read(path).map_err(|e| format!("read traffic rules: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse traffic rules: {e}"))
}

pub fn persist_rules_store(path: &PathBuf, rules: &TrafficRulesStore) -> Result<(), String> {
    let bytes =
        serde_json::to_vec_pretty(rules).map_err(|e| format!("serialize traffic rules: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write traffic rules: {e}"))
}

pub fn load_traffic_log(path: &PathBuf) -> Result<Vec<TrafficLogEntry>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(path).map_err(|e| format!("read traffic log: {e}"))?;
    let mut parsed = match serde_json::from_slice::<Vec<TrafficLogEntry>>(&bytes) {
        Ok(entries) => entries,
        Err(error) => {
            backup_corrupt_traffic_log(path, &bytes);
            eprintln!(
                "[traffic-gateway] ignoring corrupt traffic log at {}: {error}",
                path.display()
            );
            return Ok(Vec::new());
        }
    };
    prune_traffic_log(&mut parsed);
    Ok(parsed)
}

fn backup_corrupt_traffic_log(path: &PathBuf, bytes: &[u8]) {
    let backup_path = path.with_extension(format!("corrupt-{}.json", now_epoch_ms()));
    if fs::rename(path, &backup_path).is_err() {
        let _ = fs::write(backup_path, bytes);
        let _ = fs::remove_file(path);
    }
}

fn persist_traffic_log(path: &PathBuf, entries: &[TrafficLogEntry]) -> Result<(), String> {
    let bytes =
        serde_json::to_vec_pretty(entries).map_err(|e| format!("serialize traffic log: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write traffic log: {e}"))
}

pub fn ensure_profile_gateway(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Result<GatewayLaunchConfig, String> {
    let profile_id_string = profile_id.to_string();
    {
        let state = app_handle.state::<AppState>();
        let gateway = state
            .traffic_gateway
            .lock()
            .map_err(|_| "traffic gateway lock poisoned".to_string())?;
        if let Some(session) = gateway.listeners.get(&profile_id_string) {
            if !session.shutdown.load(Ordering::Relaxed) {
                return Ok(GatewayLaunchConfig { port: session.port });
            }
        }
    }

    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind traffic gateway: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("gateway local addr: {e}"))?
        .port();
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("gateway nonblocking mode: {e}"))?;
    let shutdown = Arc::new(AtomicBool::new(false));

    {
        let state = app_handle.state::<AppState>();
        let mut gateway = state
            .traffic_gateway
            .lock()
            .map_err(|_| "traffic gateway lock poisoned".to_string())?;
        gateway.listeners.insert(
            profile_id_string.clone(),
            GatewayListenerSession {
                port,
                shutdown: shutdown.clone(),
            },
        );
    }

    let app = app_handle.clone();
    thread::spawn(move || {
        while !shutdown.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let app = app.clone();
                    thread::spawn(move || {
                        if let Err(err) = handle_client(app, profile_id, stream) {
                            eprintln!("[traffic-gateway] client handling failed: {err}");
                        }
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(err) => {
                    eprintln!("[traffic-gateway] accept failed: {err}");
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    });

    eprintln!(
        "[traffic-gateway] profile={} listener started on 127.0.0.1:{}",
        profile_id, port
    );

    Ok(GatewayLaunchConfig { port })
}

pub fn stop_profile_gateway(app_handle: &AppHandle, profile_id: Uuid) {
    let state = app_handle.state::<AppState>();
    let session = {
        let mut gateway = match state.traffic_gateway.lock() {
            Ok(value) => value,
            Err(_) => return,
        };
        gateway.listeners.remove(&profile_id.to_string())
    };
    if let Some(session) = session {
        eprintln!(
            "[traffic-gateway] profile={} listener stopping on 127.0.0.1:{}",
            profile_id, session.port
        );
        session.shutdown.store(true, Ordering::Relaxed);
    }
}

pub fn stop_all_profile_gateways(app_handle: &AppHandle) {
    let profile_ids = {
        let state = app_handle.state::<AppState>();
        let gateway = match state.traffic_gateway.lock() {
            Ok(value) => value,
            Err(_) => return,
        };
        gateway
            .listeners
            .keys()
            .filter_map(|value| Uuid::parse_str(value).ok())
            .collect::<Vec<_>>()
    };
    for profile_id in profile_ids {
        stop_profile_gateway(app_handle, profile_id);
    }
}

pub fn list_traffic_log(state: &AppState) -> Result<Vec<TrafficLogEntry>, String> {
    let mut gateway = state
        .traffic_gateway
        .lock()
        .map_err(|_| "traffic gateway lock poisoned".to_string())?;
    let original_len = gateway.traffic_log.len();
    prune_traffic_log(&mut gateway.traffic_log);
    let changed = gateway.traffic_log.len() != original_len;
    let snapshot = gateway.traffic_log.clone();
    drop(gateway);
    if changed {
        let path = state.traffic_gateway_log_path(&state.app_handle)?;
        let _ = persist_traffic_log(&path, &snapshot);
    }
    Ok(snapshot.iter().rev().cloned().collect())
}

pub fn set_domain_block_rule(
    state: &AppState,
    profile_id: Option<String>,
    domain: &str,
    blocked: bool,
) -> Result<(), String> {
    let normalized = normalize_domain(domain);
    if normalized.is_empty() {
        return Err("domain is empty".to_string());
    }
    let mut gateway = state
        .traffic_gateway
        .lock()
        .map_err(|_| "traffic gateway lock poisoned".to_string())?;
    if let Some(profile_id) = profile_id {
        let entry = gateway
            .rules
            .profile_blocked_domains
            .entry(profile_id)
            .or_default();
        if blocked {
            entry.insert(normalized);
        } else {
            entry.remove(&normalized);
        }
    } else if blocked {
        gateway.rules.global_blocked_domains.insert(normalized);
    } else {
        gateway.rules.global_blocked_domains.remove(&normalized);
    }
    let path = state.traffic_gateway_rules_path(&state.app_handle)?;
    persist_rules_store(&path, &gateway.rules)
}

fn handle_client(
    app_handle: AppHandle,
    profile_id: Uuid,
    mut client: TcpStream,
) -> Result<(), String> {
    client
        .set_nonblocking(false)
        .map_err(|e| format!("client blocking mode: {e}"))?;
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    client
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;

    let started_at = Instant::now();
    let parsed = read_proxy_request(&mut client)?;
    let profile_name = profile_name(&app_handle, profile_id);
    let decision = evaluate_request(&app_handle, profile_id, &parsed.host);
    eprintln!(
        "[traffic-gateway] profile={} host={} kind={} blocked={} reason={}",
        profile_id, parsed.host, parsed.request_kind, decision.blocked, decision.reason
    );
    if decision.blocked {
        write_forbidden(&mut client, &parsed.host, &decision.reason).map_err(|e| e.to_string())?;
        append_traffic_log(
            &app_handle,
            TrafficLogEntry {
                id: format!("traffic-{}", now_epoch_ms()),
                timestamp_epoch_ms: now_epoch_ms(),
                profile_id: profile_id.to_string(),
                profile_name,
                request_host: parsed.host.clone(),
                request_kind: parsed.request_kind.clone(),
                status: "blocked".to_string(),
                reason: decision.reason,
                route: decision.route,
                latency_ms: started_at.elapsed().as_millis(),
                source_ip: client
                    .peer_addr()
                    .map(|addr| addr.ip().to_string())
                    .unwrap_or_else(|_| "127.0.0.1".to_string()),
                blocked_globally: decision.blocked_globally,
                blocked_for_profile: decision.blocked_for_profile,
            },
        );
        return Ok(());
    }

    let route_policy = current_route_policy(&app_handle, profile_id);
    let processing_result = if parsed.connect_tunnel {
        handle_connect_request(&app_handle, profile_id, &mut client, &parsed, &route_policy)
    } else {
        handle_http_request(&app_handle, profile_id, &mut client, &parsed, &route_policy)
    };
    let (status, reason) = match processing_result {
        Ok(_) => ("processed".to_string(), "allowed".to_string()),
        Err(error) => ("error".to_string(), error.to_string()),
    };
    append_traffic_log(
        &app_handle,
        TrafficLogEntry {
            id: format!("traffic-{}", now_epoch_ms()),
            timestamp_epoch_ms: now_epoch_ms(),
            profile_id: profile_id.to_string(),
            profile_name,
            request_host: parsed.host,
            request_kind: parsed.request_kind,
            status,
            reason: reason.clone(),
            route: decision.route,
            latency_ms: started_at.elapsed().as_millis(),
            source_ip: client
                .peer_addr()
                .map(|addr| addr.ip().to_string())
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            blocked_globally: false,
            blocked_for_profile: false,
        },
    );
    if reason == "allowed" {
        Ok(())
    } else {
        Err(reason)
    }
}

fn read_proxy_request(client: &mut TcpStream) -> Result<ParsedProxyRequest, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    while !buffer.windows(4).any(|window| window == b"\r\n\r\n") && buffer.len() < 64 * 1024 {
        let read = client.read(&mut chunk).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    if buffer.is_empty() {
        return Err("empty proxy request".to_string());
    }
    let request_text = String::from_utf8_lossy(&buffer);
    let head_end = request_text
        .find("\r\n\r\n")
        .map(|idx| idx + 4)
        .unwrap_or(buffer.len());
    let head = &request_text[..head_end];
    let mut lines = head.lines();
    let first_line = lines.next().unwrap_or_default().trim().to_string();
    if first_line.is_empty() {
        return Err("invalid proxy first line".to_string());
    }
    let parts = first_line.split_whitespace().collect::<Vec<_>>();
    let method = parts.first().copied().unwrap_or("UNKNOWN").to_uppercase();
    if method == "CONNECT" {
        let authority = parts.get(1).copied().unwrap_or_default();
        let (host, port) = split_host_port(authority, 443);
        return Ok(ParsedProxyRequest {
            request_kind: method,
            host,
            port,
            connect_tunnel: true,
            header_bytes: buffer[..head_end].to_vec(),
            passthrough_bytes: buffer[head_end..].to_vec(),
        });
    }

    let request_target = parts.get(1).copied().unwrap_or_default();
    let host_header = head
        .lines()
        .find_map(|line| {
            line.strip_prefix("Host:")
                .or_else(|| line.strip_prefix("host:"))
        })
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    let target = if request_target.starts_with("http://") || request_target.starts_with("https://")
    {
        request_target.to_string()
    } else if !host_header.is_empty() {
        format!("http://{host_header}{request_target}")
    } else {
        request_target.to_string()
    };
    let (host, port, normalized_first_line) = normalize_http_first_line(&first_line, &target);
    Ok(ParsedProxyRequest {
        request_kind: method,
        host,
        port,
        connect_tunnel: false,
        header_bytes: rebuild_header(&buffer[..head_end], &first_line, &normalized_first_line),
        passthrough_bytes: buffer[head_end..].to_vec(),
    })
}

fn handle_connect_request(
    app_handle: &AppHandle,
    profile_id: Uuid,
    client: &mut TcpStream,
    parsed: &ParsedProxyRequest,
    route_policy: &Option<VpnProxyTabPayload>,
) -> std::io::Result<()> {
    let mut upstream = open_upstream_stream(
        app_handle,
        profile_id,
        route_policy,
        &parsed.host,
        parsed.port,
        true,
    )?;
    if route_uses_http_proxy(app_handle, profile_id, route_policy) {
        upstream.write_all(&parsed.header_bytes)?;
        upstream.flush()?;
        let response_head = read_proxy_response_head(&mut upstream)?;
        client.write_all(&response_head)?;
        client.flush()?;
        if !parsed.passthrough_bytes.is_empty() {
            upstream.write_all(&parsed.passthrough_bytes)?;
            upstream.flush()?;
        }
    } else {
        client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
        client.flush()?;
        if !parsed.passthrough_bytes.is_empty() {
            upstream.write_all(&parsed.passthrough_bytes)?;
            upstream.flush()?;
        }
    }
    clear_bridge_timeouts(client)?;
    clear_bridge_timeouts(&upstream)?;
    bridge_streams(client, upstream)
}

fn handle_http_request(
    app_handle: &AppHandle,
    profile_id: Uuid,
    client: &mut TcpStream,
    parsed: &ParsedProxyRequest,
    route_policy: &Option<VpnProxyTabPayload>,
) -> std::io::Result<()> {
    let mut upstream = open_upstream_stream(
        app_handle,
        profile_id,
        route_policy,
        &parsed.host,
        parsed.port,
        false,
    )?;
    upstream.write_all(&parsed.header_bytes)?;
    if !parsed.passthrough_bytes.is_empty() {
        upstream.write_all(&parsed.passthrough_bytes)?;
    }
    upstream.flush()?;
    clear_bridge_timeouts(client)?;
    clear_bridge_timeouts(&upstream)?;
    bridge_streams(client, upstream)
}

fn clear_bridge_timeouts(stream: &TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    Ok(())
}

fn bridge_streams(client: &mut TcpStream, upstream: TcpStream) -> std::io::Result<()> {
    let mut upstream_reader = upstream.try_clone()?;
    let mut client_writer = client.try_clone()?;
    let upstream_to_client = thread::spawn(move || {
        if let Err(error) = std::io::copy(&mut upstream_reader, &mut client_writer) {
            eprintln!("[traffic-gateway] upstream->client bridge failed: {error}");
        }
        let _ = client_writer.shutdown(Shutdown::Write);
    });

    let mut upstream_writer = upstream;
    let mut client_reader = client.try_clone()?;
    if let Err(error) = std::io::copy(&mut client_reader, &mut upstream_writer) {
        eprintln!("[traffic-gateway] client->upstream bridge failed: {error}");
    }
    let _ = upstream_writer.shutdown(Shutdown::Write);
    let _ = upstream_to_client.join();
    Ok(())
}

fn read_proxy_response_head(upstream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    while !buffer.windows(4).any(|window| window == b"\r\n\r\n") && buffer.len() < 64 * 1024 {
        let read = upstream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    Ok(buffer)
}

fn open_upstream_stream(
    app_handle: &AppHandle,
    profile_id: Uuid,
    route_policy: &Option<VpnProxyTabPayload>,
    target_host: &str,
    target_port: u16,
    connect_tunnel: bool,
) -> std::io::Result<TcpStream> {
    if let Some((runtime_host, runtime_port)) = runtime_proxy_endpoint(app_handle, profile_id) {
        return connect_via_local_socks5_endpoint(
            &runtime_host,
            runtime_port,
            target_host,
            target_port,
        );
    }
    if let Some(proxy) = route_policy
        .as_ref()
        .and_then(|payload| payload.proxy.as_ref())
    {
        match proxy.protocol {
            ProxyProtocol::Http => {
                let stream = TcpStream::connect(format!("{}:{}", proxy.host, proxy.port))?;
                if connect_tunnel {
                    stream.set_nodelay(true)?;
                }
                return Ok(stream);
            }
            ProxyProtocol::Socks4 | ProxyProtocol::Socks5 => {
                return connect_via_socks_proxy(proxy, target_host, target_port);
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "unsupported proxy protocol for traffic gateway",
                ));
            }
        }
    }
    TcpStream::connect(format!("{target_host}:{target_port}"))
}

fn connect_via_local_socks5_endpoint(
    runtime_host: &str,
    runtime_port: u16,
    target_host: &str,
    target_port: u16,
) -> std::io::Result<TcpStream> {
    let mut stream = TcpStream::connect(format!("{runtime_host}:{runtime_port}"))?;
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    socks5_connect(&mut stream, target_host, target_port, None, None)?;
    Ok(stream)
}

fn route_uses_http_proxy(
    app_handle: &AppHandle,
    profile_id: Uuid,
    route_policy: &Option<VpnProxyTabPayload>,
) -> bool {
    if runtime_proxy_endpoint(app_handle, profile_id).is_some() {
        return false;
    }
    route_policy
        .as_ref()
        .and_then(|payload| payload.proxy.as_ref())
        .map(|proxy| matches!(proxy.protocol, ProxyProtocol::Http))
        .unwrap_or(false)
}

fn connect_via_socks_proxy(
    proxy: &browser_network_policy::ProxyTransportAdapter,
    target_host: &str,
    target_port: u16,
) -> std::io::Result<TcpStream> {
    let mut stream = TcpStream::connect(format!("{}:{}", proxy.host, proxy.port))?;
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    match proxy.protocol {
        ProxyProtocol::Socks4 => socks4_connect(
            &mut stream,
            target_host,
            target_port,
            proxy.username.as_deref(),
        )?,
        ProxyProtocol::Socks5 => socks5_connect(
            &mut stream,
            target_host,
            target_port,
            proxy.username.as_deref(),
            proxy.password.as_deref(),
        )?,
        _ => {}
    }
    Ok(stream)
}

fn socks5_connect(
    stream: &mut TcpStream,
    target_host: &str,
    target_port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> std::io::Result<()> {
    let use_auth = username.is_some() || password.is_some();
    let methods = if use_auth {
        vec![0x00u8, 0x02u8]
    } else {
        vec![0x00u8]
    };
    let mut greeting = Vec::with_capacity(2 + methods.len());
    greeting.push(0x05);
    greeting.push(methods.len() as u8);
    greeting.extend_from_slice(&methods);
    stream.write_all(&greeting)?;

    let mut method_reply = [0u8; 2];
    stream.read_exact(&mut method_reply)?;
    if method_reply[0] != 0x05 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "SOCKS5: invalid handshake response version",
        ));
    }
    match method_reply[1] {
        0x00 => {}
        0x02 => {
            let user = username.unwrap_or_default().as_bytes();
            let pass = password.unwrap_or_default().as_bytes();
            if user.len() > 255 || pass.len() > 255 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "SOCKS5: username/password is too long",
                ));
            }
            let mut auth_packet = Vec::with_capacity(3 + user.len() + pass.len());
            auth_packet.push(0x01);
            auth_packet.push(user.len() as u8);
            auth_packet.extend_from_slice(user);
            auth_packet.push(pass.len() as u8);
            auth_packet.extend_from_slice(pass);
            stream.write_all(&auth_packet)?;
            let mut auth_reply = [0u8; 2];
            stream.read_exact(&mut auth_reply)?;
            if auth_reply[1] != 0x00 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "SOCKS5: authentication failed",
                ));
            }
        }
        0xFF => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "SOCKS5: no compatible auth method",
            ));
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "SOCKS5: unsupported auth method",
            ));
        }
    }

    let mut request = Vec::with_capacity(8 + target_host.len());
    request.extend_from_slice(&[0x05, 0x01, 0x00]);
    if let Ok(ipv4) = target_host.parse::<std::net::Ipv4Addr>() {
        request.push(0x01);
        request.extend_from_slice(&ipv4.octets());
    } else if let Ok(ipv6) = target_host.parse::<std::net::Ipv6Addr>() {
        request.push(0x04);
        request.extend_from_slice(&ipv6.octets());
    } else {
        let host_bytes = target_host.as_bytes();
        if host_bytes.len() > 255 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "SOCKS5: target host is too long",
            ));
        }
        request.push(0x03);
        request.push(host_bytes.len() as u8);
        request.extend_from_slice(host_bytes);
    }
    request.push((target_port >> 8) as u8);
    request.push((target_port & 0xFF) as u8);
    stream.write_all(&request)?;

    let mut header = [0u8; 4];
    stream.read_exact(&mut header)?;
    if header[0] != 0x05 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "SOCKS5: invalid connect response version",
        ));
    }
    if header[1] != 0x00 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("SOCKS5: connect failed with code {}", header[1]),
        ));
    }
    match header[3] {
        0x01 => {
            let mut payload = [0u8; 6];
            stream.read_exact(&mut payload)?;
        }
        0x04 => {
            let mut payload = [0u8; 18];
            stream.read_exact(&mut payload)?;
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len)?;
            let mut payload = vec![0u8; len[0] as usize + 2];
            stream.read_exact(&mut payload)?;
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "SOCKS5: invalid connect response address type",
            ));
        }
    }

    Ok(())
}

fn socks4_connect(
    stream: &mut TcpStream,
    target_host: &str,
    target_port: u16,
    username: Option<&str>,
) -> std::io::Result<()> {
    let user = username.unwrap_or_default().as_bytes();
    if user.len() > 255 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "SOCKS4: username is too long",
        ));
    }
    let mut request = Vec::with_capacity(16 + target_host.len() + user.len());
    request.push(0x04);
    request.push(0x01);
    request.push((target_port >> 8) as u8);
    request.push((target_port & 0xFF) as u8);
    if let Ok(ipv4) = target_host.parse::<std::net::Ipv4Addr>() {
        request.extend_from_slice(&ipv4.octets());
        request.extend_from_slice(user);
        request.push(0x00);
    } else {
        request.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        request.extend_from_slice(user);
        request.push(0x00);
        request.extend_from_slice(target_host.as_bytes());
        request.push(0x00);
    }
    stream.write_all(&request)?;
    let mut response = [0u8; 8];
    stream.read_exact(&mut response)?;
    if response[1] != 0x5A {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("SOCKS4: connect failed with code {}", response[1]),
        ));
    }
    Ok(())
}

fn write_forbidden(client: &mut TcpStream, host: &str, reason: &str) -> std::io::Result<()> {
    let body = format!("Blocked by Cerbena gateway: {host}\nReason: {reason}\n");
    let response = format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    client.write_all(response.as_bytes())?;
    client.flush()
}

fn append_traffic_log(app_handle: &AppHandle, entry: TrafficLogEntry) {
    let state = app_handle.state::<AppState>();
    let mut gateway = match state.traffic_gateway.lock() {
        Ok(value) => value,
        Err(_) => return,
    };
    gateway.traffic_log.push(entry.clone());
    prune_traffic_log(&mut gateway.traffic_log);
    let snapshot = gateway.traffic_log.clone();
    drop(gateway);
    if let Ok(path) = state.traffic_gateway_log_path(app_handle) {
        let _ = persist_traffic_log(&path, &snapshot);
    }
    let _ = app_handle.emit("traffic-gateway-event", &entry);
}

fn prune_traffic_log(entries: &mut Vec<TrafficLogEntry>) {
    let cutoff = now_epoch_ms().saturating_sub(TRAFFIC_RETENTION_MS);
    entries.retain(|entry| entry.timestamp_epoch_ms >= cutoff);
}

fn evaluate_request(app_handle: &AppHandle, profile_id: Uuid, host: &str) -> GatewayDecision {
    let normalized = normalize_domain(host);
    let state = app_handle.state::<AppState>();
    let route_policy = current_route_policy(app_handle, profile_id);
    let route = current_route_mode(app_handle, profile_id, route_policy.as_ref());

    if let Some(reason) = route_kill_switch_reason(app_handle, profile_id, &route_policy) {
        return GatewayDecision {
            blocked: true,
            reason,
            route,
            blocked_globally: false,
            blocked_for_profile: false,
        };
    }

    if let Ok(gateway) = state.traffic_gateway.lock() {
        if gateway
            .rules
            .global_blocked_domains
            .iter()
            .any(|rule| host_matches(&normalized, rule))
        {
            return GatewayDecision {
                blocked: true,
                reason: "User global rule".to_string(),
                route,
                blocked_globally: true,
                blocked_for_profile: false,
            };
        }
        if gateway
            .rules
            .profile_blocked_domains
            .get(&profile_id.to_string())
            .map(|rules| rules.iter().any(|rule| host_matches(&normalized, rule)))
            .unwrap_or(false)
        {
            return GatewayDecision {
                blocked: true,
                reason: "User profile rule".to_string(),
                route,
                blocked_globally: false,
                blocked_for_profile: true,
            };
        }
    }

    if let Some(reason) = dns_block_reason(&state, profile_id, &normalized) {
        return GatewayDecision {
            blocked: true,
            reason,
            route,
            blocked_globally: false,
            blocked_for_profile: false,
        };
    }

    if let Some(reason) = global_security_block_reason(&state, &normalized) {
        return GatewayDecision {
            blocked: true,
            reason,
            route,
            blocked_globally: false,
            blocked_for_profile: false,
        };
    }

    GatewayDecision {
        blocked: false,
        reason: "Allowed".to_string(),
        route,
        blocked_globally: false,
        blocked_for_profile: false,
    }
}

fn route_kill_switch_reason(
    app_handle: &AppHandle,
    profile_id: Uuid,
    route_policy: &Option<VpnProxyTabPayload>,
) -> Option<String> {
    let state = app_handle.state::<AppState>();
    let profile_route_mode = profile_route_mode(&state, profile_id);
    if profile_route_mode == "direct" {
        return None;
    }
    let (global_vpn_enabled, block_without_vpn) = state
        .network_store
        .lock()
        .ok()
        .map(|store| {
            (
                store.global_route_settings.global_vpn_enabled,
                store.global_route_settings.block_without_vpn,
            )
        })
        .unwrap_or((false, false));
    if global_vpn_enabled || block_without_vpn {
        return if runtime_session_active(app_handle, profile_id) {
            None
        } else if global_vpn_enabled {
            Some("Kill-switch: global VPN route is unavailable".to_string())
        } else {
            Some("Kill-switch: VPN tunnel is required by global policy".to_string())
        };
    }

    let policy = route_policy.as_ref()?;
    if !policy.kill_switch_enabled {
        return None;
    }
    let cache_key = format!(
        "{}:{}",
        profile_id,
        serde_json::to_string(policy).unwrap_or_else(|_| policy.route_mode.clone())
    );
    let now = now_epoch_ms();

    {
        let state = app_handle.state::<AppState>();
        let lock_result = state.traffic_gateway.lock();
        if let Ok(gateway) = lock_result {
            if let Some(cached) = gateway.route_health_cache.get(&cache_key) {
                if now.saturating_sub(cached.checked_at_ms) < ROUTE_HEALTH_TTL_MS {
                    return cached.blocked_reason.clone();
                }
            }
        };
    };

    let computed = compute_route_kill_switch_reason(app_handle, profile_id, policy);
    {
        let state = app_handle.state::<AppState>();
        let lock_result = state.traffic_gateway.lock();
        if let Ok(mut gateway) = lock_result {
            gateway.route_health_cache.insert(
                cache_key,
                RouteHealthCacheEntry {
                    checked_at_ms: now,
                    blocked_reason: computed.clone(),
                },
            );
            if gateway.route_health_cache.len() > 128 {
                let cutoff = now.saturating_sub(ROUTE_HEALTH_TTL_MS * 2);
                gateway
                    .route_health_cache
                    .retain(|_, entry| entry.checked_at_ms >= cutoff);
            }
        };
    };
    computed
}

fn compute_route_kill_switch_reason(
    app_handle: &AppHandle,
    profile_id: Uuid,
    policy: &VpnProxyTabPayload,
) -> Option<String> {
    if let Some(strategy) = resolved_route_strategy(app_handle, profile_id) {
        match strategy {
            ResolvedNetworkSandboxMode::Blocked => {
                let state = app_handle.state::<AppState>();
                let reason = resolve_sandbox_strategy_reason(state.inner(), profile_id)
                    .unwrap_or_else(|| "selected isolated route policy blocks this backend".to_string());
                return Some(format!("Kill-switch: {reason}"));
            }
            ResolvedNetworkSandboxMode::Container => {
                if !resolved_sandbox_adapter_available(app_handle, profile_id) {
                    let state = app_handle.state::<AppState>();
                    let reason = resolve_sandbox_adapter_reason(state.inner(), profile_id)
                        .unwrap_or_else(|| {
                            "container sandbox mode is selected, but the adapter is not available"
                                .to_string()
                        });
                    return Some(format!("Kill-switch: {reason}"));
                }
            }
            _ => {}
        }
    }
    let runtime_required = route_runtime_required_for_profile(app_handle, profile_id);
    let runtime_active = runtime_session_active(app_handle, profile_id);
    if runtime_required && !runtime_active {
        let strategy_label = resolved_route_strategy(app_handle, profile_id)
            .map(|value| value.as_str().to_string())
            .unwrap_or_else(|| "runtime".to_string());
        return Some(format!(
            "Kill-switch: selected {strategy_label} route runtime is unavailable"
        ));
    }
    if runtime_active {
        return None;
    }
    match policy.route_mode.trim().to_lowercase().as_str() {
        "direct" => None,
        "proxy" => proxy_unavailable_reason(policy.proxy.as_ref()),
        "vpn" => vpn_unavailable_reason(policy.vpn.as_ref()),
        "hybrid" => proxy_unavailable_reason(policy.proxy.as_ref())
            .or_else(|| vpn_unavailable_reason(policy.vpn.as_ref())),
        "tor" => tor_unavailable_reason(app_handle, profile_id),
        _ => Some("Kill-switch: invalid route mode in policy".to_string()),
    }
}

fn proxy_unavailable_reason(
    proxy: Option<&browser_network_policy::ProxyTransportAdapter>,
) -> Option<String> {
    let Some(proxy) = proxy else {
        return Some("Kill-switch: proxy route is not configured".to_string());
    };
    if endpoint_reachable(&proxy.host, proxy.port, ROUTE_HEALTH_TIMEOUT_MS) {
        None
    } else {
        Some(format!(
            "Kill-switch: {} proxy endpoint is unavailable",
            proxy_protocol_label(proxy.protocol)
        ))
    }
}

fn vpn_unavailable_reason(
    vpn: Option<&browser_network_policy::VpnTunnelAdapter>,
) -> Option<String> {
    let Some(vpn) = vpn else {
        return Some("Kill-switch: VPN route is not configured".to_string());
    };
    if vpn_endpoint_reachable(
        vpn.protocol,
        &vpn.endpoint_host,
        vpn.endpoint_port,
        ROUTE_HEALTH_TIMEOUT_MS,
    ) {
        None
    } else {
        Some(format!(
            "Kill-switch: {} tunnel endpoint is unavailable",
            vpn_protocol_label(vpn.protocol)
        ))
    }
}

fn tor_unavailable_reason(app_handle: &AppHandle, profile_id: Uuid) -> Option<String> {
    let state = app_handle.state::<AppState>();
    let store = match state.network_store.lock() {
        Ok(value) => value,
        Err(_) => return Some("Kill-switch: unable to read route state".to_string()),
    };
    let Some(template_id) = store
        .profile_template_selection
        .get(&profile_id.to_string())
    else {
        return Some("Kill-switch: TOR route template is not selected".to_string());
    };
    let Some(template) = store.connection_templates.get(template_id) else {
        return Some("Kill-switch: TOR route template is missing".to_string());
    };
    let node = template
        .nodes
        .iter()
        .find(|item| item.connection_type == "tor");
    let Some(node) = node else {
        return Some("Kill-switch: TOR node is not configured in the template".to_string());
    };
    match node.protocol.as_str() {
        "obfs4" => {
            let bridge = node
                .bridges
                .as_deref()
                .and_then(parse_first_bridge_endpoint);
            let Some((host, port)) = bridge else {
                return Some("Kill-switch: TOR obfs4 bridge is not configured".to_string());
            };
            if endpoint_reachable(&host, port, ROUTE_HEALTH_TIMEOUT_MS) {
                None
            } else {
                Some("Kill-switch: TOR obfs4 bridge endpoint is unavailable".to_string())
            }
        }
        "snowflake" | "meek" | "none" => None,
        _ => Some("Kill-switch: TOR transport is unsupported".to_string()),
    }
}

fn endpoint_reachable(host: &str, port: u16, timeout_ms: u64) -> bool {
    if host.trim().is_empty() || port == 0 {
        return false;
    }
    let mut addrs = match (host, port).to_socket_addrs() {
        Ok(value) => value,
        Err(_) => return false,
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms.max(1))).is_ok()
}

fn udp_endpoint_reachable(host: &str, port: u16, timeout_ms: u64) -> bool {
    if host.trim().is_empty() || port == 0 {
        return false;
    }
    let mut addrs = match (host, port).to_socket_addrs() {
        Ok(value) => value,
        Err(_) => return false,
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    let bind_addr = if addr.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let socket = match UdpSocket::bind(bind_addr) {
        Ok(value) => value,
        Err(_) => return false,
    };
    if socket
        .set_write_timeout(Some(Duration::from_millis(timeout_ms.max(1))))
        .is_err()
    {
        return false;
    }
    if socket.connect(addr).is_err() {
        return false;
    }
    socket.send(&[0u8]).is_ok()
}

fn vpn_endpoint_reachable(protocol: VpnProtocol, host: &str, port: u16, timeout_ms: u64) -> bool {
    match protocol {
        VpnProtocol::Wireguard | VpnProtocol::Amnezia => {
            udp_endpoint_reachable(host, port, timeout_ms)
        }
        VpnProtocol::Openvpn => {
            endpoint_reachable(host, port, timeout_ms)
                || udp_endpoint_reachable(host, port, timeout_ms)
        }
        _ => endpoint_reachable(host, port, timeout_ms),
    }
}

fn proxy_protocol_label(protocol: ProxyProtocol) -> &'static str {
    match protocol {
        ProxyProtocol::Http => "HTTP",
        ProxyProtocol::Socks4 => "SOCKS4",
        ProxyProtocol::Socks5 => "SOCKS5",
        ProxyProtocol::Shadowsocks => "SHADOWSOCKS",
        ProxyProtocol::Vmess => "VMESS",
        ProxyProtocol::Vless => "VLESS",
        ProxyProtocol::Trojan => "TROJAN",
    }
}

fn vpn_protocol_label(protocol: VpnProtocol) -> &'static str {
    match protocol {
        VpnProtocol::Wireguard => "WIREGUARD",
        VpnProtocol::Openvpn => "OPENVPN",
        VpnProtocol::Amnezia => "AMNEZIA",
        VpnProtocol::Vmess => "VMESS",
        VpnProtocol::Vless => "VLESS",
        VpnProtocol::Trojan => "TROJAN",
        VpnProtocol::Shadowsocks => "SHADOWSOCKS",
    }
}

fn dns_block_reason(state: &AppState, profile_id: Uuid, host: &str) -> Option<String> {
    let store = state.network_store.lock().ok()?;
    let dns = store.dns.get(&profile_id.to_string())?;
    for domain in &dns.domain_denylist {
        if host_matches(host, domain) {
            return Some("Profile domain denylist".to_string());
        }
    }
    for (_, service) in &dns.selected_services {
        if service_matches_host(host, service) {
            return Some(format!("Blocked service: {service}"));
        }
    }
    for list in &dns.selected_blocklists {
        for domain in &list.domains {
            if host_matches(host, domain) {
                return Some(format!("DNS blocklist: {}", list.list_id));
            }
        }
    }
    None
}

fn service_matches_host(host: &str, service: &str) -> bool {
    let normalized_host = normalize_domain(host);
    if normalized_host.is_empty() {
        return false;
    }
    service_domain_seeds(service)
        .into_iter()
        .any(|domain| host_matches(&normalized_host, &domain))
}

fn global_security_block_reason(state: &AppState, host: &str) -> Option<String> {
    let record = load_global_security_record(state).ok()?;
    for suffix in &record.blocked_domain_suffixes {
        if host_matches(host, suffix) {
            return Some("Global suffix blacklist".to_string());
        }
    }
    for item in record.blocklists {
        if !item.active {
            continue;
        }
        let list_name = item.name;
        for domain in item.domains {
            if host_matches(host, &domain) {
                return Some(format!("Global blocklist: {list_name}"));
            }
        }
    }
    None
}

fn current_route_policy(app_handle: &AppHandle, profile_id: Uuid) -> Option<VpnProxyTabPayload> {
    let state = app_handle.state::<AppState>();
    let store = state.network_store.lock().ok()?;
    store.vpn_proxy.get(&profile_id.to_string()).cloned()
}

fn current_route_mode(
    app_handle: &AppHandle,
    profile_id: Uuid,
    policy: Option<&VpnProxyTabPayload>,
) -> String {
    let state = app_handle.state::<AppState>();
    if profile_route_mode(&state, profile_id) == "direct" {
        return "direct".to_string();
    }
    let global_vpn_enabled = state
        .network_store
        .lock()
        .ok()
        .map(|store| store.global_route_settings.global_vpn_enabled)
        .unwrap_or(false);
    let base_route = if global_vpn_enabled {
        "vpn".to_string()
    } else {
        policy
        .map(|value| value.route_mode.clone())
        .unwrap_or_else(|| "direct".to_string())
    };
    match resolved_route_strategy(app_handle, profile_id) {
        Some(ResolvedNetworkSandboxMode::CompatibilityNative) => {
            format!("{base_route}:compatibility-native")
        }
        Some(ResolvedNetworkSandboxMode::Container) => format!("{base_route}:container"),
        Some(ResolvedNetworkSandboxMode::Blocked) => format!("{base_route}:blocked"),
        _ => base_route,
    }
}

fn resolved_route_strategy(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Option<ResolvedNetworkSandboxMode> {
    let state = app_handle.state::<AppState>();
    let template = selected_route_template(state.inner(), profile_id)?;
    resolve_profile_network_sandbox_mode(state.inner(), profile_id, Some(&template))
        .ok()
        .map(|value| value.mode)
}

fn resolve_sandbox_strategy_reason(state: &AppState, profile_id: Uuid) -> Option<String> {
    let template = selected_route_template(state, profile_id)?;
    resolve_profile_network_sandbox_mode(state, profile_id, Some(&template))
        .ok()
        .map(|value| value.reason)
}

fn resolve_sandbox_adapter_reason(state: &AppState, profile_id: Uuid) -> Option<String> {
    resolve_profile_network_sandbox_view(state, profile_id)
        .ok()
        .map(|value| value.adapter.reason)
}

fn resolved_sandbox_adapter_available(app_handle: &AppHandle, profile_id: Uuid) -> bool {
    let state = app_handle.state::<AppState>();
    resolve_profile_network_sandbox_view(state.inner(), profile_id)
        .ok()
        .map(|value| value.adapter.available)
        .unwrap_or(false)
}

fn selected_route_template(state: &AppState, profile_id: Uuid) -> Option<crate::state::ConnectionTemplate> {
    let store = state.network_store.lock().ok()?;
    let profile_key = profile_id.to_string();
    let profile_route_mode = store
        .vpn_proxy
        .get(&profile_key)
        .map(|value| value.route_mode.trim().to_lowercase())
        .unwrap_or_else(|| "direct".to_string());
    if profile_route_mode == "direct" {
        return None;
    }
    let template_id = if store.global_route_settings.global_vpn_enabled {
        store
            .global_route_settings
            .default_template_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    } else {
        store.profile_template_selection.get(&profile_key).cloned()
    };
    template_id.and_then(|id| store.connection_templates.get(&id).cloned())
}

fn profile_route_mode(state: &AppState, profile_id: Uuid) -> String {
    state
        .network_store
        .lock()
        .ok()
        .and_then(|store| store.vpn_proxy.get(&profile_id.to_string()).cloned())
        .map(|payload| payload.route_mode.trim().to_lowercase())
        .unwrap_or_else(|| "direct".to_string())
}

fn profile_name(app_handle: &AppHandle, profile_id: Uuid) -> String {
    let state = app_handle.state::<AppState>();
    state
        .manager
        .lock()
        .ok()
        .and_then(|manager| manager.get_profile(profile_id).ok())
        .map(|profile| profile.name)
        .unwrap_or_else(|| profile_id.to_string())
}

fn normalize_domain(domain: &str) -> String {
    domain
        .trim()
        .trim_start_matches("*.")
        .trim_start_matches('.')
        .trim_end_matches('.')
        .to_lowercase()
}

fn host_matches(host: &str, rule: &str) -> bool {
    let normalized_rule = normalize_domain(rule);
    if normalized_rule.is_empty() {
        return false;
    }
    host == normalized_rule || host.ends_with(&format!(".{normalized_rule}"))
}

fn split_host_port(value: &str, default_port: u16) -> (String, u16) {
    let raw = value.trim();
    if let Some((host, port)) = raw.rsplit_once(':') {
        if let Ok(port) = port.parse::<u16>() {
            return (normalize_domain(host), port);
        }
    }
    (normalize_domain(raw), default_port)
}

fn parse_first_bridge_endpoint(bridges: &str) -> Option<(String, u16)> {
    for line in bridges
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        if let Some((host, port)) = parts[1].rsplit_once(':') {
            if let Ok(port) = port.parse::<u16>() {
                return Some((host.to_string(), port));
            }
        }
    }
    None
}

fn normalize_http_first_line(first_line: &str, absolute_target: &str) -> (String, u16, String) {
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let version = parts.next_back().unwrap_or("HTTP/1.1");
    if let Some(rest) = absolute_target.strip_prefix("http://") {
        let (authority, path) = rest
            .split_once('/')
            .map(|(host, path)| (host, format!("/{path}")))
            .unwrap_or((rest, "/".to_string()));
        let (host, port) = split_host_port(authority, 80);
        return (host, port, format!("{method} {path} {version}"));
    }
    if let Some(rest) = absolute_target.strip_prefix("https://") {
        let (authority, path) = rest
            .split_once('/')
            .map(|(host, path)| (host, format!("/{path}")))
            .unwrap_or((rest, "/".to_string()));
        let (host, port) = split_host_port(authority, 443);
        return (host, port, format!("{method} {path} {version}"));
    }
    ("".to_string(), 80, first_line.to_string())
}

fn rebuild_header(
    original: &[u8],
    original_first_line: &str,
    rewritten_first_line: &str,
) -> Vec<u8> {
    let mut text = String::from_utf8_lossy(original).to_string();
    text = text.replacen(original_first_line, rewritten_first_line, 1);
    text.into_bytes()
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{load_traffic_log, service_matches_host};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(prefix: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cerbena-{prefix}-{unique}.json"))
    }

    #[test]
    fn vk_service_does_not_match_unrelated_com_domains() {
        assert!(!service_matches_host("duckduckgo.com", "vk_com"));
        assert!(!service_matches_host("myip.com", "vk_com"));
    }

    #[test]
    fn vk_service_matches_vk_domains() {
        assert!(service_matches_host("vk.com", "vk_com"));
        assert!(service_matches_host("m.vk.com", "vk_com"));
    }

    #[test]
    fn corrupt_traffic_log_is_ignored_instead_of_crashing() {
        let path = temp_path("traffic-log-corrupt");
        fs::write(&path, b"[]\ncorrupt").expect("write corrupt traffic log");

        let loaded = load_traffic_log(&path).expect("load traffic log");

        assert!(loaded.is_empty());
        let corrupt_backup = fs::read_dir(path.parent().expect("temp parent"))
            .expect("read temp dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|candidate| {
                candidate
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(|value| {
                        value.contains("traffic-log-corrupt") && value.contains(".corrupt-")
                    })
                    .unwrap_or(false)
            });
        assert!(corrupt_backup.is_some());

        let _ = fs::remove_file(path);
        if let Some(backup) = corrupt_backup {
            let _ = fs::remove_file(backup);
        }
    }
}
