use super::*;

pub(crate) fn evaluate_extension_policy_cmd(
    request: EvaluateExtensionPolicyRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    library::evaluate_extension_policy_cmd(request, correlation_id)
}
