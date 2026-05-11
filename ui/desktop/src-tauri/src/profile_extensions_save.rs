use super::*;

pub(crate) fn save_profile_extensions_impl(
    state: &AppState,
    profile_id: &str,
    selections: Vec<ProfileExtensionSelection>,
) -> Result<(), String> {
    let profile_uuid = uuid::Uuid::parse_str(profile_id).map_err(|e| format!("profile id: {e}"))?;
    let profile = {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "profile manager lock poisoned".to_string())?;
        manager.get_profile(profile_uuid).map_err(|e| e.to_string())?
    };
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let mut store = state
        .profile_extension_store
        .lock()
        .map_err(|_| "profile extension store lock poisoned".to_string())?;
    super::apply_profile_extension_selections(state, &mut store, &mut library, &profile, selections)?;
    super::persist_all(state, &store, &library)
}
