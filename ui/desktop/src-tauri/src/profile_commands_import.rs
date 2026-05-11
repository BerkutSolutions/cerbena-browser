use super::*;

pub(crate) fn import_profile_impl(
    state: State<AppState>,
    request: ImportProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ImportProfileResponse>, String> {
    let archive: EncryptedProfileArchive =
        serde_json::from_str(&request.archive_json).map_err(|e| e.to_string())?;
    let expected_id =
        Uuid::parse_str(&request.expected_profile_id).map_err(|e| format!("profile id: {e}"))?;
    let payload = import_profile_archive(&archive, expected_id, &request.passphrase)
        .map_err(|e| e.to_string())?;

    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let imported = manager
        .create_profile(CreateProfileInput {
            name: payload.metadata.name,
            description: payload.metadata.description,
            tags: payload.metadata.tags,
            engine: payload.metadata.engine,
            default_start_page: payload.metadata.default_start_page,
            default_search_provider: payload.metadata.default_search_provider,
            ephemeral_mode: payload.metadata.ephemeral_mode,
            password_lock_enabled: false,
            panic_frame_enabled: payload.metadata.panic_frame_enabled,
            panic_frame_color: payload.metadata.panic_frame_color,
            panic_protected_sites: payload.metadata.panic_protected_sites,
            ephemeral_retain_paths: payload.metadata.ephemeral_retain_paths,
        })
        .map_err(|e| e.to_string())?;

    transfer::write_imported_files_impl(&state.profile_root, imported.id, payload.files)?;
    Ok(ok(correlation_id, ImportProfileResponse { profile: imported }))
}
