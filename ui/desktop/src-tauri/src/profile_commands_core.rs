use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use browser_engine::{EngineKind, EngineRuntime};
use browser_fingerprint::IdentityPreset;
use browser_import_export::{
    export_profile_archive, import_profile_archive, EncryptedProfileArchive,
};
use browser_profile::{
    validate_modal_payload, CreateProfileInput, Engine, PatchProfileInput, ProfileMetadata,
    ProfileModalPayload, ProfileState,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Emitter, State};
use uuid::Uuid;

use crate::{
    certificate_runtime::{
        clear_librewolf_profile_certificates, prepare_librewolf_profile_certificates_for_state,
    },
    device_posture::enforce_launch_posture,
    envelope::ok,
    envelope::UiEnvelope,
    keepassxc_bridge::ensure_keepassxc_bridge_for_profile,
    launch_sessions::{
        issue_launch_session, revoke_launch_session, trusted_session_for_profile,
        trusted_session_pid,
    },
    launcher_commands::{load_global_security_record, persist_global_security_record},
    network_sandbox_lifecycle::{ensure_profile_network_stack, stop_profile_network_stack},
    panic_frame::{close_panic_frame, maybe_start_panic_frame},
    platform::dialogs,
    process_tracking::{
        clear_profile_process, describe_profile_process_candidates,
        find_profile_main_window_pid_for_dir, find_profile_process_pid_for_dir,
        is_process_running as is_pid_running, terminate_process_tree, terminate_profile_processes,
        track_profile_process,
    },
    profile_extensions,
    profile_runtime_logs::{append_profile_log, read_profile_log_lines},
    profile_security::{
        assess_profile, cookies_copy_allowed, first_launch_blocker, ERR_COOKIES_COPY_BLOCKED,
        ERR_LOCKED_REQUIRES_UNLOCK, ERR_MAXIMUM_POLICY_EXTENSIONS_FORBIDDEN,
    },
    service_domains::service_domain_seeds,
    state::{
        ensure_default_profiles, is_builtin_default_profile_name,
        persist_hidden_default_profiles_store, AppState,
    },
};

const ERR_CHROMIUM_PROFILE_CERTIFICATES_UNSUPPORTED: &str =
    "profile.security.chromium_certificates_not_supported";

#[path = "profile_commands_create.rs"]
mod create;
#[path = "profile_commands_certificates.rs"]
mod certificates;
#[path = "profile_commands_delete.rs"]
mod delete;
#[path = "profile_commands_duplicate.rs"]
mod duplicate;
#[path = "profile_commands_engine.rs"]
mod engine;
#[path = "profile_commands_launch.rs"]
mod launch;
#[path = "profile_commands_list.rs"]
mod list;
#[path = "profile_commands_lock_set.rs"]
mod lock_set;
#[path = "profile_commands_lock_unlock.rs"]
mod lock_unlock;
#[path = "profile_commands_logs.rs"]
mod logs;
#[path = "profile_commands_modal_validation.rs"]
mod modal_validation;
#[path = "profile_commands_policy.rs"]
mod policy;
#[path = "profile_commands_state.rs"]
mod state_helpers;
#[path = "profile_commands_export.rs"]
mod export_flow;
#[path = "profile_commands_import.rs"]
mod import_flow;
#[path = "profile_commands_transfer.rs"]
mod transfer;
#[path = "profile_commands_update.rs"]
mod update;
#[path = "profile_commands_workspace.rs"]
mod workspace;
#[path = "profile_commands_util.rs"]
mod util;

pub(crate) use self::launch::normalize_start_page_url;
pub(crate) use self::policy::{
    persist_identity_applied_marker_impl as persist_identity_applied_marker,
    should_restart_for_identity_policy_impl as should_restart_for_identity_policy,
    write_locked_app_policy_impl as write_locked_app_policy,
    write_profile_identity_policy_impl as write_profile_identity_policy,
};
pub(crate) use self::state_helpers::{
    parse_state_impl as parse_state, patch_state_impl as patch_state,
};
pub(crate) use self::workspace::{
    global_startup_page_impl as global_startup_page,
    purge_profile_related_state_impl as purge_profile_related_state,
    reset_profile_runtime_workspace_impl as reset_profile_runtime_workspace,
};
pub(crate) use self::util::{
    engine_kind, engine_session_key, ensure_engine_ready,
    ensure_engine_supports_isolated_certificates, open_url_in_running_profile, parse_engine,
    parse_nullable_string_field, LockedAppPolicyRecord, NullableStringField,
};

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
    pub engine: Option<String>,
    pub state: Option<String>,
    #[serde(default)]
    pub default_start_page: NullableStringField,
    #[serde(default)]
    pub default_search_provider: NullableStringField,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityAppliedMarker {
    engine: String,
    identity_hash: String,
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
    list::list_profiles_impl(&state, correlation_id)
}

#[tauri::command]
pub fn create_profile(
    state: State<AppState>,
    request: CreateProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    create::create_profile_impl(&state, request, correlation_id)
}

#[tauri::command]
pub fn update_profile(
    state: State<AppState>,
    request: UpdateProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    update::update_profile_impl(&state, request, correlation_id)
}

#[tauri::command]
pub fn delete_profile(
    state: State<AppState>,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    delete::delete_profile_impl(&state, request, correlation_id)
}

#[tauri::command]
pub fn duplicate_profile(
    state: State<AppState>,
    request: DuplicateProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    duplicate::duplicate_profile_impl(&state, request, correlation_id)
}

#[tauri::command]
pub async fn launch_profile(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    launch::launch_profile_impl(app_handle, state, request, correlation_id).await
}

#[tauri::command]
pub fn stop_profile(
    state: State<AppState>,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    launch::stop_profile_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn read_profile_logs(
    app_handle: tauri::AppHandle,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<String>>, String> {
    logs::read_profile_logs_impl(app_handle, request, correlation_id)
}

#[tauri::command]
pub async fn ensure_engine_binaries(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<String>>, String> {
    engine::ensure_engine_binaries_impl(app_handle, state, correlation_id).await
}

#[tauri::command]
pub fn copy_profile_cookies(
    state: State<AppState>,
    request: CopyCookiesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<CopyCookiesResponse>, String> {
    transfer::copy_profile_cookies_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn set_profile_password(
    state: State<AppState>,
    request: LockProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    lock_set::set_profile_password_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn unlock_profile(
    state: State<AppState>,
    request: UnlockProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    lock_unlock::unlock_profile_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn validate_profile_modal(
    payload: ProfileModalPayload,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    modal_validation::validate_profile_modal_impl(payload, correlation_id)
}

#[tauri::command]
pub fn pick_certificate_files(correlation_id: String) -> Result<UiEnvelope<Vec<String>>, String> {
    certificates::pick_certificate_files_impl(correlation_id)
}

#[tauri::command]
pub fn cancel_engine_download(
    state: State<AppState>,
    engine: String,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    engine::cancel_engine_download_impl(state, engine, correlation_id)
}

#[tauri::command]
pub fn export_profile(
    state: State<AppState>,
    request: ExportProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ExportProfileResponse>, String> {
    export_flow::export_profile_impl(state, request, correlation_id)
}

#[tauri::command]
pub fn import_profile(
    state: State<AppState>,
    request: ImportProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ImportProfileResponse>, String> {
    import_flow::import_profile_impl(state, request, correlation_id)
}
