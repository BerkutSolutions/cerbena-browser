use super::*;

pub(crate) fn parse_state_impl(state: &str) -> Result<ProfileState, String> {
    match state {
        "created" => Ok(ProfileState::Created),
        "ready" => Ok(ProfileState::Ready),
        "running" => Ok(ProfileState::Running),
        "stopped" => Ok(ProfileState::Stopped),
        "locked" => Ok(ProfileState::Locked),
        "error" => Ok(ProfileState::Error),
        _ => Err(format!("unsupported state: {state}")),
    }
}

pub(crate) fn patch_state_impl(
    state: &State<AppState>,
    request: &ActionProfileRequest,
    correlation_id: String,
    target: ProfileState,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let profile = manager
        .update_profile(
            profile_id,
            PatchProfileInput {
                state: Some(target),
                ..PatchProfileInput::default()
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, profile))
}
