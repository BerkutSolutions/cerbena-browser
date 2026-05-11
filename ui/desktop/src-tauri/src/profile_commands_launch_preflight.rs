use super::*;

pub(crate) fn prepare_launch_context_impl(
    app_handle: &tauri::AppHandle,
    state: &State<'_, AppState>,
    request: &ActionProfileRequest,
    profile_id: Uuid,
) -> Result<LaunchContext, String> {
    let launch_url_requested = request
        .launch_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    let profile = {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "lock poisoned".to_string())?;
        manager
            .ensure_unlocked(profile_id)
            .map_err(|_| ERR_LOCKED_REQUIRES_UNLOCK.to_string())?;
        manager.get_profile(profile_id).map_err(|e| e.to_string())?
    };
    append_profile_log(
        app_handle,
        profile_id,
        "launcher",
        format!(
            "Launch requested for profile '{}' engine={}",
            profile.name,
            super::engine_session_key(&profile.engine)
        ),
    );
    let profile_key = profile.id.to_string();
    let _ = crate::extensions_commands::refresh_extension_library_updates_impl(
        state.inner(),
        Some(profile_key.as_str()),
    );
    if let Some(code) = first_launch_blocker(&profile) {
        return Err(code.to_string());
    }
    super::ensure_engine_supports_isolated_certificates(
        state,
        Some(profile.id),
        &profile.engine,
        &profile.tags,
    )?;
    let assessment = assess_profile(&profile);
    let active_extensions =
        profile_extensions::collect_active_profile_extensions(state.inner(), &profile)?;
    if assessment.policy_level == "maximum" && !active_extensions.is_empty() {
        return Err(ERR_MAXIMUM_POLICY_EXTENSIONS_FORBIDDEN.to_string());
    }
    enforce_launch_posture(state, &profile, request.device_posture_ack_id.as_deref())?;

    let profile_root = state.profile_root.join(profile.id.to_string());
    let user_data_dir = profile_root.join("engine-profile");
    fs::create_dir_all(&user_data_dir).map_err(|e| e.to_string())?;
    let identity_policy_hash =
        super::write_profile_identity_policy(state.inner(), profile.id, &profile_root)
            .map_err(|e| e.to_string())?;
    emit_profile_launch_progress(
        app_handle,
        profile.id,
        "preflight",
        "profile.launchProgress.preflight",
        false,
        None,
    );

    let session_engine = super::engine_session_key(&profile.engine).to_string();
    Ok(LaunchContext {
        profile_id,
        launch_url_requested,
        profile,
        profile_root,
        user_data_dir,
        identity_policy_hash,
        session_engine,
    })
}
