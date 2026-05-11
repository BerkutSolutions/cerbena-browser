use super::*;

pub(crate) fn runtime_proxy_endpoint_impl(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Option<(String, u16)> {
    session::runtime_proxy_endpoint_impl(app_handle, profile_id)
}

pub(crate) fn runtime_session_active_impl(app_handle: &AppHandle, profile_id: Uuid) -> bool {
    session::runtime_session_active_impl(app_handle, profile_id)
}

pub(crate) fn runtime_session_snapshot_impl(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> Option<RouteRuntimeSessionSnapshot> {
    session::runtime_session_snapshot_impl(app_handle, profile_id)
}

pub(crate) fn session_is_active_impl(session: &RouteRuntimeSession) -> bool {
    session::session_is_active_impl(session)
}

pub(crate) fn route_runtime_required_for_profile_impl(
    app_handle: &AppHandle,
    profile_id: Uuid,
) -> bool {
    session::route_runtime_required_for_profile_impl(app_handle, profile_id)
}

pub(crate) fn stop_profile_route_runtime_impl(app_handle: &AppHandle, profile_id: Uuid) {
    session::stop_profile_route_runtime_impl(app_handle, profile_id)
}

pub(crate) fn route_runtime_backend_label_impl(backend: RouteRuntimeBackend) -> &'static str {
    session::route_runtime_backend_label_impl(backend)
}

pub(crate) fn stop_all_route_runtime_impl(app_handle: &AppHandle) {
    session::stop_all_route_runtime_impl(app_handle)
}

pub(crate) fn profile_route_runtime_needs_download_impl(
    app_handle: &AppHandle,
    required_tools: &BTreeSet<NetworkTool>,
) -> bool {
    selection::profile_route_runtime_needs_download_impl(app_handle, required_tools)
}

pub(crate) fn resolve_effective_route_selection_impl(
    store: &crate::state::NetworkStore,
    profile_key: &str,
) -> (String, Option<String>) {
    selection::resolve_effective_route_selection_impl(store, profile_key)
}
