use super::*;
use std::{thread, time::Duration};

pub(crate) fn reuse_tracked_session_impl(
    app_handle: &tauri::AppHandle,
    state: &State<'_, AppState>,
    request: &ActionProfileRequest,
    correlation_id: &str,
    context: &LaunchContext,
) -> Result<Option<UiEnvelope<ProfileMetadata>>, String> {
    let launched = state
        .launched_processes
        .lock()
        .map_err(|_| "launch map lock poisoned".to_string())?;
    let Some(existing_pid) = launched.get(&context.profile_id).copied() else {
        return Ok(None);
    };
    drop(launched);

    let identity_restart_required = super::should_restart_for_identity_policy(
        &context.profile_root,
        &context.session_engine,
        context.identity_policy_hash.as_deref(),
    );
    let trusted = trusted_session_for_profile(
        state,
        context.profile_id,
        existing_pid,
        &context.session_engine,
        &context.profile_root,
        &context.user_data_dir,
    )?;
    eprintln!(
        "[profile-launch][reuse-tracked] profile={} pid={} trusted={} running={} launch_url_requested={} identity_restart_required={}",
        context.profile_id,
        existing_pid,
        trusted.is_some(),
        is_pid_running(existing_pid),
        context.launch_url_requested,
        identity_restart_required
    );
    if trusted.is_some() && is_pid_running(existing_pid) && !context.launch_url_requested {
        if identity_restart_required {
            append_profile_log(
                app_handle,
                context.profile_id,
                "launcher",
                format!(
                    "Restarting running session pid={existing_pid} to apply updated identity policy"
                ),
            );
            terminate_process_tree(existing_pid);
            let _ = revoke_launch_session(state, context.profile_id, Some(existing_pid));
            return Ok(None);
        }
        append_profile_log(
            app_handle,
            context.profile_id,
            "launcher",
            format!("Trusted running session reused pid={existing_pid}"),
        );
        return Ok(Some(ok(
            correlation_id.to_string(),
            context.profile.clone(),
        )));
    }
    if trusted.is_some() && is_pid_running(existing_pid) && context.launch_url_requested {
        if identity_restart_required {
            append_profile_log(
                app_handle,
                context.profile_id,
                "launcher",
                format!(
                    "Restarting running session pid={existing_pid} before opening URL so identity policy is applied"
                ),
            );
            terminate_process_tree(existing_pid);
            let _ = revoke_launch_session(state, context.profile_id, Some(existing_pid));
            return Ok(None);
        }
        append_profile_log(
            app_handle,
            context.profile_id,
            "launcher",
            format!("Forwarding URL into running session pid={existing_pid}"),
        );
        super::open_url_in_running_profile(
            state.inner(),
            &context.profile,
            &context.profile_root,
            request.launch_url.as_deref().unwrap_or_default(),
        )?;
        let _ = app_handle.emit(
            "profile-state-changed",
            serde_json::json!({
                "profileId": context.profile_id.to_string(),
                "state": "running"
            }),
        );
        return Ok(Some(super::patch_state(
            state,
            request,
            correlation_id.to_string(),
            ProfileState::Running,
        )?));
    }
    if trusted.is_none() && !context.launch_url_requested && is_pid_running(existing_pid) {
        append_profile_log(
            app_handle,
            context.profile_id,
            "launcher",
            format!("Terminating untrusted lingering process pid={existing_pid}"),
        );
        terminate_process_tree(existing_pid);
        let _ = revoke_launch_session(state, context.profile_id, Some(existing_pid));
    }

    Ok(None)
}

