use super::*;

pub(crate) fn list_profiles_impl(
    state: &State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<ProfileMetadata>>, String> {
    let hidden = state
        .hidden_default_profiles
        .lock()
        .map_err(|_| "hidden default profiles lock poisoned".to_string())?
        .names
        .clone();
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    ensure_default_profiles(&manager, &hidden)?;
    let list = manager.list_profiles().map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, list))
}
