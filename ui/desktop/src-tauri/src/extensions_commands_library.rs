use super::*;

pub(crate) fn list_extension_library_cmd(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let store = state
        .profile_extension_store
        .lock()
        .map_err(|_| "profile extension store lock poisoned".to_string())?;
    profile_extensions::sync_library_assignments_from_profile_store(&mut library, &store);
    let mut normalized = library.clone();
    for item in normalized.items.values_mut() {
        sync_extension_item_legacy_fields(item);
    }
    let json = serde_json::to_string_pretty(&normalized).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, json))
}

pub(crate) fn update_extension_library_item_cmd(
    state: State<AppState>,
    request: UpdateExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let item = library
        .items
        .get_mut(&request.extension_id)
        .ok_or_else(|| "extension not found".to_string())?;
    if let Some(display_name) = request.display_name.filter(|value| !value.trim().is_empty()) {
        item.display_name = display_name;
    }
    if let Some(version) = request.version.filter(|value| !value.trim().is_empty()) {
        item.version = version;
    }
    if let Some(engine_scope) = request.engine_scope.filter(|value| !value.trim().is_empty()) {
        validate_assigned_profiles(&state, &engine_scope, &item.assigned_profile_ids)?;
        item.engine_scope = engine_scope;
    }
    item.store_url = request.store_url.filter(|value| !value.trim().is_empty());
    item.logo_url = request
        .logo_url
        .filter(|value| !value.trim().is_empty())
        .or(item.logo_url.clone());
    if let Some(tags) = request.tags {
        item.tags = normalize_tags(tags);
    }
    if let Some(auto_update_enabled) = request.auto_update_enabled {
        item.auto_update_enabled = auto_update_enabled;
    }
    if let Some(preserve_on_panic_wipe) = request.preserve_on_panic_wipe {
        item.preserve_on_panic_wipe = preserve_on_panic_wipe;
    }
    if let Some(protect_data_from_panic_wipe) = request.protect_data_from_panic_wipe {
        item.protect_data_from_panic_wipe = protect_data_from_panic_wipe;
    }
    sync_extension_item_legacy_fields(item);
    persist_library(&state, &library)?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn update_extension_library_preferences_cmd(
    state: State<AppState>,
    request: UpdateExtensionLibraryPreferencesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    library.auto_update_enabled = request.auto_update_enabled;
    persist_library(&state, &library)?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn refresh_extension_library_updates_cmd(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<RefreshExtensionLibraryUpdatesResponse>, String> {
    let summary = refresh::refresh_extension_library_updates_impl(&state, None)?;
    Ok(ok(correlation_id, summary))
}

pub(crate) fn set_extension_profiles_cmd(
    state: State<AppState>,
    request: SetExtensionProfilesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let item = library
        .items
        .get(&request.extension_id)
        .ok_or_else(|| "extension not found".to_string())?;
    validate_assigned_profiles(&state, &item.engine_scope, &request.assigned_profile_ids)?;
    drop(library);
    profile_extensions::set_library_item_profile_assignments(
        state.inner(),
        &request.extension_id,
        &request.assigned_profile_ids,
    )?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn remove_extension_library_item_cmd(
    state: State<AppState>,
    request: RemoveExtensionRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let removed = if let Some(variant_engine_scope) = request
        .variant_engine_scope
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized_scope = normalize_engine_scope(variant_engine_scope);
        let mut remove_item = false;
        let removed_variant_path;
        {
            let item = library
                .items
                .get_mut(&request.extension_id)
                .ok_or_else(|| "extension not found".to_string())?;
            let mut variants = normalized_extension_variants(item);
            let before = variants.len();
            let mut removed_variant = None;
            variants.retain(|variant| {
                let matches = normalize_engine_scope(&variant.engine_scope) == normalized_scope;
                if matches && removed_variant.is_none() {
                    removed_variant = Some(variant.clone());
                }
                !matches
            });
            if before == variants.len() {
                return Err("extension variant not found".to_string());
            }
            removed_variant_path = removed_variant.and_then(|variant| variant.package_path);
            if variants.is_empty() {
                remove_item = true;
            } else {
                item.package_variants = variants;
                sync_extension_item_legacy_fields(item);
            }
        }
        if remove_item {
            let removed = library.items.remove(&request.extension_id);
            if removed.is_none() {
                delete_extension_package(removed_variant_path.as_deref());
            }
            removed
        } else {
            delete_extension_package(removed_variant_path.as_deref());
            None
        }
    } else {
        library.items.remove(&request.extension_id)
    };
    persist_library(&state, &library)?;
    if let Some(item) = removed {
        profile_extensions::remove_library_item_from_profiles(state.inner(), &item.id)?;
        for variant in normalized_extension_variants(&item) {
            delete_extension_package(variant.package_path.as_deref());
        }
    }
    Ok(ok(correlation_id, true))
}

pub(crate) fn install_extension_cmd(
    state: State<AppState>,
    request: ImportExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    super::import_extension_library_item(state, request, correlation_id)
}

pub(crate) fn enable_extension_cmd(
    state: State<AppState>,
    request: SetExtensionProfilesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    super::set_extension_profiles(state, request, correlation_id)
}

pub(crate) fn disable_extension_cmd(
    state: State<AppState>,
    request: RemoveExtensionRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    super::remove_extension_library_item(state, request, correlation_id)
}

pub(crate) fn process_first_launch_extensions_cmd(
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    first_launch::process_first_launch_extensions_cmd(correlation_id)
}

pub(crate) fn evaluate_extension_policy_cmd(
    request: EvaluateExtensionPolicyRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let runtime_request = PolicyRequest {
        has_profile_context: request.request.has_profile_context,
        vpn_up: request.request.vpn_up,
        target_domain: request.request.target_domain,
        target_service: request.request.target_service,
        tor_up: request.request.tor_up,
        dns_over_tor: request.request.dns_over_tor,
        active_route: request.request.active_route,
    };
    let enforcer = ExtensionPolicyEnforcer::default();
    let guardrails = OverrideGuardrails {
        require_explicit_allow: true,
        allow_service_override: true,
    };
    let decision = enforcer.evaluate(
        &request.policy,
        &runtime_request,
        request.extension_override_allowed,
        &guardrails,
    );
    let payload = serde_json::to_string_pretty(&decision).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, payload))
}
