use super::*;

pub(crate) fn run_update_cycle_impl(state: &AppState, manual: bool) -> Result<AppUpdateView, String> {
    let latest_release_url = resolve_latest_release_api_url();
    push_runtime_log(
        state,
        format!(
            "[updater] check start manual={} version={} url={} user_agent={}",
            manual, CURRENT_VERSION, latest_release_url, USER_AGENT
        ),
    );
    let client = build_release_http_client(Duration::from_secs(20), false)
        .map_err(|e| format!("build update http client: {e}"))?;

    let result = fetch_latest_release_from_url(&client, &latest_release_url);
    let mut store = refresh_update_store_snapshot(state)?;

    store.last_checked_at = Some(now_iso());
    store.last_checked_epoch_ms = Some(now_epoch_ms());

    match result {
        Ok(candidate) => {
            push_runtime_log(
                state,
                format!(
                    "[updater] latest release discovered version={} release_url={}",
                    candidate.version, candidate.release_url
                ),
            );
            push_runtime_log(
                state,
                format!(
                    "[updater] selected asset type={} handoff={} reason={}",
                    candidate.asset_type.as_deref().unwrap_or("unknown"),
                    candidate
                        .install_handoff_mode
                        .as_deref()
                        .unwrap_or("unknown"),
                    candidate
                        .asset_selection_reason
                        .as_deref()
                        .unwrap_or("unspecified")
                ),
            );
            store.latest_version = Some(candidate.version.clone());
            store.release_url = Some(candidate.release_url.clone());
            store.has_update = is_version_newer(&candidate.version, CURRENT_VERSION);
            store.selected_asset_type = candidate.asset_type.clone();
            store.selected_asset_reason = candidate.asset_selection_reason.clone();
            store.install_handoff_mode = candidate.install_handoff_mode.clone();
            store.last_error = None;
            if store.has_update {
                store.status = "available".to_string();
                if manual {
                    match spawn_updater_process(&state.app_handle, UpdaterLaunchMode::Auto) {
                        Ok(()) => {
                            store.updater_handoff_version = Some(candidate.version.clone());
                            store.status = "handoff".to_string();
                            push_runtime_log(
                                state,
                                format!(
                                    "[updater] handoff started version={} mode=manual",
                                    candidate.version
                                ),
                            );
                        }
                        Err(error) => {
                            push_runtime_log(state, format!("[updater] handoff failed: {error}"));
                            store.status = "error".to_string();
                            store.last_error = Some(error);
                        }
                    }
                } else if store.auto_update_enabled {
                    if should_launch_external_updater(&store, &candidate) {
                        match spawn_updater_process(&state.app_handle, UpdaterLaunchMode::Auto) {
                            Ok(()) => {
                                store.updater_handoff_version = Some(candidate.version.clone());
                                store.status = "handoff".to_string();
                                push_runtime_log(
                                    state,
                                    format!(
                                        "[updater] handoff started version={} mode=auto",
                                        candidate.version
                                    ),
                                );
                            }
                            Err(error) => {
                                push_runtime_log(
                                    state,
                                    format!("[updater] handoff failed: {error}"),
                                );
                                store.status = "error".to_string();
                                store.last_error = Some(error);
                            }
                        }
                    } else {
                        if let Err(error) = stage_release_if_needed(state, &mut store, &candidate) {
                            push_runtime_log(
                                state,
                                format!("[updater] stage release failed: {error}"),
                            );
                            store.status = "error".to_string();
                            store.last_error = Some(error);
                        }
                    }
                }
            } else {
                push_runtime_log(
                    state,
                    format!(
                        "[updater] no update available current={} latest={}",
                        CURRENT_VERSION, candidate.version
                    ),
                );
                clear_staged_update(&mut store);
                store.updater_handoff_version = None;
                store.status = "up_to_date".to_string();
            }
        }
        Err(error) => {
            push_runtime_log(state, format!("[updater] check failed: {error}"));
            store.status = "error".to_string();
            store.last_error = Some(error);
        }
    }

    write_update_store_snapshot(state, &store)?;
    Ok(to_view(&store))
}
