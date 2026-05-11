use super::*;

pub(crate) fn list_extension_library_cmd(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    library::list_extension_library_cmd(state, correlation_id)
}
