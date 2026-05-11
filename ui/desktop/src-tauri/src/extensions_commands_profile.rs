use super::*;

pub(crate) fn list_extensions_cmd(
    state: State<AppState>,
    profile_id: String,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let json = profile_extensions::list_profile_extensions_json(state.inner(), &profile_id)?;
    Ok(ok(correlation_id, json))
}

pub(crate) fn save_profile_extensions_cmd(
    state: State<AppState>,
    request: SaveProfileExtensionsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    profile_extensions::save_profile_extensions(state.inner(), &request.profile_id, request.items)?;
    Ok(ok(correlation_id, true))
}
