use super::*;

pub(crate) fn process_first_launch_extensions_cmd(
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    Ok(ok(correlation_id, "[]".to_string()))
}
