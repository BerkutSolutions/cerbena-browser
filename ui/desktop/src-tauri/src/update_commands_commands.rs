use super::*;

pub(crate) fn get_updater_overview_impl(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<UpdaterOverview>, String> {
    let overview = state
        .updater_runtime
        .lock()
        .map_err(|e| format!("lock updater runtime: {e}"))?
        .overview
        .clone();
    Ok(ok(correlation_id, overview))
}

pub(crate) fn start_updater_flow_impl(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<UpdaterOverview>, String> {
    ensure_updater_flow_started(&state)?;
    let overview = state
        .updater_runtime
        .lock()
        .map_err(|e| format!("lock updater runtime: {e}"))?
        .overview
        .clone();
    Ok(ok(correlation_id, overview))
}

pub(crate) fn launch_updater_preview_impl(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    spawn_updater_process(&state.app_handle, UpdaterLaunchMode::Preview)?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn get_launcher_update_state_impl(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    let store = refresh_update_store_snapshot(&state)?;
    Ok(ok(correlation_id, to_view(&store)))
}

pub(crate) fn set_launcher_auto_update_impl(
    state: State<AppState>,
    enabled: bool,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    let mut store = refresh_update_store_snapshot(&state)?;
    store.auto_update_enabled = enabled;
    if store.status.trim().is_empty() {
        store.status = "idle".to_string();
    }
    write_update_store_snapshot(&state, &store)?;
    Ok(ok(correlation_id, to_view(&store)))
}

pub(crate) fn check_launcher_updates_impl(
    state: State<AppState>,
    manual: bool,
    correlation_id: String,
) -> Result<UiEnvelope<AppUpdateView>, String> {
    let view = run_update_cycle(&state, manual)?;
    Ok(ok(correlation_id, view))
}
