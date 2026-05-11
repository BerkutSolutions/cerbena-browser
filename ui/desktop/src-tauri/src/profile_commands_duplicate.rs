use super::*;

pub(crate) fn duplicate_profile_impl(
    state: &State<AppState>,
    request: DuplicateProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let source_id = Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let source = manager.get_profile(source_id).map_err(|e| e.to_string())?;
    let created = manager
        .create_profile(CreateProfileInput {
            name: request.new_name,
            description: source.description,
            tags: source.tags,
            engine: source.engine,
            default_start_page: source.default_start_page,
            default_search_provider: source.default_search_provider,
            ephemeral_mode: source.ephemeral_mode,
            password_lock_enabled: false,
            panic_frame_enabled: source.panic_frame_enabled,
            panic_frame_color: source.panic_frame_color,
            panic_protected_sites: source.panic_protected_sites,
            ephemeral_retain_paths: source.ephemeral_retain_paths,
        })
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, created))
}
