use super::*;

pub(crate) fn set_extension_profiles_cmd(
    state: State<AppState>,
    request: SetExtensionProfilesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    library::set_extension_profiles_cmd(state, request, correlation_id)
}
