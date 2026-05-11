use super::*;

pub(crate) fn update_extension_library_item_cmd(
    state: State<AppState>,
    request: UpdateExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    library::update_extension_library_item_cmd(state, request, correlation_id)
}
