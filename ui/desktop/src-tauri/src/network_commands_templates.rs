use super::*;

#[path = "network_commands_templates_core.rs"]
mod core;

pub(crate) fn validate_connection_template_request_impl(
    request: &SaveConnectionTemplateRequest,
) -> Result<(), String> {
    core::validate_connection_template_request_impl(request)
}

pub(crate) fn validate_connection_template_impl(
    template: &ConnectionTemplate,
) -> Result<(), String> {
    core::validate_connection_template_impl(template)
}

pub(crate) fn build_template_id_impl(
    seed: &str,
    existing: &std::collections::BTreeMap<String, ConnectionTemplate>,
) -> String {
    core::build_template_id_impl(seed, existing)
}

pub(crate) fn build_nodes_from_request_impl(
    request: &SaveConnectionTemplateRequest,
) -> Result<Vec<ConnectionNode>, String> {
    core::build_nodes_from_request_impl(request)
}

pub(crate) fn sync_legacy_primary_fields_impl(template: &mut ConnectionTemplate) {
    core::sync_legacy_primary_fields_impl(template)
}

pub(crate) fn now_epoch_ms_impl() -> u128 {
    core::now_epoch_ms_impl()
}

pub(crate) fn test_connection_template_impl_impl(
    template: &ConnectionTemplate,
) -> Result<ConnectionHealthView, String> {
    core::test_connection_template_impl_impl(template)
}
