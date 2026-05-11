use super::*;

pub(crate) fn create_profile_impl(
    state: &State<AppState>,
    request: CreateProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let engine = super::parse_engine(&request.engine)?;
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile = manager
        .create_profile(CreateProfileInput {
            name: request.name,
            description: request.description,
            tags: request.tags,
            engine,
            default_start_page: request
                .default_start_page
                .or_else(|| super::global_startup_page(state)),
            default_search_provider: request.default_search_provider,
            ephemeral_mode: request.ephemeral_mode,
            password_lock_enabled: request.password_lock_enabled,
            panic_frame_enabled: request.panic_frame_enabled,
            panic_frame_color: request.panic_frame_color,
            panic_protected_sites: request.panic_protected_sites,
            ephemeral_retain_paths: request.ephemeral_retain_paths,
        })
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, profile))
}
