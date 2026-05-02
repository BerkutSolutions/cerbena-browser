use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

use browser_engine::{EngineDownloadProgress, EngineInstallation, EngineKind, EngineRuntime};
use browser_import_export::{
    export_profile_archive, import_profile_archive, EncryptedProfileArchive,
};
use browser_profile::{
    validate_modal_payload, CreateProfileInput, Engine, PatchProfileInput, ProfileMetadata,
    ProfileModalPayload, ProfileState,
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use uuid::Uuid;

use crate::{
    device_posture::enforce_launch_posture,
    envelope::ok,
    envelope::UiEnvelope,
    keepassxc_bridge::ensure_keepassxc_bridge_for_profile,
    network_sandbox_lifecycle::{ensure_profile_network_stack, stop_profile_network_stack},
    launch_sessions::{
        issue_launch_session, revoke_launch_session, trusted_session_for_profile,
        trusted_session_pid,
    },
    launcher_commands::load_global_security_record,
    panic_frame::maybe_start_panic_frame,
    process_tracking::{
        clear_profile_process, find_profile_main_window_pid_for_dir,
        find_profile_process_pid_for_dir, is_process_running as is_pid_running,
        terminate_process_tree, terminate_profile_processes, track_profile_process,
    },
    profile_security::{
        assess_profile, cookies_copy_allowed, first_launch_blocker, tags_allow_keepassxc,
        tags_allow_system_access, tags_disable_extension_launch, ERR_COOKIES_COPY_BLOCKED,
        ERR_LOCKED_REQUIRES_UNLOCK, ERR_MAXIMUM_POLICY_EXTENSIONS_FORBIDDEN,
    },
    service_domains::service_domain_seeds,
    state::{ensure_default_profiles, AppState, ExtensionLibraryItem},
};

fn emit_profile_launch_progress(
    app_handle: &tauri::AppHandle,
    profile_id: Uuid,
    stage_key: &str,
    message_key: &str,
    done: bool,
    error: Option<&str>,
) {
    let _ = app_handle.emit(
        "profile-launch-progress",
        serde_json::json!({
            "profileId": profile_id.to_string(),
            "stageKey": stage_key,
            "messageKey": message_key,
            "done": done,
            "error": error,
        }),
    );
}
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProfileRequest {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub engine: String,
    pub default_start_page: Option<String>,
    pub default_search_provider: Option<String>,
    pub ephemeral_mode: bool,
    pub password_lock_enabled: bool,
    #[serde(default)]
    pub panic_frame_enabled: bool,
    #[serde(default)]
    pub panic_frame_color: Option<String>,
    #[serde(default)]
    pub panic_protected_sites: Vec<String>,
    pub ephemeral_retain_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    pub profile_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub state: Option<String>,
    pub default_start_page: Option<String>,
    pub default_search_provider: Option<String>,
    pub ephemeral_mode: Option<bool>,
    pub password_lock_enabled: Option<bool>,
    pub panic_frame_enabled: Option<bool>,
    pub panic_frame_color: Option<String>,
    pub panic_protected_sites: Option<Vec<String>>,
    pub ephemeral_retain_paths: Option<Vec<String>>,
    pub expected_updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionProfileRequest {
    pub profile_id: String,
    pub launch_url: Option<String>,
    pub device_posture_ack_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateProfileRequest {
    pub profile_id: String,
    pub new_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockProfileRequest {
    pub profile_id: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockProfileRequest {
    pub profile_id: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfileRequest {
    pub profile_id: String,
    pub passphrase: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProfileRequest {
    pub archive_json: String,
    pub expected_profile_id: String,
    pub passphrase: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcknowledgeWayfernTosRequest {
    pub profile_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfileResponse {
    pub archive_json: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProfileResponse {
    pub profile: ProfileMetadata,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyCookiesRequest {
    pub source_profile_id: String,
    pub target_profile_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyCookiesResponse {
    pub copied_targets: usize,
    pub skipped_targets: Vec<String>,
}

#[tauri::command]
pub fn list_profiles(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<ProfileMetadata>>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    ensure_default_profiles(&manager)?;
    let list = manager.list_profiles().map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, list))
}

#[tauri::command]
pub fn create_profile(
    state: State<AppState>,
    request: CreateProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile = manager
        .create_profile(CreateProfileInput {
            name: request.name,
            description: request.description,
            tags: request.tags,
            engine: parse_engine(&request.engine)?,
            default_start_page: request
                .default_start_page
                .or_else(|| global_startup_page(&state)),
            default_search_provider: request.default_search_provider,
            ephemeral_mode: request.ephemeral_mode,
            password_lock_enabled: request.password_lock_enabled,
            panic_frame_enabled: request.panic_frame_enabled,
            panic_frame_color: request.panic_frame_color,
            panic_protected_sites: request.panic_protected_sites,
            ephemeral_retain_paths: request.ephemeral_retain_paths,
        })
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, profile))
}

#[tauri::command]
pub fn update_profile(
    state: State<AppState>,
    request: UpdateProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let patch = PatchProfileInput {
        name: request.name,
        description: request.description.map(Some),
        tags: request.tags,
        state: request.state.map(|v| parse_state(&v)).transpose()?,
        default_start_page: request.default_start_page.map(Some),
        default_search_provider: request.default_search_provider.map(Some),
        ephemeral_mode: request.ephemeral_mode,
        password_lock_enabled: request.password_lock_enabled,
        panic_frame_enabled: request.panic_frame_enabled,
        panic_frame_color: request.panic_frame_color.map(Some),
        panic_protected_sites: request.panic_protected_sites,
        ephemeral_retain_paths: request.ephemeral_retain_paths,
    };
    let profile = manager
        .update_profile_with_actor(
            profile_id,
            patch,
            request.expected_updated_at.as_deref(),
            "ui",
        )
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, profile))
}

#[tauri::command]
pub fn delete_profile(
    state: State<AppState>,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    manager
        .delete_profile_with_actor(profile_id, "ui")
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn duplicate_profile(
    state: State<AppState>,
    request: DuplicateProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let source_id = Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let source = manager.get_profile(source_id).map_err(|e| e.to_string())?;
    let created = manager
        .create_profile(CreateProfileInput {
            name: request.new_name,
            description: source.description,
            tags: source.tags,
            engine: source.engine,
            default_start_page: source.default_start_page,
            default_search_provider: source.default_search_provider,
            ephemeral_mode: source.ephemeral_mode,
            password_lock_enabled: false,
            panic_frame_enabled: source.panic_frame_enabled,
            panic_frame_color: source.panic_frame_color,
            panic_protected_sites: source.panic_protected_sites,
            ephemeral_retain_paths: source.ephemeral_retain_paths,
        })
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, created))
}

#[tauri::command]
pub async fn launch_profile(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
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
    let profile_key = profile.id.to_string();
    let _ = crate::extensions_commands::refresh_extension_library_updates_impl(
        state.inner(),
        Some(profile_key.as_str()),
    );
    if let Some(code) = first_launch_blocker(&profile) {
        return Err(code.to_string());
    }
    let assessment = assess_profile(&profile);
    let active_extensions = collect_active_profile_extensions(
        &state,
        profile.id,
        &profile.tags,
        profile.engine.clone(),
    );
    if assessment.policy_level == "maximum" && !active_extensions.is_empty() {
        return Err(ERR_MAXIMUM_POLICY_EXTENSIONS_FORBIDDEN.to_string());
    }
    enforce_launch_posture(&state, &profile, request.device_posture_ack_id.as_deref())?;

    let profile_root = state.profile_root.join(profile.id.to_string());
    let user_data_dir = profile_root.join("engine-profile");
    fs::create_dir_all(&user_data_dir).map_err(|e| e.to_string())?;
    emit_profile_launch_progress(
        &app_handle,
        profile.id,
        "preflight",
        "profile.launchProgress.preflight",
        false,
        None,
    );
    let session_engine = engine_session_key(&profile.engine);
    {
        let launched = state
            .launched_processes
            .lock()
            .map_err(|_| "launch map lock poisoned".to_string())?;
        if let Some(existing_pid) = launched.get(&profile_id).copied() {
            let trusted = trusted_session_for_profile(
                &state,
                profile_id,
                existing_pid,
                session_engine,
                &profile_root,
                &user_data_dir,
            )?;
            if trusted.is_some() && is_pid_running(existing_pid) && !launch_url_requested {
                return Ok(ok(correlation_id, profile.clone()));
            }
            if trusted.is_none() && !launch_url_requested && is_pid_running(existing_pid) {
                terminate_process_tree(existing_pid);
                let _ = revoke_launch_session(&state, profile_id, Some(existing_pid));
            }
        }
    }
    eprintln!(
        "[profile-launch] start profile={} engine={:?} profile_root={} user_data_dir={}",
        profile.id,
        profile.engine,
        profile_root.display(),
        user_data_dir.display()
    );
    if let Some(existing_pid) = find_profile_process_pid_for_dir(&user_data_dir) {
        let trusted = trusted_session_for_profile(
            &state,
            profile_id,
            existing_pid,
            session_engine,
            &profile_root,
            &user_data_dir,
        )?;
        if trusted.is_some() {
            if launch_url_requested {
                eprintln!(
                    "[profile-launch] trusted running session retained for url-targeted launch pid={existing_pid}"
                );
            } else {
                let mut launched = state
                    .launched_processes
                    .lock()
                    .map_err(|_| "launch map lock poisoned".to_string())?;
                launched.insert(profile_id, existing_pid);
                drop(launched);
                let _ = app_handle.emit(
                    "profile-state-changed",
                    serde_json::json!({
                        "profileId": profile_id.to_string(),
                        "state": "running"
                    }),
                );
                return patch_state(&state, &request, correlation_id, ProfileState::Running);
            }
        } else {
            eprintln!(
                "[profile-launch] untrusted process detected for workspace, terminating pid={} profile_dir={}",
                existing_pid,
                user_data_dir.display()
            );
            terminate_process_tree(existing_pid);
            let _ = revoke_launch_session(&state, profile_id, Some(existing_pid));
        }
    }

    let runtime =
        EngineRuntime::new(state.engine_runtime_root.clone()).map_err(|e| e.to_string())?;
    let engine = engine_kind(profile.engine.clone());
    emit_profile_launch_progress(
        &app_handle,
        profile.id,
        "policy",
        "profile.launchProgress.policy",
        false,
        None,
    );
    write_profile_blocked_domains(&state, &profile.id, &profile_root).map_err(|e| e.to_string())?;
    if matches!(engine, EngineKind::Wayfern) {
        emit_profile_launch_progress(
            &app_handle,
            profile.id,
            "extensions",
            "profile.launchProgress.extensions",
            false,
            None,
        );
        prepare_profile_wayfern_extensions(&state, &profile, &profile_root)?;
        emit_profile_launch_progress(
            &app_handle,
            profile.id,
            "keepassxc",
            "profile.launchProgress.keepassxc",
            false,
            None,
        );
        ensure_keepassxc_bridge_for_profile(state.inner(), &profile, &profile_root)?;
    }
    emit_profile_launch_progress(
        &app_handle,
        profile.id,
        "network",
        "profile.launchProgress.network",
        false,
        None,
    );
    let network_handle = app_handle.clone();
    let network_profile_id = profile.id;
    let gateway = tauri::async_runtime::spawn_blocking(move || {
        ensure_profile_network_stack(&network_handle, network_profile_id)
    })
    .await
    .map_err(|e| e.to_string())??;
    let runtime_hardening = assess_profile(&profile).runtime_hardening;
    if matches!(engine, EngineKind::Camoufox) {
        emit_profile_launch_progress(
            &app_handle,
            profile.id,
            "profile-runtime",
            "profile.launchProgress.profileRuntime",
            false,
            None,
        );
        prepare_camoufox_profile_runtime(
            &user_data_dir,
            profile.default_start_page.as_deref(),
            profile.default_search_provider.as_deref(),
            Some(gateway.port),
            runtime_hardening,
        )
        .map_err(|e| e.to_string())?;
        let user_js = user_data_dir.join("user.js");
        eprintln!(
            "[profile-launch] camoufox preflight user.js_exists={} user.js_path={}",
            user_js.exists(),
            user_js.display()
        );
        if let Some(existing_pid) = find_profile_process_pid_for_dir(&user_data_dir) {
            if launch_url_requested {
                eprintln!(
                    "[profile-launch] camoufox existing process kept for launch_url pid={} profile_dir={}",
                    existing_pid,
                    user_data_dir.display()
                );
            } else {
                eprintln!(
                "[profile-launch] camoufox existing profile process detected pid={} profile_dir={}",
                existing_pid,
                user_data_dir.display()
            );
                terminate_process_tree(existing_pid);
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
    emit_profile_launch_progress(
        &app_handle,
        profile.id,
        "engine",
        "profile.launchProgress.engine",
        false,
        None,
    );
    let installation = ensure_engine_ready(&app_handle, &state, &runtime, engine).await?;
    if matches!(engine, EngineKind::Camoufox) {
        neutralize_camoufox_builtin_theme(&installation.binary_path).map_err(|e| e.to_string())?;
        apply_camoufox_website_filter(&state, &profile.id, &installation.binary_path)
            .map_err(|e| e.to_string())?;
    }
    let start_page = profile
        .default_start_page
        .clone()
        .unwrap_or_else(|| "https://duckduckgo.com".to_string());
    let start_page = request
        .launch_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or(start_page);
    let private_mode = profile.ephemeral_mode
        && profile
            .tags
            .iter()
            .any(|tag| tag.eq_ignore_ascii_case("private"));

    let launch_runtime = runtime.clone();
    let launch_root = profile_root.clone();
    let gateway_port = Some(gateway.port);
    emit_profile_launch_progress(
        &app_handle,
        profile.id,
        "browser",
        "profile.launchProgress.browser",
        false,
        None,
    );
    let pid = tauri::async_runtime::spawn_blocking(move || {
        launch_runtime.launch(
            engine,
            launch_root,
            profile_id,
            start_page,
            private_mode,
            gateway_port,
            runtime_hardening,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let tracked_pid = if matches!(engine, EngineKind::Camoufox | EngineKind::Wayfern) {
        find_profile_main_window_pid_for_dir(&user_data_dir)
            .or_else(|| find_profile_process_pid_for_dir(&user_data_dir))
            .unwrap_or(pid)
    } else {
        pid
    };
    let mut launched = state
        .launched_processes
        .lock()
        .map_err(|_| "launch map lock poisoned".to_string())?;
    launched.insert(profile_id, tracked_pid);
    drop(launched);

    track_profile_process(
        app_handle.clone(),
        profile_id,
        tracked_pid,
        user_data_dir.clone(),
    );
    maybe_start_panic_frame(&app_handle, profile_id, tracked_pid);
    issue_launch_session(
        &state,
        profile_id,
        tracked_pid,
        session_engine,
        &profile_root,
        &user_data_dir,
    )?;

    let _ = installation;
    let _ = app_handle.emit(
        "profile-state-changed",
        serde_json::json!({
            "profileId": profile_id.to_string(),
            "state": "running"
        }),
    );
    emit_profile_launch_progress(
        &app_handle,
        profile.id,
        "done",
        "profile.launchProgress.done",
        true,
        None,
    );

    patch_state(&state, &request, correlation_id, ProfileState::Running)
}

fn prepare_camoufox_profile_runtime(
    profile_dir: &std::path::Path,
    default_start_page: Option<&str>,
    default_search_provider: Option<&str>,
    gateway_proxy_port: Option<u16>,
    runtime_hardening: bool,
) -> Result<(), std::io::Error> {
    fs::create_dir_all(profile_dir)?;
    let startup_page = normalize_start_page_url(default_start_page)
        .replace('\\', "\\\\")
        .replace('"', "\\\"");

    let stale_files = [
        "xulstore.json",
        "prefs.js",
        "sessionstore.jsonlz4",
        "sessionCheckpoints.json",
        "times.json",
        "search.json.mozlz4",
        "search.sqlite",
        "search.sqlite-wal",
        "search.sqlite-shm",
    ];
    for file_name in stale_files {
        let path = profile_dir.join(file_name);
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }

    let startup_cache = profile_dir.join("startupCache");
    if startup_cache.exists() {
        let _ = fs::remove_dir_all(startup_cache);
    }

    let mut user_js_lines = vec![
        "user_pref(\"browser.startup.page\", 1);".to_string(),
        format!("user_pref(\"browser.startup.homepage\", \"{startup_page}\");"),
        "user_pref(\"browser.newtabpage.enabled\", false);".to_string(),
        format!("user_pref(\"browser.newtab.url\", \"{startup_page}\");"),
        "user_pref(\"browser.tabs.hideSingleTab\", false);".to_string(),
        "user_pref(\"browser.tabs.inTitlebar\", 1);".to_string(),
        "user_pref(\"browser.tabs.drawInTitlebar\", true);".to_string(),
        "user_pref(\"browser.tabs.closeWindowWithLastTab\", false);".to_string(),
        "user_pref(\"browser.startup.homepage_override.mstone\", \"ignore\");".to_string(),
        "user_pref(\"startup.homepage_welcome_url\", \"\");".to_string(),
        "user_pref(\"startup.homepage_welcome_url.additional\", \"\");".to_string(),
        "user_pref(\"startup.homepage_override_url\", \"\");".to_string(),
        "user_pref(\"browser.sessionstore.resume_from_crash\", false);".to_string(),
        "user_pref(\"browser.sessionstore.max_tabs_undo\", 0);".to_string(),
        "user_pref(\"browser.sessionstore.max_windows_undo\", 0);".to_string(),
        "user_pref(\"browser.search.suggest.enabled\", false);".to_string(),
        "user_pref(\"browser.search.geoSpecificDefaults\", false);".to_string(),
        "user_pref(\"browser.search.geoSpecificDefaults.url\", \"\");".to_string(),
        "user_pref(\"browser.search.region\", \"US\");".to_string(),
        "user_pref(\"browser.urlbar.suggest.searches\", false);".to_string(),
        "user_pref(\"browser.shell.checkDefaultBrowser\", false);".to_string(),
        "user_pref(\"dom.w3c_touch_events.enabled\", 0);".to_string(),
        "user_pref(\"dom.w3c_touch_events.legacy_apis.enabled\", false);".to_string(),
        "user_pref(\"dom.w3c_pointer_events.enabled\", true);".to_string(),
        "user_pref(\"ui.primaryPointerCapabilities\", 1);".to_string(),
        "user_pref(\"ui.allPointerCapabilities\", 1);".to_string(),
        "user_pref(\"browser.ui.touch_activation.enabled\", false);".to_string(),
        "user_pref(\"userChrome.decoration.cursor\", false);".to_string(),
        "user_pref(\"browser.search.newSearchConfigEnabled\", false);".to_string(),
    ];
    if let Some(engine_name) = map_search_provider_to_firefox_engine(default_search_provider) {
        user_js_lines
            .push("user_pref(\"browser.search.separatePrivateDefault\", false);".to_string());
        user_js_lines.push(
            "user_pref(\"browser.search.separatePrivateDefault.ui.enabled\", false);".to_string(),
        );
        user_js_lines.push("user_pref(\"browser.search.update\", false);".to_string());
        user_js_lines.push(format!(
            "user_pref(\"browser.search.defaultenginename\", \"{}\");",
            engine_name
        ));
        user_js_lines.push(format!(
            "user_pref(\"browser.search.defaultenginename.private\", \"{}\");",
            engine_name
        ));
        user_js_lines.push(format!(
            "user_pref(\"browser.search.defaultEngine\", \"{}\");",
            engine_name
        ));
        user_js_lines.push(format!(
            "user_pref(\"browser.search.defaultEngineName\", \"{}\");",
            engine_name
        ));
        user_js_lines.push(format!(
            "user_pref(\"browser.search.selectedEngine\", \"{}\");",
            engine_name
        ));
        user_js_lines.push(format!(
            "user_pref(\"browser.search.order.1\", \"{}\");",
            engine_name
        ));
    }
    if runtime_hardening {
        user_js_lines.push("user_pref(\"signon.rememberSignons\", false);".to_string());
        user_js_lines.push("user_pref(\"signon.autofillForms\", false);".to_string());
        user_js_lines.push("user_pref(\"browser.formfill.enable\", false);".to_string());
        user_js_lines
            .push("user_pref(\"extensions.formautofill.addresses.enabled\", false);".to_string());
        user_js_lines
            .push("user_pref(\"extensions.formautofill.creditCards.enabled\", false);".to_string());
        user_js_lines.push("user_pref(\"browser.sessionstore.privacy_level\", 2);".to_string());
    }
    if let Some(port) = gateway_proxy_port {
        user_js_lines.push("user_pref(\"network.proxy.type\", 1);".to_string());
        user_js_lines.push("user_pref(\"network.proxy.share_proxy_settings\", true);".to_string());
        user_js_lines.push("user_pref(\"network.proxy.http\", \"127.0.0.1\");".to_string());
        user_js_lines.push(format!("user_pref(\"network.proxy.http_port\", {port});"));
        user_js_lines.push("user_pref(\"network.proxy.ssl\", \"127.0.0.1\");".to_string());
        user_js_lines.push(format!("user_pref(\"network.proxy.ssl_port\", {port});"));
        user_js_lines.push("user_pref(\"network.proxy.no_proxies_on\", \"\");".to_string());
    }
    let user_js = user_js_lines.join("\n");
    fs::write(profile_dir.join("user.js"), format!("{user_js}\n"))?;

    let chrome_dir = profile_dir.join("chrome");
    fs::create_dir_all(&chrome_dir)?;
    let user_chrome = r#"
/* Force full Firefox-like chrome instead of compact/new-tab shell styles */
#TabsToolbar {
  visibility: visible !important;
  display: -moz-box !important;
  min-height: 34px !important;
}
#tabbrowser-tabs,
#tabbrowser-arrowscrollbox,
#titlebar {
  visibility: visible !important;
  display: -moz-box !important;
}
#nav-bar {
  visibility: visible !important;
}
"#;
    fs::write(chrome_dir.join("userChrome.css"), user_chrome.trim_start())?;

    eprintln!(
        "[profile-launch] camoufox profile prepared dir={} cleaned=true",
        profile_dir.display()
    );
    Ok(())
}

fn normalize_start_page_url(default_start_page: Option<&str>) -> String {
    let raw = default_start_page
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("https://duckduckgo.com");
    if raw.contains("://")
        || raw.starts_with("about:")
        || raw.starts_with("chrome:")
        || raw.starts_with("file:")
        || raw.starts_with("data:")
    {
        return raw.to_string();
    }
    format!("https://{raw}")
}

fn map_search_provider_to_firefox_engine(provider: Option<&str>) -> Option<&'static str> {
    match provider.unwrap_or("duckduckgo").to_lowercase().as_str() {
        "duckduckgo" => Some("DuckDuckGo"),
        "google" => Some("Google"),
        "bing" => Some("Bing"),
        "yandex" => Some("Yandex"),
        "brave" => Some("Brave"),
        "ecosia" => Some("Ecosia"),
        "startpage" => Some("Startpage"),
        _ => Some("DuckDuckGo"),
    }
}

fn firefox_search_engine_policy_entries() -> Vec<serde_json::Value> {
    vec![
        firefox_search_engine_entry(
            "DuckDuckGo",
            "https://duckduckgo.com/?q={searchTerms}",
            Some("https://duckduckgo.com/ac/?q={searchTerms}&type=list"),
        ),
        firefox_search_engine_entry(
            "Google",
            "https://www.google.com/search?q={searchTerms}",
            Some("https://suggestqueries.google.com/complete/search?output=firefox&q={searchTerms}"),
        ),
        firefox_search_engine_entry(
            "Bing",
            "https://www.bing.com/search?q={searchTerms}",
            Some("https://www.bing.com/osjson.aspx?query={searchTerms}"),
        ),
        firefox_search_engine_entry(
            "Yandex",
            "https://yandex.com/search/?text={searchTerms}",
            Some("https://suggest.yandex.com/suggest-ff.cgi?part={searchTerms}"),
        ),
        firefox_search_engine_entry(
            "Brave",
            "https://search.brave.com/search?q={searchTerms}",
            Some("https://search.brave.com/api/suggest?q={searchTerms}"),
        ),
        firefox_search_engine_entry(
            "Ecosia",
            "https://www.ecosia.org/search?q={searchTerms}",
            Some("https://ac.ecosia.org/autocomplete?q={searchTerms}"),
        ),
        firefox_search_engine_entry(
            "Startpage",
            "https://www.startpage.com/sp/search?query={searchTerms}",
            Some("https://www.startpage.com/suggestions?q={searchTerms}"),
        ),
    ]
}

fn firefox_search_engine_entry(
    name: &str,
    url_template: &str,
    suggest_url_template: Option<&str>,
) -> serde_json::Value {
    let mut entry = serde_json::json!({
        "Name": name,
        "URLTemplate": url_template,
    });
    if let Some(suggest_url_template) = suggest_url_template {
        entry["SuggestURLTemplate"] = serde_json::Value::String(suggest_url_template.to_string());
    }
    entry
}

fn firefox_search_engine_catalog() -> Vec<(&'static str, &'static str, &'static str, Option<&'static str>)> {
    vec![
        (
            "duckduckgo",
            "DuckDuckGo",
            "https://duckduckgo.com/?q={searchTerms}",
            Some("https://duckduckgo.com/ac/?q={searchTerms}&type=list"),
        ),
        (
            "google",
            "Google",
            "https://www.google.com/search?q={searchTerms}",
            Some("https://suggestqueries.google.com/complete/search?output=firefox&q={searchTerms}"),
        ),
        (
            "bing",
            "Bing",
            "https://www.bing.com/search?q={searchTerms}",
            Some("https://www.bing.com/osjson.aspx?query={searchTerms}"),
        ),
        (
            "yandex",
            "Yandex",
            "https://yandex.com/search/?text={searchTerms}",
            Some("https://suggest.yandex.com/suggest-ff.cgi?part={searchTerms}"),
        ),
        (
            "brave",
            "Brave",
            "https://search.brave.com/search?q={searchTerms}",
            Some("https://search.brave.com/api/suggest?q={searchTerms}"),
        ),
        (
            "ecosia",
            "Ecosia",
            "https://www.ecosia.org/search?q={searchTerms}",
            Some("https://ac.ecosia.org/autocomplete?q={searchTerms}"),
        ),
        (
            "startpage",
            "Startpage",
            "https://www.startpage.com/sp/search?query={searchTerms}",
            Some("https://www.startpage.com/suggestions?q={searchTerms}"),
        ),
    ]
}

fn write_firefox_search_plugin_bundle(distribution_dir: &Path) -> Result<(), std::io::Error> {
    let searchplugins_dir = distribution_dir.join("searchplugins").join("common");
    fs::create_dir_all(&searchplugins_dir)?;
    for (id, name, url_template, suggest_template) in firefox_search_engine_catalog() {
        let xml = build_firefox_search_plugin_xml(name, url_template, suggest_template);
        fs::write(searchplugins_dir.join(format!("{id}.xml")), xml)?;
    }
    Ok(())
}

fn build_firefox_search_plugin_xml(
    name: &str,
    url_template: &str,
    suggest_template: Option<&str>,
) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/">
  <ShortName>{name}</ShortName>
  <Description>{name}</Description>
  <InputEncoding>UTF-8</InputEncoding>
  <Url type="text/html" method="GET" template="{url_template}"/>
"#
    );
    if let Some(suggest_template) = suggest_template {
        xml.push_str(&format!(
            "  <Url type=\"application/x-suggestions+json\" method=\"GET\" template=\"{suggest_template}\"/>\n"
        ));
    }
    xml.push_str("</OpenSearchDescription>\n");
    xml
}

fn prepare_profile_wayfern_extensions(
    state: &State<'_, AppState>,
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Result<(), String> {
    let extensions_root = profile_root.join("policy").join("wayfern-extensions");
    if extensions_root.exists() {
        fs::remove_dir_all(&extensions_root)
            .map_err(|e| format!("clear wayfern extensions dir: {e}"))?;
    }
    fs::create_dir_all(&extensions_root)
        .map_err(|e| format!("create wayfern extensions dir: {e}"))?;

    for item in collect_active_profile_extensions(state, profile.id, &profile.tags, Engine::Wayfern)
    {
        let Some(package_path) = item
            .package_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let destination = extensions_root.join(sanitize_extension_dir_name(&item.id));
        fs::create_dir_all(&destination)
            .map_err(|e| format!("create wayfern extension dir: {e}"))?;
        unpack_extension_archive(Path::new(package_path), &destination)?;
    }
    Ok(())
}

fn collect_active_profile_extensions(
    state: &State<'_, AppState>,
    profile_id: Uuid,
    tags: &[String],
    engine: Engine,
) -> Vec<ExtensionLibraryItem> {
    let profile_key = profile_id.to_string();
    let allow_system_access = tags_allow_system_access(tags);
    let allow_keepassxc = tags_allow_keepassxc(tags);
    let disable_extensions_launch = tags_disable_extension_launch(tags);
    let enabled = tags
        .iter()
        .filter_map(|tag| tag.strip_prefix("ext:").map(str::to_string))
        .collect::<std::collections::BTreeSet<_>>();
    let disabled = tags
        .iter()
        .filter_map(|tag| tag.strip_prefix("ext-disabled:").map(str::to_string))
        .collect::<std::collections::BTreeSet<_>>();
    let library = match state.extension_library.lock() {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    library
        .items
        .values()
        .filter(|item| extension_scope_matches_engine(&item.engine_scope, engine.clone()))
        .filter(|item| {
            enabled.contains(&item.id)
                || item
                    .assigned_profile_ids
                    .iter()
                    .any(|value| value == &profile_key)
        })
        .filter(|item| !disabled.contains(&item.id))
        .filter(|item| {
            extension_allowed_for_launch(
                item,
                allow_system_access,
                allow_keepassxc,
                disable_extensions_launch,
            )
        })
        .cloned()
        .collect()
}

fn extension_allowed_for_launch(
    item: &ExtensionLibraryItem,
    allow_system_access: bool,
    allow_keepassxc: bool,
    disable_extensions_launch: bool,
) -> bool {
    if is_keepassxc_extension(item) {
        return allow_keepassxc;
    }
    if disable_extensions_launch {
        return false;
    }
    if is_system_access_extension(item) {
        return allow_system_access;
    }
    true
}

fn extension_has_tag(item: &ExtensionLibraryItem, expected: &str) -> bool {
    item.tags
        .iter()
        .any(|tag| tag.trim().eq_ignore_ascii_case(expected))
}

fn extension_contains_marker(item: &ExtensionLibraryItem, marker: &str) -> bool {
    [
        Some(item.display_name.as_str()),
        Some(item.source_value.as_str()),
        item.store_url.as_deref(),
        item.package_file_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.to_ascii_lowercase().contains(marker))
}

fn is_keepassxc_extension(item: &ExtensionLibraryItem) -> bool {
    extension_has_tag(item, "keepassxc")
        || extension_contains_marker(item, "keepassxc")
        || item.id.eq_ignore_ascii_case("oboonakemofpalcgghocfoadofidjkkk")
}

fn is_system_access_extension(item: &ExtensionLibraryItem) -> bool {
    is_keepassxc_extension(item)
        || extension_has_tag(item, "system-access")
        || extension_has_tag(item, "system access")
        || extension_has_tag(item, "native-messaging")
        || extension_has_tag(item, "native messaging")
        || extension_contains_marker(item, "native messaging")
}

fn extension_scope_matches_engine(engine_scope: &str, engine: Engine) -> bool {
    match engine_scope.trim().to_ascii_lowercase().as_str() {
        "firefox" => matches!(engine, Engine::Camoufox),
        "chromium" => matches!(engine, Engine::Wayfern),
        _ => true,
    }
}

fn sanitize_extension_dir_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('-').trim();
    if trimmed.is_empty() {
        "extension".to_string()
    } else {
        trimmed.to_string()
    }
}

fn unpack_extension_archive(package_path: &Path, destination: &Path) -> Result<(), String> {
    let bytes = fs::read(package_path).map_err(|e| format!("read extension package: {e}"))?;
    let archive_bytes = if package_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("crx"))
        .unwrap_or(false)
    {
        extract_crx_zip_bytes(&bytes)?
    } else {
        bytes
    };
    let cursor = Cursor::new(archive_bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("open extension archive: {e}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("read extension archive entry: {e}"))?;
        let Some(relative_path) = entry.enclosed_name().map(|value| value.to_path_buf()) else {
            continue;
        };
        let output_path = destination.join(relative_path);
        if entry.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|e| format!("create extension directory: {e}"))?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create extension parent directory: {e}"))?;
        }
        let mut buffer = Vec::new();
        entry
            .read_to_end(&mut buffer)
            .map_err(|e| format!("read extension file: {e}"))?;
        fs::write(&output_path, buffer).map_err(|e| format!("write extension file: {e}"))?;
    }
    Ok(())
}

fn extract_crx_zip_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let signature = b"PK\x03\x04";
    let Some(offset) = bytes
        .windows(signature.len())
        .position(|window| window == signature)
    else {
        return Err("embedded zip payload not found in CRX package".to_string());
    };
    Ok(bytes[offset..].to_vec())
}

fn apply_camoufox_website_filter(
    state: &State<'_, AppState>,
    profile_id: &Uuid,
    binary_path: &std::path::Path,
) -> Result<(), std::io::Error> {
    let Some(engine_dir) = binary_path.parent() else {
        return Ok(());
    };
    let distribution_dir = engine_dir.join("distribution");
    fs::create_dir_all(&distribution_dir)?;
    write_firefox_search_plugin_bundle(&distribution_dir)?;

    let mut blocked_domains: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    if let Ok(store) = state.network_store.lock() {
        if let Some(dns) = store.dns.get(&profile_id.to_string()) {
            for domain in &dns.domain_denylist {
                let trimmed = domain.trim().to_lowercase();
                if !trimmed.is_empty() {
                    blocked_domains.insert(trimmed);
                }
            }
            for (_, service) in &dns.selected_services {
                for domain in service_domain_seeds(service) {
                    blocked_domains.insert(domain.to_string());
                }
            }
            for list in &dns.selected_blocklists {
                for domain in &list.domains {
                    let trimmed = domain
                        .trim()
                        .trim_start_matches("*.")
                        .trim_start_matches('.')
                        .to_lowercase();
                    if !trimmed.is_empty() {
                        blocked_domains.insert(trimmed);
                    }
                }
            }
        }
    }
    for domain in global_active_blocklist_domains(state) {
        blocked_domains.insert(domain);
    }
    for suffix in global_domain_suffixes(state) {
        blocked_domains.insert(suffix);
    }

    let block_entries: Vec<String> = blocked_domains
        .into_iter()
        .flat_map(|domain| vec![format!("*://{domain}/*"), format!("*://*.{domain}/*")])
        .collect();
    let mut cert_paths: Vec<String> = Vec::new();
    let mut extension_install_urls: Vec<String> = Vec::new();
    if let Ok(manager) = state.manager.lock() {
        if let Ok(profile) = manager.get_profile(*profile_id) {
            cert_paths.extend(resolve_profile_certificate_paths(
                state,
                *profile_id,
                &profile.tags,
            ));
            extension_install_urls.extend(resolve_profile_extension_install_urls(
                state,
                *profile_id,
                &profile.tags,
            ));
        }
    }
    cert_paths.sort();
    cert_paths.dedup();
    extension_install_urls.sort();
    extension_install_urls.dedup();
    let policy = serde_json::json!({
        "policies": {
            "WebsiteFilter": {
                "Block": block_entries,
                "Exceptions": []
            },
            "Extensions": {
                "Install": extension_install_urls
            },
            "Certificates": {
                "Install": cert_paths
            },
            "SearchEngines": {
                "Add": firefox_search_engine_policy_entries()
            }
        }
    });
    fs::write(
        distribution_dir.join("policies.json"),
        serde_json::to_vec_pretty(&policy).unwrap_or_default(),
    )?;
    Ok(())
}

fn write_profile_blocked_domains(
    state: &State<'_, AppState>,
    profile_id: &Uuid,
    profile_root: &std::path::Path,
) -> Result<(), std::io::Error> {
    let mut blocked_domains: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    if let Ok(store) = state.network_store.lock() {
        if let Some(dns) = store.dns.get(&profile_id.to_string()) {
            for domain in &dns.domain_denylist {
                let trimmed = domain.trim().to_lowercase();
                if !trimmed.is_empty() {
                    blocked_domains.insert(trimmed);
                }
            }
            for (_, service) in &dns.selected_services {
                for domain in service_domain_seeds(service) {
                    blocked_domains.insert(domain.to_string());
                }
            }
            for list in &dns.selected_blocklists {
                for domain in &list.domains {
                    let trimmed = domain
                        .trim()
                        .trim_start_matches("*.")
                        .trim_start_matches('.')
                        .to_lowercase();
                    if !trimmed.is_empty() {
                        blocked_domains.insert(trimmed);
                    }
                }
            }
        }
    }
    for domain in global_active_blocklist_domains(state) {
        blocked_domains.insert(domain);
    }
    for suffix in global_domain_suffixes(state) {
        blocked_domains.insert(suffix);
    }

    let policy_dir = profile_root.join("policy");
    fs::create_dir_all(&policy_dir)?;
    fs::write(
        policy_dir.join("blocked-domains.json"),
        serde_json::to_vec(&blocked_domains.into_iter().collect::<Vec<_>>()).unwrap_or_default(),
    )?;
    Ok(())
}

fn global_domain_suffixes(state: &State<'_, AppState>) -> Vec<String> {
    load_global_security_record(state)
        .map(|record| {
            record
                .blocked_domain_suffixes
                .into_iter()
                .map(|value| {
                    value
                        .trim()
                        .trim_start_matches("*.")
                        .trim_start_matches('.')
                        .to_string()
                })
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn global_active_blocklist_domains(state: &State<'_, AppState>) -> Vec<String> {
    load_global_security_record(state)
        .map(|record| {
            let mut domains = std::collections::BTreeSet::new();
            for item in record.blocklists {
                if !item.active {
                    continue;
                }
                for domain in item.domains {
                    let trimmed = domain
                        .trim()
                        .trim_start_matches("*.")
                        .trim_start_matches('.')
                        .to_lowercase();
                    if !trimmed.is_empty() {
                        domains.insert(trimmed);
                    }
                }
            }
            domains.into_iter().collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn resolve_profile_certificate_paths(
    state: &State<'_, AppState>,
    profile_id: Uuid,
    tags: &[String],
) -> Vec<String> {
    let selected_ids = tags
        .iter()
        .filter_map(|tag| tag.strip_prefix("cert-id:").map(|v| v.to_string()))
        .collect::<std::collections::BTreeSet<_>>();
    let mut paths = std::collections::BTreeSet::new();
    if let Ok(record) = load_global_security_record(state) {
        for item in record.certificates {
            let assigned = item
                .profile_ids
                .iter()
                .any(|value| value == &profile_id.to_string());
            if !item.path.trim().is_empty()
                && (item.apply_globally || assigned || selected_ids.contains(&item.id))
            {
                paths.insert(item.path.trim().to_string());
            }
        }
    }
    for tag in tags {
        if let Some(path) = tag.strip_prefix("cert:") {
            if path != "global" && !path.trim().is_empty() {
                paths.insert(path.trim().to_string());
            }
        }
    }
    paths.into_iter().collect()
}

fn resolve_profile_extension_install_urls(
    state: &State<'_, AppState>,
    profile_id: Uuid,
    tags: &[String],
) -> Vec<String> {
    collect_active_profile_extensions(state, profile_id, tags, Engine::Camoufox)
        .into_iter()
        .filter_map(|item| {
            item.package_path
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(path_to_file_url)
        })
        .collect()
}

fn path_to_file_url(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with("//") {
        format!("file:{normalized}")
    } else {
        format!("file:///{normalized}")
    }
}

fn global_startup_page(state: &State<'_, AppState>) -> Option<String> {
    load_global_security_record(state)
        .ok()
        .and_then(|record| record.startup_page)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn neutralize_camoufox_builtin_theme(binary_path: &std::path::Path) -> Result<(), std::io::Error> {
    let Some(engine_dir) = binary_path.parent() else {
        return Ok(());
    };
    let chrome_css = engine_dir.join("chrome.css");
    if !chrome_css.exists() {
        return Ok(());
    }

    let backup = engine_dir.join("chrome.css.launcher-backup");
    if !backup.exists() {
        fs::copy(&chrome_css, &backup)?;
    }

    let current = fs::read_to_string(&chrome_css).unwrap_or_default();
    if current.contains("launcher-neutralized") {
        return Ok(());
    }

    fs::write(
        &chrome_css,
        "/* launcher-neutralized: restore default Firefox chrome UI */\n",
    )?;
    eprintln!(
        "[profile-launch] camoufox builtin chrome.css neutralized path={}",
        chrome_css.display()
    );
    Ok(())
}

#[tauri::command]
pub fn stop_profile(
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
    terminate_profile_processes(&user_data_dir);
    if let Some(pid) = pid {
        terminate_process_tree(pid);
    }
    revoke_launch_session(&state, profile_id, tracked_pid)?;
    stop_profile_network_stack(&state.app_handle, profile_id);
    clear_profile_process(
        &state.app_handle,
        profile_id,
        tracked_pid.unwrap_or(pid.unwrap_or_default()),
        false,
    );

    let result = patch_state(&state, &request, correlation_id, ProfileState::Stopped)?;
    let _ = state.app_handle.emit(
        "profile-state-changed",
        serde_json::json!({
            "profileId": profile_id.to_string(),
            "state": "stopped"
        }),
    );
    Ok(result)
}

#[tauri::command]
pub fn acknowledge_wayfern_tos(
    state: State<AppState>,
    request: AcknowledgeWayfernTosRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let runtime =
        EngineRuntime::new(state.engine_runtime_root.clone()).map_err(|e| e.to_string())?;
    runtime
        .acknowledge_wayfern_tos(&state.profile_root.join(profile_id.to_string()), profile_id)
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub async fn ensure_engine_binaries(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<String>>, String> {
    let runtime =
        EngineRuntime::new(state.engine_runtime_root.clone()).map_err(|e| e.to_string())?;
    let mut ready = Vec::new();
    for engine in [EngineKind::Wayfern, EngineKind::Camoufox] {
        let installation = ensure_engine_ready(&app_handle, &state, &runtime, engine).await?;
        ready.push(format!(
            "{} {}",
            installation.engine.as_key(),
            installation.version
        ));
    }
    Ok(ok(correlation_id, ready))
}

#[tauri::command]
pub fn copy_profile_cookies(
    state: State<AppState>,
    request: CopyCookiesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<CopyCookiesResponse>, String> {
    let source_id =
        Uuid::parse_str(&request.source_profile_id).map_err(|e| format!("profile id: {e}"))?;
    if request.target_profile_ids.is_empty() {
        return Err("target profile list is empty".to_string());
    }

    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    manager
        .ensure_unlocked(source_id)
        .map_err(|_| ERR_LOCKED_REQUIRES_UNLOCK.to_string())?;
    let source = manager.get_profile(source_id).map_err(|e| e.to_string())?;
    drop(manager);

    if is_profile_running(&state, source_id)? {
        return Err("source profile must be stopped before copying cookies".to_string());
    }

    let mut copied_targets = 0usize;
    let mut skipped_targets = Vec::new();
    for raw_id in request.target_profile_ids {
        let target_id = Uuid::parse_str(&raw_id).map_err(|e| format!("profile id: {e}"))?;
        if target_id == source_id {
            skipped_targets.push(raw_id);
            continue;
        }

        let manager = state
            .manager
            .lock()
            .map_err(|_| "lock poisoned".to_string())?;
        manager
            .ensure_unlocked(target_id)
            .map_err(|_| ERR_LOCKED_REQUIRES_UNLOCK.to_string())?;
        let target = manager.get_profile(target_id).map_err(|e| e.to_string())?;
        drop(manager);
        if !cookies_copy_allowed(&source, &target) {
            return Err(ERR_COOKIES_COPY_BLOCKED.to_string());
        }

        if target.engine != source.engine {
            skipped_targets.push(target_id.to_string());
            continue;
        }
        if is_profile_running(&state, target_id)? {
            skipped_targets.push(target_id.to_string());
            continue;
        }

        copy_engine_cookies(
            source.engine.clone(),
            &state
                .profile_root
                .join(source_id.to_string())
                .join("engine-profile"),
            &state
                .profile_root
                .join(target_id.to_string())
                .join("engine-profile"),
        )?;
        copied_targets += 1;
    }

    Ok(ok(
        correlation_id,
        CopyCookiesResponse {
            copied_targets,
            skipped_targets,
        },
    ))
}

#[tauri::command]
pub fn set_profile_password(
    state: State<AppState>,
    request: LockProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    manager
        .set_profile_password(profile_id, &request.password, None)
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn unlock_profile(
    state: State<AppState>,
    request: UnlockProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let unlocked = manager
        .unlock_profile(profile_id, &request.password)
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, unlocked))
}

#[tauri::command]
pub fn validate_profile_modal(
    payload: ProfileModalPayload,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    validate_modal_payload(&payload).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn pick_certificate_files(correlation_id: String) -> Result<UiEnvelope<Vec<String>>, String> {
    #[cfg(target_os = "windows")]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Filter = 'Certificates (*.pem;*.crt;*.cer)|*.pem;*.crt;*.cer'
$dialog.Multiselect = $true
$dialog.CheckFileExists = $true
$dialog.CheckPathExists = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  $dialog.FileNames | ConvertTo-Json -Compress
}
"#;
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", script])
            .output()
            .map_err(|e| format!("certificate picker failed: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                "certificate picker failed".to_string()
            } else {
                format!("certificate picker failed: {stderr}")
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            return Ok(ok(correlation_id, Vec::new()));
        }
        let files = serde_json::from_str::<Vec<String>>(&stdout)
            .or_else(|_| serde_json::from_str::<String>(&stdout).map(|item| vec![item]))
            .map_err(|e| format!("certificate picker parse failed: {e}"))?;
        return Ok(ok(correlation_id, files));
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("certificate picker is not supported on this platform".to_string())
    }
}

#[tauri::command]
pub fn cancel_engine_download(
    state: State<AppState>,
    engine: String,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let normalized = engine.trim().to_lowercase();
    if normalized.is_empty() {
        return Err("engine is required".to_string());
    }
    let mut cancelled = state
        .cancelled_engine_downloads
        .lock()
        .map_err(|_| "cancelled engine download lock poisoned".to_string())?;
    cancelled.insert(normalized);
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn export_profile(
    state: State<AppState>,
    request: ExportProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ExportProfileResponse>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let profile = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    let files = collect_profile_data_files(&state.profile_root, profile_id)?;
    let archive =
        export_profile_archive(&profile, files, &request.passphrase).map_err(|e| e.to_string())?;
    let archive_json = serde_json::to_string(&archive).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, ExportProfileResponse { archive_json }))
}

#[tauri::command]
pub fn import_profile(
    state: State<AppState>,
    request: ImportProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ImportProfileResponse>, String> {
    let archive: EncryptedProfileArchive =
        serde_json::from_str(&request.archive_json).map_err(|e| e.to_string())?;
    let expected_id =
        Uuid::parse_str(&request.expected_profile_id).map_err(|e| format!("profile id: {e}"))?;
    let payload = import_profile_archive(&archive, expected_id, &request.passphrase)
        .map_err(|e| e.to_string())?;

    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let imported = manager
        .create_profile(CreateProfileInput {
            name: payload.metadata.name,
            description: payload.metadata.description,
            tags: payload.metadata.tags,
            engine: payload.metadata.engine,
            default_start_page: payload.metadata.default_start_page,
            default_search_provider: payload.metadata.default_search_provider,
            ephemeral_mode: payload.metadata.ephemeral_mode,
            password_lock_enabled: false,
            panic_frame_enabled: payload.metadata.panic_frame_enabled,
            panic_frame_color: payload.metadata.panic_frame_color,
            panic_protected_sites: payload.metadata.panic_protected_sites,
            ephemeral_retain_paths: payload.metadata.ephemeral_retain_paths,
        })
        .map_err(|e| e.to_string())?;

    write_imported_files(&state.profile_root, imported.id, payload.files)?;

    Ok(ok(
        correlation_id,
        ImportProfileResponse { profile: imported },
    ))
}

fn parse_engine(engine: &str) -> Result<Engine, String> {
    match engine {
        "wayfern" => Ok(Engine::Wayfern),
        "camoufox" => Ok(Engine::Camoufox),
        _ => Err(format!("unsupported engine: {engine}")),
    }
}

fn engine_session_key(engine: &Engine) -> &'static str {
    match engine {
        Engine::Wayfern => "wayfern",
        Engine::Camoufox => "camoufox",
    }
}

fn engine_kind(engine: Engine) -> EngineKind {
    match engine {
        Engine::Wayfern => EngineKind::Wayfern,
        Engine::Camoufox => EngineKind::Camoufox,
    }
}

fn parse_state(state: &str) -> Result<ProfileState, String> {
    match state {
        "created" => Ok(ProfileState::Created),
        "ready" => Ok(ProfileState::Ready),
        "running" => Ok(ProfileState::Running),
        "stopped" => Ok(ProfileState::Stopped),
        "locked" => Ok(ProfileState::Locked),
        "error" => Ok(ProfileState::Error),
        _ => Err(format!("unsupported state: {state}")),
    }
}

fn patch_state(
    state: &State<AppState>,
    request: &ActionProfileRequest,
    correlation_id: String,
    target: ProfileState,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let profile_id =
        Uuid::parse_str(&request.profile_id).map_err(|e| format!("profile id: {e}"))?;
    let profile = manager
        .update_profile(
            profile_id,
            PatchProfileInput {
                state: Some(target),
                ..PatchProfileInput::default()
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, profile))
}

fn collect_profile_data_files(
    root: &PathBuf,
    profile_id: Uuid,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let data_root = root.join(profile_id.to_string()).join("data");
    let mut files = Vec::new();
    if !data_root.exists() {
        return Ok(files);
    }
    collect_files_recursive(&data_root, &data_root, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    base: &PathBuf,
    current: &PathBuf,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(base, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            out.push((rel, bytes));
        }
    }
    Ok(())
}

fn write_imported_files(
    root: &PathBuf,
    profile_id: Uuid,
    files: Vec<browser_import_export::ExportFile>,
) -> Result<(), String> {
    let base = root.join(profile_id.to_string()).join("data");
    for file in files {
        let target = base.join(file.relative_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, file.content_b64)
                .map_err(|e| e.to_string())?;
        fs::write(target, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn is_profile_running(state: &AppState, profile_id: Uuid) -> Result<bool, String> {
    let launched = state
        .launched_processes
        .lock()
        .map_err(|_| "launch map lock poisoned".to_string())?;
    let pid = launched.get(&profile_id).copied();
    drop(launched);

    let Some(pid) = pid else {
        return Ok(false);
    };
    if is_pid_running(pid) {
        return Ok(true);
    }

    clear_profile_process(&state.app_handle, profile_id, pid, true);
    Ok(false)
}

fn copy_engine_cookies(
    engine: Engine,
    source_root: &PathBuf,
    target_root: &PathBuf,
) -> Result<(), String> {
    fs::create_dir_all(target_root).map_err(|e| e.to_string())?;
    let copied = match engine {
        Engine::Wayfern => {
            copy_cookie_path(source_root, target_root, "Default\\Network\\Cookies")?
                | copy_cookie_path(
                    source_root,
                    target_root,
                    "Default\\Network\\Cookies-journal",
                )?
                | copy_cookie_path(source_root, target_root, "Default\\Cookies")?
                | copy_cookie_path(source_root, target_root, "Default\\Cookies-journal")?
        }
        Engine::Camoufox => {
            copy_cookie_path(source_root, target_root, "cookies.sqlite")?
                | copy_cookie_path(source_root, target_root, "cookies.sqlite-wal")?
                | copy_cookie_path(source_root, target_root, "cookies.sqlite-shm")?
        }
    };

    if !copied {
        return Err("source profile does not contain cookie store files yet".to_string());
    }
    Ok(())
}

fn copy_cookie_path(
    source_root: &PathBuf,
    target_root: &PathBuf,
    relative: &str,
) -> Result<bool, String> {
    let source = source_root.join(relative);
    if !source.exists() {
        return Ok(false);
    }
    let target = target_root.join(relative);
    if source.is_dir() {
        copy_dir_recursive(&source, &target)?;
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(&source, &target).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

fn copy_dir_recursive(source: &PathBuf, target: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&source_path, &target_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

async fn ensure_engine_ready(
    app_handle: &tauri::AppHandle,
    state: &AppState,
    runtime: &EngineRuntime,
    engine: EngineKind,
) -> Result<EngineInstallation, String> {
    let key = engine.as_key().to_string();
    loop {
        let started_here = {
            if let Ok(mut cancelled) = state.cancelled_engine_downloads.lock() {
                cancelled.remove(&key);
            }
            let mut active = state
                .active_engine_downloads
                .lock()
                .map_err(|_| "engine download lock poisoned".to_string())?;
            if active.contains(&key) {
                false
            } else {
                active.insert(key.clone());
                true
            }
        };

        if started_here {
            let app_handle = app_handle.clone();
            let progress_handle = app_handle.clone();
            let runtime = runtime.clone();
            let cancel_state = state.cancelled_engine_downloads.clone();
            let key_for_cancel = key.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                runtime.ensure_ready(
                    engine,
                    |progress| {
                        let _ = progress_handle.emit("engine-download-progress", progress);
                    },
                    || {
                        cancel_state
                            .lock()
                            .map(|cancelled| cancelled.contains(&key_for_cancel))
                            .unwrap_or(false)
                    },
                )
            })
            .await
            .map_err(|e| e.to_string())?;
            let mut active = state
                .active_engine_downloads
                .lock()
                .map_err(|_| "engine download lock poisoned".to_string())?;
            active.remove(&key);
            if let Err(error) = &result {
                let is_cancelled = error
                    .to_string()
                    .to_lowercase()
                    .contains("interrupted by user");
                let _ = app_handle.emit(
                    "engine-download-progress",
                    EngineDownloadProgress {
                        engine,
                        version: "pending".to_string(),
                        stage: if is_cancelled {
                            "cancelled".to_string()
                        } else {
                            "error".to_string()
                        },
                        host: None,
                        downloaded_bytes: 0,
                        total_bytes: None,
                        percentage: 0.0,
                        speed_bytes_per_sec: 0.0,
                        eta_seconds: None,
                        message: Some(if is_cancelled {
                            "Download interrupted by user.".to_string()
                        } else {
                            error.to_string()
                        }),
                    },
                );
            }
            return result.map_err(|e| e.to_string());
        }

        let runtime_check =
            EngineRuntime::new(state.engine_runtime_root.clone()).map_err(|e| e.to_string())?;
        if let Some(install) = runtime_check.installed(engine).map_err(|e| e.to_string())? {
            return Ok(install);
        }
        thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_firefox_search_plugin_xml, extension_allowed_for_launch,
        firefox_search_engine_policy_entries, normalize_start_page_url,
        prepare_camoufox_profile_runtime,
    };
    use crate::state::ExtensionLibraryItem;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn camoufox_profile_runtime_applies_homepage_and_search_provider() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("cerbena-camoufox-profile-{unique}"));
        prepare_camoufox_profile_runtime(
            &temp_dir,
            Some("https://duckduckgo.com"),
            Some("startpage"),
            None,
            false,
        )
        .expect("prepare camoufox profile runtime");

        let user_js = fs::read_to_string(temp_dir.join("user.js")).expect("read user.js");
        assert!(user_js.contains("browser.startup.homepage\", \"https://duckduckgo.com\""));
        assert!(user_js.contains("browser.newtab.url\", \"https://duckduckgo.com\""));
        assert!(user_js.contains("browser.search.defaultenginename\", \"Startpage\""));
        assert!(user_js.contains("browser.search.defaultEngine\", \"Startpage\""));
        assert!(user_js.contains("browser.search.selectedEngine\", \"Startpage\""));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn camoufox_profile_runtime_applies_hardening_when_requested() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("cerbena-camoufox-hardened-{unique}"));
        prepare_camoufox_profile_runtime(
            &temp_dir,
            Some("https://duckduckgo.com"),
            Some("duckduckgo"),
            None,
            true,
        )
        .expect("prepare camoufox hardened runtime");

        let user_js = fs::read_to_string(temp_dir.join("user.js")).expect("read user.js");
        assert!(user_js.contains("signon.rememberSignons\", false"));
        assert!(user_js.contains("browser.formfill.enable\", false"));
        assert!(user_js.contains("browser.sessionstore.privacy_level\", 2"));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn firefox_search_policy_catalog_contains_supported_defaults() {
        let entries = firefox_search_engine_policy_entries();
        let names = entries
            .iter()
            .filter_map(|entry| entry.get("Name").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();
        assert!(names.contains(&"DuckDuckGo"));
        assert!(names.contains(&"Google"));
        assert!(names.contains(&"Startpage"));
        assert!(entries.iter().all(|entry| entry.get("URLTemplate").is_some()));
    }

    #[test]
    fn normalize_start_page_url_adds_https_for_plain_host() {
        assert_eq!(
            normalize_start_page_url(Some("duckduckgo.com")),
            "https://duckduckgo.com"
        );
        assert_eq!(
            normalize_start_page_url(Some("https://example.com")),
            "https://example.com"
        );
        assert_eq!(
            normalize_start_page_url(Some("about:blank")),
            "about:blank"
        );
    }

    #[test]
    fn firefox_search_plugin_xml_contains_engine_name_and_url() {
        let xml = build_firefox_search_plugin_xml(
            "DuckDuckGo",
            "https://duckduckgo.com/?q={searchTerms}",
            Some("https://duckduckgo.com/ac/?q={searchTerms}&type=list"),
        );
        assert!(xml.contains("<ShortName>DuckDuckGo</ShortName>"));
        assert!(xml.contains("https://duckduckgo.com/?q={searchTerms}"));
        assert!(xml.contains("application/x-suggestions+json"));
    }

    fn sample_extension(
        id: &str,
        display_name: &str,
        store_url: Option<&str>,
        tags: &[&str],
    ) -> ExtensionLibraryItem {
        ExtensionLibraryItem {
            id: id.to_string(),
            display_name: display_name.to_string(),
            version: "1.0.0".to_string(),
            engine_scope: "chromium".to_string(),
            source_kind: "url".to_string(),
            source_value: store_url.unwrap_or_default().to_string(),
            logo_url: None,
            store_url: store_url.map(str::to_string),
            tags: tags.iter().map(|value| value.to_string()).collect(),
            assigned_profile_ids: Vec::new(),
            auto_update_enabled: false,
            preserve_on_panic_wipe: false,
            protect_data_from_panic_wipe: false,
            package_path: None,
            package_file_name: None,
        }
    }

    #[test]
    fn keepassxc_requires_explicit_profile_allowance() {
        let keepass = sample_extension(
            "oboonakemofpalcgghocfoadofidjkkk",
            "KeePassXC-Browser",
            Some("https://chromewebstore.google.com/detail/keepassxc-browser/oboonakemofpalcgghocfoadofidjkkk"),
            &[],
        );
        assert!(!extension_allowed_for_launch(&keepass, false, false, false));
        assert!(extension_allowed_for_launch(&keepass, false, true, false));
        assert!(!extension_allowed_for_launch(&keepass, true, false, false));
    }

    #[test]
    fn disable_extensions_launch_blocks_non_keepassxc_extensions() {
        let regular = sample_extension(
            "plain-extension",
            "Sample Extension",
            Some("https://example.invalid/sample"),
            &[],
        );
        assert!(!extension_allowed_for_launch(&regular, false, false, true));
        assert!(!extension_allowed_for_launch(&regular, true, true, true));
    }

    #[test]
    fn system_access_extensions_require_opt_in() {
        let native_extension = sample_extension(
            "native-extension",
            "Native Bridge",
            Some("https://example.invalid/native"),
            &["native-messaging"],
        );
        assert!(!extension_allowed_for_launch(
            &native_extension,
            false,
            false,
            false
        ));
        assert!(extension_allowed_for_launch(
            &native_extension,
            true,
            false,
            false
        ));
    }
}
