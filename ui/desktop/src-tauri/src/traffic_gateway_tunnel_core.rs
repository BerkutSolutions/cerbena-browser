use super::*;

pub(crate) fn read_proxy_request(client: &mut TcpStream) -> Result<ParsedProxyRequest, String> {
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

pub(crate) fn handle_connect_request(
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

pub(crate) fn handle_http_request(
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
            log_bridge_copy_error("upstream->client", &error);
        }
        let _ = client_writer.shutdown(Shutdown::Write);
    });

    let mut upstream_writer = upstream;
    let mut client_reader = client.try_clone()?;
    if let Err(error) = std::io::copy(&mut client_reader, &mut upstream_writer) {
        log_bridge_copy_error("client->upstream", &error);
    }
    let _ = upstream_writer.shutdown(Shutdown::Write);
    let _ = upstream_to_client.join();
    Ok(())
}

fn log_bridge_copy_error(direction: &str, error: &std::io::Error) {
    if is_expected_bridge_disconnect(error) {
        return;
    }
    eprintln!("[traffic-gateway] {direction} bridge failed: {error}");
}

pub(crate) fn is_expected_bridge_disconnect(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::NotConnected
    ) || matches!(error.raw_os_error(), Some(10053 | 10054))
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


#[path = "traffic_gateway_tunnel_core_support.rs"]
mod support;
pub(crate) use support::*;


