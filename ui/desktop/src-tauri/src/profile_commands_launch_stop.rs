use super::*;

pub(crate) fn stop_profile_impl(
    state: State<AppState>,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    drop(manager);

    let profile_root = state.profile_root.join(profile.id.to_string());
    let user_data_dir = profile_root.join("engine-profile");
    let tracked_pid = trusted_session_pid(&state, profile_id)?.or_else(|| {
        let launched = state.launched_processes.lock().ok()?;
        launched.get(&profile_id).copied()
    });
    let pid = tracked_pid.or_else(|| find_profile_process_pid_for_dir(&user_data_dir));
    append_profile_log(
        &state.app_handle,
        profile_id,
        "launcher",
        format!("Stop requested pid={}", pid.unwrap_or_default()),
    );
    terminate_profile_processes(&user_data_dir);
    if let Some(pid) = pid {
        terminate_process_tree(pid);
    }
    close_panic_frame(&state.app_handle, profile_id);
    revoke_launch_session(&state, profile_id, tracked_pid)?;
    stop_profile_network_stack(&state.app_handle, profile_id);
    clear_librewolf_profile_certificates(&state.app_handle, profile_id);
    clear_profile_process(
        &state.app_handle,
        profile_id,
        tracked_pid.unwrap_or(pid.unwrap_or_default()),
        false,
    );

    let result = super::patch_state(&state, &request, correlation_id, ProfileState::Stopped)?;
    let _ = state.app_handle.emit(
        "profile-state-changed",
        serde_json::json!({
            "profileId": profile_id.to_string(),
            "state": "stopped"
        }),
    );
    Ok(result)
}
