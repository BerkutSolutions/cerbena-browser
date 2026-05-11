use super::*;

pub(crate) fn get_network_state_impl(
    state: State<AppState>,
    profile_id: String,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    let payload = store.vpn_proxy.get(&profile_id).cloned();
    let selected_template_id = store.profile_template_selection.get(&profile_id).cloned();
    let global_route = store.global_route_settings.clone();
    let global_template = global_route
        .default_template_id
        .as_ref()
        .and_then(|id| store.connection_templates.get(id))
        .cloned();
    let connection_templates = store
        .connection_templates
        .values()
        .cloned()
        .map(|mut template| {
            sync_legacy_primary_fields(&mut template);
            template
        })
        .collect::<Vec<_>>();
    drop(store);
    let sandbox = if let Ok(id) = Uuid::parse_str(&profile_id) {
        resolve_profile_network_sandbox_view(state.inner(), id)?
    } else {
        resolve_global_network_sandbox_view(state.inner(), global_template.as_ref())?
    };
    let json = serde_json::to_string_pretty(&NetworkStateView {
        payload,
        selected_template_id,
        connection_templates,
        global_route,
        sandbox,
    })
    .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, json))
}

pub(crate) fn save_connection_template_impl(
    state: State<AppState>,
    request: SaveConnectionTemplateRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ConnectionTemplate>, String> {
    validate_connection_template_request(&request)?;
    let nodes = build_nodes_from_request(&request)?;
    let mut store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    let id = request
        .template_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| build_template_id(&request.name, &store.connection_templates));
    let mut template = ConnectionTemplate {
        id: id.clone(),
        name: request.name.trim().to_string(),
        nodes,
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
    validate_connection_template(&template)?;
    store.connection_templates.insert(id, template.clone());
    let affected_profiles = store
        .profile_template_selection
        .iter()
        .filter(|(_, template_id)| *template_id == &template.id)
        .filter_map(|(profile_id, _)| Uuid::parse_str(profile_id).ok())
        .collect::<Vec<_>>();
    persist_store(&state, &store)?;
    drop(store);
    refresh_running_profiles_route_runtime(&state, &affected_profiles)?;
    Ok(ok(correlation_id, template))
}

pub(crate) fn delete_connection_template_impl(
    state: State<AppState>,
    request: DeleteConnectionTemplateRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    store.connection_templates.remove(&request.template_id);
    let affected_profiles = store
        .profile_template_selection
        .iter()
        .filter(|(_, value)| *value == &request.template_id)
        .filter_map(|(profile_id, _)| Uuid::parse_str(profile_id).ok())
        .collect::<Vec<_>>();
    store
        .profile_template_selection
        .retain(|_, value| value != &request.template_id);
    if store
        .global_route_settings
        .default_template_id
        .as_ref()
        .map(|value| value == &request.template_id)
        .unwrap_or(false)
    {
        store.global_route_settings.default_template_id = None;
    }
    persist_store(&state, &store)?;
    drop(store);
    for profile_id in &affected_profiles {
        stop_profile_network_stack(&state.app_handle, *profile_id);
    }
    refresh_running_profiles_route_runtime(&state, &affected_profiles)?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn save_vpn_proxy_policy_impl(
    state: State<AppState>,
    request: SaveVpnProxyRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    validate_vpn_proxy_tab(&request.payload)?;
    let mut store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    let route_mode = request.payload.route_mode.trim().to_ascii_lowercase();
    if !store.global_route_settings.global_vpn_enabled && route_mode != "direct" {
        let has_template = request
            .selected_template_id
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        if !has_template {
            return Err(
                "selected connection template is required for non-direct route mode".to_string(),
            );
        }
    }
    if let Some(template_id) = request
        .selected_template_id
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        if !store.connection_templates.contains_key(&template_id) {
            return Err("selected connection template not found".to_string());
        }
        store
            .profile_template_selection
            .insert(request.profile_id.clone(), template_id);
    } else {
        store.profile_template_selection.remove(&request.profile_id);
    }
    let profile_id = request.profile_id.clone();
    store.vpn_proxy.insert(request.profile_id, request.payload);
    persist_store(&state, &store)?;
    drop(store);

    if let Ok(profile_uuid) = Uuid::parse_str(&profile_id) {
        let maybe_pid = state
            .launched_processes
            .lock()
            .ok()
            .and_then(|map| map.get(&profile_uuid).copied());
        if let Some(pid) = maybe_pid {
            if is_pid_running(pid) {
                ensure_profile_network_stack(&state.app_handle, profile_uuid)?;
            }
        }
    }
    Ok(ok(correlation_id, true))
}

pub(crate) fn save_global_route_settings_impl(
    state: State<AppState>,
    request: SaveGlobalRouteSettingsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut store = state
        .network_store
        .lock()
        .map_err(|_| "network store lock poisoned".to_string())?;
    let default_template_id = request
        .default_template_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(template_id) = default_template_id.as_deref() {
        if !store.connection_templates.contains_key(template_id) {
            return Err("default connection template not found".to_string());
        }
    }
    store.global_route_settings = NetworkGlobalRouteSettings {
        global_vpn_enabled: request.global_vpn_enabled,
        block_without_vpn: request.block_without_vpn,
        default_template_id,
    };
    persist_store(&state, &store)?;
    drop(store);

    let running_profiles = collect_running_profile_ids(&state)?;
    refresh_running_profiles_route_runtime(&state, &running_profiles)?;
    Ok(ok(correlation_id, true))
}
