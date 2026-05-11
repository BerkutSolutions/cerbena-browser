use super::*;

pub(crate) fn validate_profile_modal_impl(
    payload: ProfileModalPayload,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    validate_modal_payload(&payload).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}
