use super::*;

pub(crate) fn delete_profile_impl(
    state: &State<AppState>,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let deleted_profile = {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "lock poisoned".to_string())?;
        manager.get_profile(profile_id).map_err(|e| e.to_string())?
    };
    if is_builtin_default_profile_name(&deleted_profile.name)
        && deleted_profile.tags.iter().any(|tag| tag == "default")
    {
        let path = state.hidden_default_profiles_path(&state.app_handle)?;
        let mut hidden = state
            .hidden_default_profiles
            .lock()
            .map_err(|_| "hidden default profiles lock poisoned".to_string())?;
        hidden.names.insert(deleted_profile.name.clone());
        persist_hidden_default_profiles_store(&path, &hidden)?;
    }
    super::purge_profile_related_state(state, profile_id)?;
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    manager
        .delete_profile_with_actor(profile_id, "ui")
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}
