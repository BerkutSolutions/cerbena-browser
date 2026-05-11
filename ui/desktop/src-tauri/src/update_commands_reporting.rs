use super::*;

pub(crate) fn update_updater_overview_impl<F>(state: &AppState, mut change: F) -> Result<(), String>
where
    F: FnMut(&mut UpdaterOverview),
{
    let overview = {
        let mut runtime = state
            .updater_runtime
            .lock()
            .map_err(|e| format!("lock updater runtime: {e}"))?;
        change(&mut runtime.overview);
        runtime.overview.clone()
    };
    let _ = state.app_handle.emit(UPDATER_EVENT_NAME, &overview);
    Ok(())
}

pub(crate) fn progress_updater_step_impl(
    state: &AppState,
    step_id: &str,
    status: &str,
    detail: &str,
) -> Result<(), String> {
    update_updater_overview_impl(state, |overview| {
        if let Some(step) = overview.steps.iter_mut().find(|item| item.id == step_id) {
            step.status = status.to_string();
            step.detail = detail.to_string();
        }
    })
}

pub(crate) fn mark_remaining_updater_steps_skipped_impl(
    state: &AppState,
    from_step_id: &str,
) -> Result<(), String> {
    let mut should_mark = false;
    update_updater_overview_impl(state, |overview| {
        for step in &mut overview.steps {
            if step.id == from_step_id {
                should_mark = true;
            }
            if should_mark && step.status == "idle" {
                step.status = "skipped".to_string();
                step.detail = "i18n:updater.detail.skipped".to_string();
            }
        }
    })
}

pub(crate) fn finalize_updater_success_impl(
    state: &AppState,
    status: &str,
    summary_key: &str,
    summary_detail: &str,
    close_label_key: &str,
) -> Result<(), String> {
    update_updater_overview_impl(state, |overview| {
        overview.status = status.to_string();
        overview.summary_key = summary_key.to_string();
        overview.summary_detail = summary_detail.to_string();
        overview.finished_at = Some(now_iso());
        overview.can_close = true;
        overview.close_label_key = close_label_key.to_string();
    })?;
    let mut runtime = state
        .updater_runtime
        .lock()
        .map_err(|e| format!("lock updater runtime: {e}"))?;
    runtime.running = false;
    Ok(())
}

pub(crate) fn finalize_updater_failure_impl(state: &AppState, error: &str) -> Result<(), String> {
    update_updater_overview_impl(state, |overview| {
        overview.status = "error".to_string();
        overview.summary_key = "updater.summary.failed".to_string();
        overview.summary_detail = error.to_string();
        overview.finished_at = Some(now_iso());
        overview.can_close = true;
        overview.close_label_key = "action.close".to_string();
    })?;
    let mut runtime = state
        .updater_runtime
        .lock()
        .map_err(|e| format!("lock updater runtime: {e}"))?;
    runtime.running = false;
    Ok(())
}
