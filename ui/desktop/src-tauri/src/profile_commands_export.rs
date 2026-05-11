use super::*;

pub(crate) fn export_profile_impl(
    state: State<AppState>,
    request: ExportProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ExportProfileResponse>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let profile = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    let files = transfer::collect_profile_data_files_impl(&state.profile_root, profile_id)?;
    let archive =
        export_profile_archive(&profile, files, &request.passphrase).map_err(|e| e.to_string())?;
    let archive_json = serde_json::to_string(&archive).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, ExportProfileResponse { archive_json }))
}
