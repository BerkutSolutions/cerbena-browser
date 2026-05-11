use super::*;

pub(crate) fn update_profile_impl(
    state: &State<AppState>,
    request: UpdateProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let current = {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "lock poisoned".to_string())?;
        manager.get_profile(profile_id).map_err(|e| e.to_string())?
    };
    let next_engine = request
        .engine
        .as_deref()
        .map(super::parse_engine)
        .transpose()?
        .unwrap_or_else(|| current.engine.clone());
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let default_start_page =
        super::parse_nullable_string_field(request.default_start_page, "defaultStartPage")?;
    let default_search_provider =
        super::parse_nullable_string_field(request.default_search_provider, "defaultSearchProvider")?;
    let patch = PatchProfileInput {
        name: request.name,
        description: request.description.map(Some),
        tags: request.tags,
        engine: request.engine.as_deref().map(super::parse_engine).transpose()?,
        state: request.state.map(|v| super::parse_state(&v)).transpose()?,
        default_start_page,
        default_search_provider,
        ephemeral_mode: request.ephemeral_mode,
        password_lock_enabled: request.password_lock_enabled,
        panic_frame_enabled: request.panic_frame_enabled,
        panic_frame_color: request.panic_frame_color.map(Some),
        panic_protected_sites: request.panic_protected_sites,
        ephemeral_retain_paths: request.ephemeral_retain_paths,
    };
    let profile = manager
        .update_profile_with_actor(profile_id, patch, request.expected_updated_at.as_deref(), "ui")
        .map_err(|e| e.to_string())?;
    drop(manager);
    if current.engine != next_engine {
        super::reset_profile_runtime_workspace(state, profile_id)?;
    }
    Ok(ok(correlation_id, profile))
}
