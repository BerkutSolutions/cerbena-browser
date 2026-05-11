use super::*;

pub(crate) fn set_profile_password_impl(
    state: State<AppState>,
    request: LockProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    manager
        .set_profile_password(profile_id, &request.password, None)
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}
