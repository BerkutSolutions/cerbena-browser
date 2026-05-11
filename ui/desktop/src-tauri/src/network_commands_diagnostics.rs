use super::*;

pub(crate) fn ping_connection_template_impl(
    state: State<AppState>,
    request: TemplatePingRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ConnectionHealthView>, String> {
    let store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    let template = store
        .connection_templates
        .get(&request.template_id)
        .ok_or_else(|| "connection template not found".to_string())?
        .clone();
    drop(store);
    let health = test_connection_template_impl(&template)?;
    Ok(ok(correlation_id, health))
}

pub(crate) fn test_connection_template_request_impl(
    request: SaveConnectionTemplateRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ConnectionHealthView>, String> {
    validate_connection_template_request(&request)?;
    let mut template = ConnectionTemplate {
        id: "transient-check".to_string(),
        name: request.name.trim().to_string(),
        nodes: build_nodes_from_request(&request)?,
        connection_type: String::new(),
        protocol: String::new(),
        host: None,
        port: None,
        username: None,
        password: None,
        bridges: None,
        updated_at_epoch_ms: now_epoch_ms(),
    };
    sync_legacy_primary_fields(&mut template);
    let health = test_connection_template_impl(&template)?;
    Ok(ok(correlation_id, health))
}

pub(crate) fn test_vpn_proxy_policy_impl(
    payload: VpnProxyTabPayload,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let (proxy, vpn) = test_connect(&payload, 3_000)?;
    let result = serde_json::json!({
        "validated": true,
        "route_mode": payload.route_mode.clone(),
        "has_proxy": payload.proxy.is_some(),
        "has_vpn": payload.vpn.is_some(),
        "proxy_reachable": proxy.as_ref().map(|value| value.reachable),
        "vpn_reachable": vpn.as_ref().map(|value| value.connected),
        "proxy_message": proxy.as_ref().map(|value| value.message.clone()),
        "vpn_message": vpn.as_ref().map(|value| value.message.clone())
    });
    let json = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, json))
}
