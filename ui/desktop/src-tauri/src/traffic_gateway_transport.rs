use super::*;

pub(crate) fn ensure_profile_gateway_impl(
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
                            if is_expected_client_resolution_error(&err) {
                                return;
                            }
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

pub(crate) fn stop_profile_gateway_impl(app_handle: &AppHandle, profile_id: Uuid) {
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

pub(crate) fn stop_all_profile_gateways_impl(app_handle: &AppHandle) {
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

fn is_expected_client_resolution_error(error: &str) -> bool {
    error.contains("os error 11001")
}
