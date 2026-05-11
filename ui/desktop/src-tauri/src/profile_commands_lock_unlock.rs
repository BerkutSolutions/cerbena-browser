use super::*;

pub(crate) fn unlock_profile_impl(
    state: State<AppState>,
    request: UnlockProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let unlocked = manager
        .unlock_profile(profile_id, &request.password)
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, unlocked))
}