pub(crate) fn reuse_discovered_session_impl(
    app_handle: &tauri::AppHandle,
    state: &State<'_, AppState>,
    request: &ActionProfileRequest,
    correlation_id: &str,
    context: &LaunchContext,
) -> Result<Option<UiEnvelope<ProfileMetadata>>, String> {
    let mut discovered_pids = wait_for_discovered_profile_pids(context);
    discovered_pids.sort_unstable();
    discovered_pids.dedup();
    eprintln!(
        "[profile-launch][reuse-discovered] profile={} initial_discovered_pids={:?} launch_url_requested={}",
        context.profile_id,
        discovered_pids,
        context.launch_url_requested
    );
    if discovered_pids.is_empty() {
        return Ok(None);
    }

    let identity_restart_required = super::should_restart_for_identity_policy(
        &context.profile_root,
        &context.session_engine,
        context.identity_policy_hash.as_deref(),
    );
    let mut trusted_pid = None;
    for pid in &discovered_pids {
        let trusted = trusted_session_for_profile(
            state,
            context.profile_id,
            *pid,
            &context.session_engine,
            &context.profile_root,
            &context.user_data_dir,
        )?;
        eprintln!(
            "[profile-launch][reuse-discovered] inspect pid={} trusted={} running={}",
            pid,
            trusted.is_some(),
            is_pid_running(*pid)
        );
        if trusted.is_some() && is_pid_running(*pid) {
            trusted_pid = Some(*pid);
            break;
        }
    }
    let Some(existing_pid) = trusted_pid else {
        if !identity_restart_required {
            if let Some(existing_pid) = select_reusable_discovered_pid(&discovered_pids) {
                append_profile_log(
                    app_handle,
                    context.profile_id,
                    "launcher",
                    format!(
                        "Adopting discovered running process without trusted marker pid={existing_pid}"
                    ),
                );
                eprintln!(
                    "[profile-launch] adopting discovered running process without trusted marker pid={} profile_dir={}",
                    existing_pid,
                    context.user_data_dir.display()
                );
                let mut launched = state
                    .launched_processes
                    .lock()
                    .map_err(|_| "launch map lock poisoned".to_string())?;
                launched.insert(context.profile_id, existing_pid);
                drop(launched);
                issue_launch_session(
                    state,
                    context.profile_id,
                    existing_pid,
                    &context.session_engine,
                    &context.profile_root,
                    &context.user_data_dir,
                )?;
                if context.launch_url_requested {
                    super::open_url_in_running_profile(
                        state.inner(),
                        &context.profile,
                        &context.profile_root,
                        request.launch_url.as_deref().unwrap_or_default(),
                    )?;
                }
                let _ = app_handle.emit(
                    "profile-state-changed",
                    serde_json::json!({
                        "profileId": context.profile_id.to_string(),
                        "state": "running"
                    }),
                );
                return Ok(Some(super::patch_state(
                    state,
                    request,
                    correlation_id.to_string(),
                    ProfileState::Running,
                )?));
            }
        }
        for pid in discovered_pids {
            append_profile_log(
                app_handle,
                context.profile_id,
                "launcher",
                format!("Terminating untrusted process discovered in workspace pid={pid}"),
            );
            eprintln!(
                "[profile-launch] untrusted process detected for workspace, terminating pid={} profile_dir={}",
                pid,
                context.user_data_dir.display()
            );
            terminate_process_tree(pid);
            let _ = revoke_launch_session(state, context.profile_id, Some(pid));
        }
        return Ok(None);
    };

    if identity_restart_required {
        append_profile_log(
            app_handle,
            context.profile_id,
            "launcher",
            format!(
                "Restarting discovered trusted process pid={existing_pid} to apply updated identity policy"
            ),
        );
        eprintln!(
            "[profile-launch] restarting trusted process for identity policy pid={existing_pid}"
        );
        terminate_process_tree(existing_pid);
        let _ = revoke_launch_session(state, context.profile_id, Some(existing_pid));
        return Ok(None);
    } else if context.launch_url_requested {
        append_profile_log(
            app_handle,
            context.profile_id,
            "launcher",
            format!("Forwarding URL to discovered session pid={existing_pid}"),
        );
        eprintln!(
            "[profile-launch] forwarding url to trusted running session pid={existing_pid}"
        );
        super::open_url_in_running_profile(
            state.inner(),
            &context.profile,
            &context.profile_root,
            request.launch_url.as_deref().unwrap_or_default(),
        )?;
        let mut launched = state
            .launched_processes
            .lock()
            .map_err(|_| "launch map lock poisoned".to_string())?;
        launched.insert(context.profile_id, existing_pid);
        drop(launched);
        let _ = app_handle.emit(
            "profile-state-changed",
            serde_json::json!({
                "profileId": context.profile_id.to_string(),
                "state": "running"
            }),
        );
        return Ok(Some(super::patch_state(
            state,
            request,
            correlation_id.to_string(),
            ProfileState::Running,
        )?));
    }
    append_profile_log(
        app_handle,
        context.profile_id,
        "launcher",
        format!("Discovered trusted running process pid={existing_pid}"),
    );
    let mut launched = state
        .launched_processes
        .lock()
        .map_err(|_| "launch map lock poisoned".to_string())?;
    launched.insert(context.profile_id, existing_pid);
    drop(launched);
    let _ = app_handle.emit(
        "profile-state-changed",
        serde_json::json!({
            "profileId": context.profile_id.to_string(),
            "state": "running"
        }),
    );
    Ok(Some(super::patch_state(
        state,
        request,
        correlation_id.to_string(),
        ProfileState::Running,
    )?))
}

#[path = "profile_commands_launch_session_core_support.rs"]
mod profile_commands_launch_session_core_support;

use profile_commands_launch_session_core_support::{
    select_reusable_discovered_pid,
    wait_for_discovered_profile_pids,
};

