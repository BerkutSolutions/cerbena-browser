use super::*;

pub(crate) fn open_upstream_stream(
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

pub(crate) fn connect_via_local_socks5_endpoint(
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

pub(crate) fn route_uses_http_proxy(
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

pub(crate) fn connect_via_socks_proxy(
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

pub(crate) fn socks5_connect(
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

pub(crate) fn socks4_connect(
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

#[allow(dead_code)]
pub(crate) fn write_forbidden(
    client: &mut TcpStream,
    host: &str,
    reason: &str,
) -> std::io::Result<()> {
    let body = format!("Blocked by Cerbena gateway: {host}\nReason: {reason}\n");
    let response = format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    client.write_all(response.as_bytes())?;
    client.flush()
}


