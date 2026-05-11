use super::*;

pub(crate) fn build_home_dashboard(
    state: State<AppState>,
    request: BuildHomeRequest,
    correlation_id: String,
) -> Result<UiEnvelope<HomeDashboardModel>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    let manager = state
        .manager
        .lock()
        .map_err(|_| "manager lock poisoned".to_string())?;
    let profile = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    drop(manager);

    let service = state
        .home_service
        .lock()
        .map_err(|_| "home service lock poisoned".to_string())?;
    let dashboard = service.build_dashboard(
        profile_id,
        request.dns_blocked,
        request.tracker_blocked,
        request.service_blocked,
        profile.state == ProfileState::Running,
    );
    Ok(ok(correlation_id, dashboard))
}

pub(crate) fn panic_wipe_profile(
    state: State<AppState>,
    request: PanicRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    let manager = state
        .manager
        .lock()
        .map_err(|_| "manager lock poisoned".to_string())?;
    let profile = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    let mut retain_paths = merge_panic_retain_paths_impl(&profile, &request.retain_paths);
    for path in extension_panic_retain_paths_impl(&state, profile.id)? {
        if !retain_paths.iter().any(|item| item == &path) {
            retain_paths.push(path);
        }
    }
    drop(manager);

    let tracked_pid = {
        let launched = state
            .launched_processes
            .lock()
            .map_err(|_| "launch map lock poisoned".to_string())?;
        launched.get(&profile_id).copied()
    };
    let user_data_dir = state
        .profile_root
        .join(profile_id.to_string())
        .join("engine-profile");
    terminate_profile_processes(&user_data_dir);
    if let Some(pid) = tracked_pid {
        terminate_process_tree(pid);
        let _ = revoke_launch_session(&state, profile_id, Some(pid));
        stop_profile_network_stack(&state.app_handle, profile_id);
        clear_profile_process(&state.app_handle, profile_id, pid, false);
    }

    let manager = state
        .manager
        .lock()
        .map_err(|_| "manager lock poisoned".to_string())?;
    let service = state
        .panic_service
        .lock()
        .map_err(|_| "panic service lock poisoned".to_string())?;
    let summary = service
        .execute(
            &manager,
            profile_id,
            request.mode,
            profile.panic_protected_sites.clone(),
            retain_paths,
            &request.confirm_phrase,
            "ui",
        )
        .map_err(|e| e.to_string())?;
    Ok(ok(
        correlation_id,
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?,
    ))
}

pub(crate) fn execute_launch_hook(
    state: State<AppState>,
    request: ExecuteHookRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let service = state
        .launch_hook_service
        .lock()
        .map_err(|_| "launch hook lock poisoned".to_string())?;
    service.validate(&request.policy)?;
    drop(service);

    let timeout = Duration::from_millis(request.policy.timeout_ms.max(1));
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("launch hook client error: {e}"))?;
    let response = client
        .get(request.policy.url.clone())
        .send()
        .map_err(|e| format!("launch hook request failed: {e}"))?;
    let status = response.status();
    let executed = status.is_success() || status.is_redirection();
    let result = serde_json::json!({
        "accepted": true,
        "executed": executed,
        "statusCode": status.as_u16(),
        "messageKey": launch_hook_message_key_impl(status, executed)
    });
    if !executed {
        return Err(format!(
            "launch hook endpoint returned unexpected status: {}",
            status
        ));
    }
    Ok(ok(
        correlation_id,
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?,
    ))
}

pub(crate) fn resolve_pip_policy(
    state: State<AppState>,
    request: ResolvePipRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let service = state
        .pip_service
        .lock()
        .map_err(|_| "pip lock poisoned".to_string())?;
    let result = service.resolve(request.requested, request.platform_supported);
    Ok(ok(
        correlation_id,
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?,
    ))
}

pub(crate) fn import_search_providers(
    state: State<AppState>,
    request: ImportSearchRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut registry = state
        .search_registry
        .lock()
        .map_err(|_| "search registry lock poisoned".to_string())?;
    registry.import_presets(request.providers)?;
    Ok(ok(correlation_id, true))
}

