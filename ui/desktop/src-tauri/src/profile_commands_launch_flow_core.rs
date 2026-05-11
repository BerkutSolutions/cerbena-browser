use super::*;

const LIBREWOLF_DUPLICATE_LAUNCH_GUARD_WINDOW: Duration = Duration::from_secs(45);

pub(crate) async fn launch_profile_impl(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let previous_launch_attempt = claim_recent_launch_slot(&state, profile_id)?;
    let result = async {
        let context = preflight::prepare_launch_context_impl(
            &app_handle,
            &state,
            &request,
            profile_id,
        )?;
        if should_suppress_duplicate_librewolf_launch(&context, previous_launch_attempt) {
            eprintln!(
                "[profile-launch] duplicate launch suppressed profile={} engine={:?} reason=recent-attempt-with-lock",
                context.profile.id,
                context.profile.engine
            );
            append_profile_log(
                &app_handle,
                context.profile.id,
                "launcher",
                "Suppressed duplicate launch attempt while profile lock is still active".to_string(),
            );
            super::emit_profile_launch_progress(
                &app_handle,
                context.profile.id,
                "done",
                "profile.launchProgress.done",
                true,
                None,
            );
            return super::patch_state(&state, &request, correlation_id, ProfileState::Running);
        }
        if let Some(result) = session::reuse_tracked_session_impl(
            &app_handle,
            &state,
            &request,
            &correlation_id,
            &context,
        )? {
            return Ok(result);
        }
        eprintln!(
            "[profile-launch] start profile={} engine={:?} correlation_id={} launch_url_requested={} profile_root={} user_data_dir={}",
            context.profile.id,
            context.profile.engine,
            correlation_id,
            context.launch_url_requested,
            context.profile_root.display(),
            context.user_data_dir.display()
        );
        if matches!(context.profile.engine, Engine::Librewolf) {
            super::emit_librewolf_launch_diagnostics("startup", &context.user_data_dir);
        }
        if let Some(result) = session::reuse_discovered_session_impl(
            &app_handle,
            &state,
            &request,
            &correlation_id,
            &context,
        )? {
            return Ok(result);
        }

        let runtime =
            EngineRuntime::new(state.engine_runtime_root.clone()).map_err(|e| e.to_string())?;
        let engine = super::engine_kind(context.profile.engine.clone());
        super::emit_profile_launch_progress(
            &app_handle,
            context.profile.id,
            "policy",
            "profile.launchProgress.policy",
            false,
            None,
        );
        launch_policy::apply_launch_policies_impl(
            &state,
            &context.profile,
            &context.profile_root,
            engine,
            None,
        )?;
        launch_extensions::prepare_extensions_for_launch_impl(
            &state,
            &context.profile,
            &context.profile_root,
            engine,
            &app_handle,
        )?;
        let mut prelaunch_profile_pids = std::collections::BTreeSet::new();
        super::emit_profile_launch_progress(
            &app_handle,
            context.profile.id,
            "network",
            "profile.launchProgress.network",
            false,
            None,
        );
        let network_handle = app_handle.clone();
        let network_profile_id = context.profile.id;
        let gateway = tauri::async_runtime::spawn_blocking(move || {
            ensure_profile_network_stack(&network_handle, network_profile_id)
        })
        .await
        .map_err(|e| e.to_string())??;
        eprintln!(
            "[profile-launch][trace] profile={} step=network-stack-ready gateway_port={}",
            context.profile_id, gateway.port
        );
        append_profile_log(
            &app_handle,
            context.profile_id,
            "network",
            format!("Profile gateway ready on 127.0.0.1:{}", gateway.port),
        );
        let gateway_port = Some(gateway.port);
        let runtime_hardening = assess_profile(&context.profile).runtime_hardening;
        if matches!(engine, EngineKind::Librewolf | EngineKind::FirefoxEsr) {
            super::emit_profile_launch_progress(
                &app_handle,
                context.profile.id,
                "profile-runtime",
                "profile.launchProgress.profileRuntime",
                false,
                None,
            );
            super::prepare_librewolf_profile_runtime_impl(
                &context.user_data_dir,
                context.profile.default_start_page.as_deref(),
                context.profile.default_search_provider.as_deref(),
                gateway_port,
                runtime_hardening,
                super::load_identity_preset_for_profile_impl(&state, context.profile.id).as_ref(),
            )
            .map_err(|e| e.to_string())?;
            let user_js = context.user_data_dir.join("user.js");
            eprintln!(
                "[profile-launch] firefox-family preflight user.js_exists={} user.js_path={}",
                user_js.exists(),
                user_js.display()
            );
            if matches!(engine, EngineKind::Librewolf) {
                super::emit_librewolf_launch_diagnostics("post-prepare-runtime", &context.user_data_dir);
            }
            let existing_pids =
                crate::process_tracking::find_profile_process_pids_for_dir(&context.user_data_dir);
            prelaunch_profile_pids.extend(existing_pids.iter().copied());
            if !existing_pids.is_empty() {
                if context.launch_url_requested {
                    eprintln!(
                        "[profile-launch] librewolf existing process kept for launch_url pids={:?} profile_dir={}",
                        existing_pids,
                        context.user_data_dir.display()
                    );
                } else {
                    eprintln!(
                        "[profile-launch] librewolf existing profile processes detected pids={:?} profile_dir={}",
                        existing_pids,
                        context.user_data_dir.display()
                    );
                    for existing_pid in existing_pids {
                        terminate_process_tree(existing_pid);
                    }
                    let wait_started = Instant::now();
                    while wait_started.elapsed() < Duration::from_millis(2200) {
                        if find_profile_process_pid_for_dir(&context.user_data_dir).is_none() {
                            break;
                        }
                        thread::sleep(Duration::from_millis(180));
                    }
                }
            }
        }
        super::emit_profile_launch_progress(
            &app_handle,
            context.profile.id,
            "engine",
            if runtime.installed(engine).map_err(|e| e.to_string())?.is_some() {
                "profile.launchProgress.engine"
            } else {
                "profile.launchProgress.engineDownload"
            },
            false,
            None,
        );
        let installation = super::ensure_engine_ready(&app_handle, &state, &runtime, engine).await?;
        launch_policy::apply_launch_policies_impl(
            &state,
            &context.profile,
            &context.profile_root,
            engine,
            Some(&installation.binary_path),
        )?;
        let has_restore_session =
            super::profile_runtime_has_session_state_impl(engine, &context.user_data_dir);
        eprintln!(
            "[profile-launch][trace] profile={} step=session-restore-check done has_restore_session={}",
            context.profile_id, has_restore_session
        );
        let launch_plan = launch_plan::build_launch_plan_impl(
            engine,
            request.launch_url.as_deref(),
            context.profile.default_start_page.as_deref(),
            has_restore_session,
        );
        eprintln!(
            "[profile-launch] resolved start_page profile={} engine={:?} explicit_launch_url={:?} profile_default_start_page={:?} runtime_start_page={:?} has_restore_session={}",
            context.profile.id,
            context.profile.engine,
            request.launch_url,
            context.profile.default_start_page,
            launch_plan.start_page,
            has_restore_session
        );
        if matches!(engine, EngineKind::Librewolf) {
            super::emit_librewolf_launch_diagnostics("pre-spawn", &context.user_data_dir);
        }
        let private_mode = context.profile.ephemeral_mode
            && context
                .profile
                .tags
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case("private"));

        let launch_runtime = runtime.clone();
        let launch_root = context.profile_root.clone();
        let gateway_port = gateway_port;
        super::emit_profile_launch_progress(
            &app_handle,
            context.profile.id,
            "browser",
            "profile.launchProgress.browser",
            false,
            None,
        );
        let pid_result = tauri::async_runtime::spawn_blocking(move || {
            launch_runtime.launch(
                engine,
                launch_root,
                context.profile_id,
                launch_plan.start_page,
                private_mode,
                gateway_port,
                runtime_hardening,
            )
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string());
        eprintln!(
            "[profile-launch][trace] profile={} step=engine-launch-finished ok={}",
            context.profile_id,
            pid_result.is_ok()
        );
        if pid_result.is_err() && matches!(engine, EngineKind::Librewolf) {
            clear_librewolf_profile_certificates(&app_handle, context.profile_id);
        }
        let pid = pid_result?;
        super::persist_identity_applied_marker(
            &context.profile_root,
            &context.session_engine,
            context.identity_policy_hash.as_deref(),
        )
        .map_err(|e| e.to_string())?;
        let tracked_pid = super::wait_for_profile_process_startup_impl(
            &context.user_data_dir,
            pid,
            engine,
            &prelaunch_profile_pids,
        )
        .map_err(|error| {
            append_profile_log(&app_handle, context.profile_id, "launcher", error.clone());
            error
        })?;
        if matches!(engine, EngineKind::Librewolf) {
            super::emit_librewolf_launch_diagnostics("post-startup-detection", &context.user_data_dir);
        }
        let mut launched = state
            .launched_processes
            .lock()
            .map_err(|_| "launch map lock poisoned".to_string())?;
        launched.insert(context.profile_id, tracked_pid);
        drop(launched);

        track_profile_process(
            app_handle.clone(),
            context.profile_id,
            tracked_pid,
            context.user_data_dir.clone(),
            prelaunch_profile_pids.clone(),
        );
        maybe_start_panic_frame(&app_handle, context.profile_id, tracked_pid);
        issue_launch_session(
            &state,
            context.profile_id,
            tracked_pid,
            &context.session_engine,
            &context.profile_root,
            &context.user_data_dir,
        )?;
        if matches!(engine, EngineKind::Librewolf) {
            if let Some(url) = launch_plan.post_launch_url.as_deref() {
                append_profile_log(
                    &app_handle,
                    context.profile_id,
                    "launcher",
                    format!("Opening LibreWolf post-launch URL in running profile: {url}"),
                );
                super::open_url_in_running_profile(
                    state.inner(),
                    &context.profile,
                    &context.profile_root,
                    url,
                )?;
            }
        }

        append_profile_log(
            &app_handle,
            context.profile_id,
            "launcher",
            format!("Browser launched successfully pid={tracked_pid}"),
        );

        let _ = installation;
        let _ = app_handle.emit(
            "profile-state-changed",
            serde_json::json!({
                "profileId": context.profile_id.to_string(),
                "state": "running"
            }),
        );
        super::emit_profile_launch_progress(
            &app_handle,
            context.profile.id,
            "done",
            "profile.launchProgress.done",
            true,
            None,
        );

        super::patch_state(&state, &request, correlation_id, ProfileState::Running)
    }
    .await;
    if let Err(error) = &result {
        append_profile_log(
            &app_handle,
            profile_id,
            "launcher",
            format!("Launch failed: {error}"),
        );
    }
    result
}

fn claim_recent_launch_slot(
    state: &State<'_, AppState>,
    profile_id: Uuid,
) -> Result<Option<Instant>, String> {
    let mut attempts = state
        .profile_launch_attempts
        .lock()
        .map_err(|_| "profile launch attempts lock poisoned".to_string())?;
    Ok(attempts.insert(profile_id, Instant::now()))
}

fn should_suppress_duplicate_librewolf_launch(
    context: &LaunchContext,
    previous_attempt: Option<Instant>,
) -> bool {
    if !matches!(context.profile.engine, Engine::Librewolf) {
        return false;
    }
    if !crate::process_tracking::firefox_profile_lock_present_for_dir(&context.user_data_dir) {
        return false;
    }
    let Some(previous_attempt) = previous_attempt else {
        return false;
    };
    previous_attempt.elapsed() <= LIBREWOLF_DUPLICATE_LAUNCH_GUARD_WINDOW
}
