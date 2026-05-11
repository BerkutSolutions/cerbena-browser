use super::*;

pub(crate) fn pick_certificate_files_impl(
    correlation_id: String,
) -> Result<UiEnvelope<Vec<String>>, String> {
    Ok(ok(correlation_id, dialogs::pick_certificate_files()?))
}