pub(crate) fn set_default_search_provider(
    state: State<AppState>,
    request: DefaultSearchRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let registry = state
        .search_registry
        .lock()
        .map_err(|_| "search registry lock poisoned".to_string())?;
    let provider = registry.set_default(&request.provider_id)?;
    Ok(ok(
        correlation_id,
        serde_json::to_string_pretty(provider).map_err(|e| e.to_string())?,
    ))
}

pub(crate) fn run_guardrail_check(
    state: State<AppState>,
    request: GuardrailCheckRequest,
    correlation_id: String,
) -> Result<UiEnvelope<GuardrailCheckResult>, String> {
    let profile_id = Uuid::parse_str(&request.profile_id).map_err(|e| e.to_string())?;
    let granted: Result<Vec<Uuid>, String> = request
        .granted_profile_ids
        .iter()
        .map(|v| Uuid::parse_str(v).map_err(|e| e.to_string()))
        .collect();
    let granted = granted?;
    let mut guardrails = state
        .security_guardrails
        .lock()
        .map_err(|_| "guardrails lock poisoned".to_string())?;
    let rate_ok = guardrails.enforce_rate_limit(&request.token).is_ok();
    let rbac_ok = guardrails
        .enforce_rbac(request.role, &request.operation)
        .is_ok();
    let consent_ok = guardrails
        .enforce_consent(
            request.grant.as_ref(),
            profile_id,
            &request.operation,
            now_unix_ms(),
        )
        .is_ok();
    let scope_ok = guardrails
        .enforce_no_scope_escalation(profile_id, &granted)
        .is_ok();
    Ok(ok(
        correlation_id,
        GuardrailCheckResult {
            rate_ok,
            rbac_ok,
            consent_ok,
            scope_ok,
        },
    ))
}

pub(crate) fn append_runtime_log(
    state: State<AppState>,
    entry: String,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    push_runtime_log(state.inner(), entry);
    Ok(ok(correlation_id, true))
}

pub(crate) fn read_runtime_logs(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<String>>, String> {
    Ok(ok(correlation_id, read_runtime_log_lines(&state)?))
}

pub(crate) fn merge_panic_retain_paths_impl(
    profile: &browser_profile::ProfileMetadata,
    explicit_paths: &[String],
) -> Vec<String> {
    let mut merged = explicit_paths.to_vec();
    for domain in &profile.panic_protected_sites {
        let normalized = domain.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }
        for path in [
            format!("data/cookies/{normalized}"),
            format!("data/history/{normalized}"),
        ] {
            if !merged.iter().any(|item| item == &path) {
                merged.push(path);
            }
        }
    }
    merged
}

fn extension_panic_retain_paths_impl(state: &AppState, profile_id: Uuid) -> Result<Vec<String>, String> {
    let library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let profile_key = profile_id.to_string();
    let has_preserve_extension = library.items.values().any(|item| {
        item.preserve_on_panic_wipe
            && item
                .assigned_profile_ids
                .iter()
                .any(|id| id == &profile_key)
    });
    let has_protected_extension_data = library.items.values().any(|item| {
        item.protect_data_from_panic_wipe
            && item
                .assigned_profile_ids
                .iter()
                .any(|id| id == &profile_key)
    });
    let mut retain = Vec::new();
    if has_preserve_extension || has_protected_extension_data {
        retain.push("extensions".to_string());
    }
    if has_protected_extension_data {
        retain.extend([
            "engine-profile/Default/Local Extension Settings".to_string(),
            "engine-profile/Default/Extension State".to_string(),
            "engine-profile/storage/default".to_string(),
            "engine-profile/browser-extension-data".to_string(),
        ]);
    }
    Ok(retain)
}

fn launch_hook_message_key_impl(status: StatusCode, executed: bool) -> &'static str {
    if executed {
        return "launch_hook.executed";
    }
    if status.is_client_error() || status.is_server_error() {
        return "launch_hook.http_error";
    }
    "launch_hook.unexpected_status"
}
