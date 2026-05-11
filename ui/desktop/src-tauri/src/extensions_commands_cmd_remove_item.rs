use super::*;

pub(crate) fn remove_extension_library_item_cmd(
    state: State<AppState>,
    request: RemoveExtensionRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    library::remove_extension_library_item_cmd(state, request, correlation_id)
}
