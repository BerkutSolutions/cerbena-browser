use super::*;

pub(crate) fn read_profile_logs_impl(
    app_handle: tauri::AppHandle,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<String>>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    Ok(ok(
        correlation_id,
        read_profile_log_lines(&app_handle, profile_id)?,
    ))
}
