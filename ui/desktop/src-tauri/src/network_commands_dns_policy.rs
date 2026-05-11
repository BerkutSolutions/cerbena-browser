use super::*;

pub(crate) fn save_dns_policy_impl(
    state: State<AppState>,
    request: SaveDnsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut payload = request.payload;
    hydrate_dns_blocklists_from_global_security(&state, &mut payload)?;

    let catalog = state
        .service_catalog
        .lock()
        .map_err(|_| "catalog lock poisoned".to_string())?;
    validate_dns_tab(&payload, Some(&catalog))?;
    drop(catalog);

    let mut store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    store.dns.insert(request.profile_id, payload);
    persist_store(&state, &store)?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn get_service_catalog_impl(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let catalog = state
        .service_catalog
        .lock()
        .map_err(|_| "catalog lock poisoned".to_string())?;
    let json = serde_json::to_string_pretty(&catalog.state).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, json))
}

pub(crate) fn set_service_block_all_impl(
    state: State<AppState>,
    category: String,
    block_all: bool,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut catalog = state
        .service_catalog
        .lock()
        .map_err(|_| "catalog lock poisoned".to_string())?;
    catalog
        .set_category_block_all(&category, block_all)
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn set_service_allowed_impl(
    state: State<AppState>,
    category: String,
    service: String,
    allowed: bool,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut catalog = state
        .service_catalog
        .lock()
        .map_err(|_| "catalog lock poisoned".to_string())?;
    catalog
        .set_service_allowed(&category, &service, allowed)
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn evaluate_network_policy_demo_impl(
    policy: NetworkPolicy,
    request: PolicyRequestInput,
    correlation_id: String,
) -> Result<UiEnvelope<PolicyDecisionView>, String> {
    let runtime_request = PolicyRequest {
        has_profile_context: request.has_profile_context,
        vpn_up: request.vpn_up,
        target_domain: request.target_domain,
        target_service: request.target_service,
        tor_up: request.tor_up,
        dns_over_tor: request.dns_over_tor,
        active_route: request.active_route,
    };
    let engine = NetworkPolicyEngine;
    let decision = engine
        .evaluate(&policy, &runtime_request)
        .map_err(|e| e.to_string())?;
    Ok(ok(
        correlation_id,
        PolicyDecisionView {
            action: format!("{:?}", decision.action),
            reason_code: decision.reason_code,
            selected_route: match decision.selected_route {
                RouteMode::Direct => "direct".to_string(),
                RouteMode::Proxy => "proxy".to_string(),
                RouteMode::Vpn => "vpn".to_string(),
                RouteMode::Tor => "tor".to_string(),
                RouteMode::Hybrid => "hybrid".to_string(),
            },
            matched_rules: decision.matched_rules,
        },
    ))
}
