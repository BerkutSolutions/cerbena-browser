use super::*;

pub(crate) fn run_preview_updater_flow_impl(state: &AppState) -> Result<(), String> {
    update_updater_overview(state, |overview| {
        overview.target_version = Some(format!("{CURRENT_VERSION}-preview"));
        overview.release_url = Some(REPOSITORY_URL.to_string());
        overview.summary_key = "updater.summary.preview_running".to_string();
        overview.summary_detail = "i18n:updater.detail.preview_running".to_string();
    })?;
    progress_updater_step(
        state,
        UPDATER_STEP_DISCOVER,
        "running",
        "i18n:updater.detail.preview_discover_running",
    )?;
    thread::sleep(Duration::from_millis(260));
    progress_updater_step(
        state,
        UPDATER_STEP_DISCOVER,
        "done",
        "i18n:updater.detail.preview_discover_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_COMPARE,
        "running",
        "i18n:updater.detail.preview_compare_running",
    )?;
    thread::sleep(Duration::from_millis(260));
    progress_updater_step(
        state,
        UPDATER_STEP_COMPARE,
        "done",
        "i18n:updater.detail.preview_compare_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_SECURITY,
        "running",
        "i18n:updater.detail.preview_security_running",
    )?;
    thread::sleep(Duration::from_millis(260));
    progress_updater_step(
        state,
        UPDATER_STEP_SECURITY,
        "done",
        "i18n:updater.detail.preview_security_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_DOWNLOAD,
        "running",
        "i18n:updater.detail.preview_download_running",
    )?;
    thread::sleep(Duration::from_millis(260));
    progress_updater_step(
        state,
        UPDATER_STEP_DOWNLOAD,
        "done",
        "i18n:updater.detail.preview_download_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_CHECKSUM,
        "running",
        "i18n:updater.detail.preview_checksum_running",
    )?;
    thread::sleep(Duration::from_millis(260));
    progress_updater_step(
        state,
        UPDATER_STEP_CHECKSUM,
        "done",
        "i18n:updater.detail.preview_checksum_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_INSTALL,
        "done",
        "i18n:updater.detail.preview_install_done",
    )?;
    progress_updater_step(
        state,
        UPDATER_STEP_RELAUNCH,
        "done",
        "i18n:updater.detail.preview_relaunch_done",
    )?;
    finalize_updater_success(
        state,
        "completed",
        "updater.summary.preview_complete",
        "i18n:updater.detail.preview_complete",
        "action.close",
    )
}

