use super::*;

pub(crate) fn find_merge_target_extension_id(
    library: &ExtensionLibraryStore,
    display_name: &str,
    requested_engine_scope: &str,
) -> Option<String> {
    library::find_merge_target_extension_id_impl(library, display_name, requested_engine_scope)
}

pub(crate) fn validate_assigned_profiles(
    state: &AppState,
    engine_scope: &str,
    assigned_profile_ids: &[String],
) -> Result<(), String> {
    library::validate_assigned_profiles_impl(state, engine_scope, assigned_profile_ids)
}

pub(crate) fn normalize_engine_scope(value: &str) -> String {
    library::normalize_engine_scope_impl(value)
}
