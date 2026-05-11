use super::*;

pub(crate) fn get_device_posture_report(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<DevicePostureReport>, String> {
    Ok(ok(correlation_id, get_or_refresh_device_posture(&state)?))
}

pub(crate) fn refresh_device_posture_report(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<DevicePostureReport>, String> {
    Ok(ok(correlation_id, refresh_device_posture(&state)?))
}

pub(crate) fn set_default_profile_for_links(
    state: State<AppState>,
    request: DefaultProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "manager lock poisoned".to_string())?;
        let _ = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    }
    let mut store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    store.global_profile_id = Some(profile_id.to_string());
    drop(store);
    links::persist_link_routing_impl(&state)?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn clear_default_profile_for_links(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    store.global_profile_id = None;
    drop(store);
    links::persist_link_routing_impl(&state)?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn get_link_routing_overview(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<LinkRoutingOverview>, String> {
    Ok(ok(correlation_id, links::link_routing_overview_impl(&state)?))
}

pub(crate) fn save_link_type_profile_binding(
    state: State<AppState>,
    request: LinkTypeBindingRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    let link_type = links::normalize_link_type_impl(&request.link_type)
        .ok_or_else(|| "unsupported link type".to_string())?;
    {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "manager lock poisoned".to_string())?;
        let _ = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    }
    let mut store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    store
        .type_bindings
        .insert(link_type, profile_id.to_string());
    drop(store);
    links::persist_link_routing_impl(&state)?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn remove_link_type_profile_binding(
    state: State<AppState>,
    request: LinkTypeRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let link_type = links::normalize_link_type_impl(&request.link_type)
        .ok_or_else(|| "unsupported link type".to_string())?;
    let mut store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    store.type_bindings.remove(&link_type);
    drop(store);
    links::persist_link_routing_impl(&state)?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn dispatch_external_link(
    state: State<AppState>,
    request: DispatchLinkRequest,
    correlation_id: String,
) -> Result<UiEnvelope<DispatchLinkResolution>, String> {
    let link_type = links::detect_link_type_impl(&request.url)?;
    let overview = links::link_routing_overview_impl(&state)?;
    let row = overview
        .supported_types
        .iter()
        .find(|item| item.link_type == link_type)
        .ok_or_else(|| "unsupported link type".to_string())?;
    let (status, target_profile_id, resolution_scope) = if let Some(profile_id) = &row.profile_id {
        (
            "resolved".to_string(),
            Some(profile_id.clone()),
            Some("type".to_string()),
        )
    } else if row.allow_global_default {
        if let Some(profile_id) = &overview.global_profile_id {
            (
                "resolved".to_string(),
                Some(profile_id.clone()),
                Some("global".to_string()),
            )
        } else {
            ("prompt".to_string(), None, None)
        }
    } else if let Some(profile_id) = &overview.global_profile_id {
        (
            "prompt".to_string(),
            Some(profile_id.clone()),
            Some("global-disabled".to_string()),
        )
    } else {
        ("prompt".to_string(), None, None)
    };
    Ok(ok(
        correlation_id,
        DispatchLinkResolution {
            status,
            link_type,
            url: request.url.trim().to_string(),
            target_profile_id,
            resolution_scope,
        },
    ))
}

pub(crate) fn consume_pending_external_link(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Option<String>>, String> {
    let mut pending = state
        .pending_external_link
        .lock()
        .map_err(|_| "pending external link lock poisoned".to_string())?;
    Ok(ok(correlation_id, pending.take()))
}