pub(crate) fn run_live_updater_flow_impl(state: &AppState) -> Result<(), String> {
    let launch_mode = updater_launch_mode_from_state(state)?;
    push_runtime_log(
        state,
        format!(
            "[updater] live flow start current_version={} launch_mode={:?}",
            CURRENT_VERSION, launch_mode
        ),
    );
    progress_updater_step(
        state,
        UPDATER_STEP_DISCOVER,
        "running",
        "i18n:updater.detail.live_discover_running",
    )?;
    let client = build_release_http_client(Duration::from_secs(30), false)
        .map_err(|e| format!("build updater client: {e}"))?;
    let candidate = fetch_latest_release(&client)?;
    push_runtime_log(
        state,
        format!(
            "[updater] live release discovered version={} asset={} asset_type={} handoff={} reason={} release_url={}",
            candidate.version,
            candidate.asset_name.as_deref().unwrap_or("missing"),
            candidate.asset_type.as_deref().unwrap_or("unknown"),
            candidate.install_handoff_mode.as_deref().unwrap_or("unknown"),
            candidate
                .asset_selection_reason
                .as_deref()
                .unwrap_or("unspecified"),
            candidate.release_url
        ),
    );
    update_updater_overview(state, |overview| {
        overview.target_version = Some(candidate.version.clone());
        overview.release_url = Some(candidate.release_url.clone());
        overview.summary_key = "updater.summary.running".to_string();
        overview.summary_detail = "i18n:updater.detail.live_running".to_string();
    })?;
    progress_updater_step(
        state,
        UPDATER_STEP_DISCOVER,
        "done",
        "i18n:updater.detail.live_discover_done",
    )?;

    progress_updater_step(
        state,
        UPDATER_STEP_COMPARE,
        "running",
        "i18n:updater.detail.live_compare_running",
    )?;
    if !is_version_newer(&candidate.version, CURRENT_VERSION) {
        push_runtime_log(
            state,
            format!(
                "[updater] live compare up_to_date current={} latest={}",
                CURRENT_VERSION, candidate.version
            ),
        );
        progress_updater_step(
            state,
            UPDATER_STEP_COMPARE,
            "done",
            "i18n:updater.detail.live_compare_uptodate",
        )?;
        mark_remaining_updater_steps_skipped(state, UPDATER_STEP_SECURITY)?;
        return finalize_updater_success(
            state,
            "up_to_date",
            "updater.summary.up_to_date",
            "i18n:updater.detail.live_up_to_date",
            "action.close",
        );
    }
    progress_updater_step(
        state,
        UPDATER_STEP_COMPARE,
        "done",
        "i18n:updater.detail.live_compare_done",
    )?;

    progress_updater_step(
        state,
        UPDATER_STEP_SECURITY,
        "running",
        "i18n:updater.detail.live_security_running",
    )?;
    let security_bundle = verify_release_security_bundle(&candidate)?;
    push_runtime_log(
        state,
        format!(
            "[updater] release security verified checksums_bytes={}",
            security_bundle.checksums_text.len()
        ),
    );
    progress_updater_step(
        state,
        UPDATER_STEP_SECURITY,
        "done",
        "i18n:updater.detail.live_security_done",
    )?;

    let asset_name = candidate
        .asset_name
        .clone()
        .ok_or_else(|| "release asset name is missing".to_string())?;
    let asset_url = candidate
        .asset_url
        .clone()
        .ok_or_else(|| "release asset url is missing".to_string())?;
    progress_updater_step(
        state,
        UPDATER_STEP_DOWNLOAD,
        "running",
        "i18n:updater.detail.live_download_running",
    )?;
    let asset_bytes = download_release_bytes(&client, &asset_url, "release asset")?;
    push_runtime_log(
        state,
        format!(
            "[updater] asset downloaded name={} bytes={} url={}",
            asset_name,
            asset_bytes.len(),
            asset_url
        ),
    );
    progress_updater_step(
        state,
        UPDATER_STEP_DOWNLOAD,
        "done",
        "i18n:updater.detail.live_download_done",
    )?;

    progress_updater_step(
        state,
        UPDATER_STEP_CHECKSUM,
        "running",
        "i18n:updater.detail.live_checksum_running",
    )?;
    ensure_asset_matches_verified_checksum(&security_bundle, &asset_name, &asset_bytes)?;
    push_runtime_log(
        state,
        format!(
            "[updater] checksum verified asset={} sha256={}",
            asset_name,
            sha256_hex(&asset_bytes)
        ),
    );
    progress_updater_step(
        state,
        UPDATER_STEP_CHECKSUM,
        "done",
        "i18n:updater.detail.live_checksum_done",
    )?;

    progress_updater_step(
        state,
        UPDATER_STEP_INSTALL,
        "running",
        "i18n:updater.detail.live_install_running",
    )?;
    let staged_path = stage_verified_release_asset(state, &candidate, &asset_bytes)?;
    push_runtime_log(
        state,
        format!(
            "[updater] asset staged version={} asset={} path={} auto_apply={}",
            candidate.version,
            asset_name,
            staged_path.display(),
            can_auto_apply_asset(&asset_name)
        ),
    );
    progress_updater_step(
        state,
        UPDATER_STEP_INSTALL,
        "done",
        "i18n:updater.detail.live_install_done",
    )?;

    progress_updater_step(
        state,
        UPDATER_STEP_RELAUNCH,
        "done",
        "i18n:updater.detail.live_relaunch_done",
    )?;
    if can_auto_apply_asset(&asset_name) {
        push_runtime_log(
            state,
            format!(
                "[updater] flow ready_to_restart version={} asset={} awaiting close handoff",
                candidate.version, asset_name
            ),
        );
        let result = finalize_updater_success(
            state,
            "ready_to_restart",
            "updater.summary.ready_to_restart",
            "i18n:updater.detail.live_ready_to_restart",
            "updater.action.reboot",
        );
        if result.is_ok() && should_auto_close_updater_after_ready_to_restart(launch_mode) {
            schedule_updater_window_close_for_apply(state);
        }
        return result;
    }
    push_runtime_log(
        state,
        format!(
            "[updater] flow completed without auto-apply version={} asset={}",
            candidate.version, asset_name
        ),
    );
    finalize_updater_success(
        state,
        "completed",
        "updater.summary.complete",
        "i18n:updater.detail.live_complete",
        "action.close",
    )
}


#[path = "update_commands_flow_core_cycle.rs"]
mod support;
pub(crate) use support::*;


