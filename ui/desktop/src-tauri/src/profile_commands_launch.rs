use super::*;

#[path = "profile_commands_launch_extensions.rs"]
mod launch_extensions;
#[path = "profile_commands_launch_flow.rs"]
mod launch_flow;
#[path = "profile_commands_launch_plan.rs"]
mod launch_plan;
#[path = "profile_commands_launch_policy.rs"]
mod launch_policy;
#[path = "profile_commands_launch_policy_writes.rs"]
mod launch_policy_writes;
#[path = "profile_commands_launch_preflight.rs"]
mod preflight;
#[path = "profile_commands_launch_runtime_prep.rs"]
mod runtime_prep;
#[path = "profile_commands_launch_session.rs"]
mod session;
#[path = "profile_commands_launch_stop.rs"]
mod stop;
#[path = "profile_commands_launch_support.rs"]
mod support;

pub(crate) use self::normalize_start_page_url_impl as normalize_start_page_url;
pub(crate) use self::stop::stop_profile_impl;

pub(crate) struct LaunchContext {
    pub(crate) profile_id: Uuid,
    pub(crate) launch_url_requested: bool,
    pub(crate) profile: ProfileMetadata,
    pub(crate) profile_root: PathBuf,
    pub(crate) user_data_dir: PathBuf,
    pub(crate) identity_policy_hash: Option<String>,
    pub(crate) session_engine: String,
}

pub(crate) async fn launch_profile_impl(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ActionProfileRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ProfileMetadata>, String> {
    launch_flow::launch_profile_impl(app_handle, state, request, correlation_id).await
}

pub(crate) fn emit_profile_launch_progress(
    app_handle: &tauri::AppHandle,
    profile_id: Uuid,
    stage_key: &str,
    message_key: &str,
    done: bool,
    error: Option<&str>,
) {
    support::emit_profile_launch_progress(
        app_handle,
        profile_id,
        stage_key,
        message_key,
        done,
        error,
    );
}

pub(crate) fn wait_for_profile_process_startup_impl(
    user_data_dir: &Path,
    spawned_pid: u32,
    engine: EngineKind,
    prelaunch_profile_pids: &std::collections::BTreeSet<u32>,
) -> Result<u32, String> {
    support::wait_for_profile_process_startup_impl(
        user_data_dir,
        spawned_pid,
        engine,
        prelaunch_profile_pids,
    )
}

pub(crate) fn prepare_librewolf_profile_runtime_impl(
    profile_dir: &Path,
    default_start_page: Option<&str>,
    default_search_provider: Option<&str>,
    gateway_proxy_port: Option<u16>,
    runtime_hardening: bool,
    identity_preset: Option<&IdentityPreset>,
) -> Result<(), std::io::Error> {
    support::prepare_librewolf_profile_runtime_impl(
        profile_dir,
        default_start_page,
        default_search_provider,
        gateway_proxy_port,
        runtime_hardening,
        identity_preset,
    )
}

pub(crate) fn emit_librewolf_launch_diagnostics(stage: &str, profile_dir: &Path) {
    support::emit_librewolf_launch_diagnostics(stage, profile_dir);
}

pub(crate) fn profile_runtime_has_session_state_impl(engine: EngineKind, profile_dir: &Path) -> bool {
    support::profile_runtime_has_session_state_impl(engine, profile_dir)
}

pub(crate) fn load_identity_preset_for_profile_impl(state: &AppState, profile_id: Uuid) -> Option<IdentityPreset> {
    support::load_identity_preset_for_profile_impl(state, profile_id)
}

pub(crate) fn normalize_start_page_url_impl(default_start_page: Option<&str>) -> String {
    support::normalize_start_page_url_impl(default_start_page)
}

pub(crate) fn normalize_optional_start_page_url_impl(default_start_page: Option<&str>) -> Option<String> {
    support::normalize_optional_start_page_url_impl(default_start_page)
}


pub(crate) fn map_search_provider_to_firefox_engine_impl(provider: Option<&str>) -> Option<&'static str> {
    support::map_search_provider_to_firefox_engine_impl(provider)
}

#[allow(dead_code)]
pub(crate) fn firefox_search_engine_policy_entries_impl() -> Vec<serde_json::Value> {
    launch_policy_writes::firefox_search_engine_policy_entries_impl()
}

#[allow(dead_code)]
pub(crate) fn build_firefox_search_plugin_xml_impl(
    name: &str,
    url_template: &str,
    suggest_template: Option<&str>,
) -> String {
    launch_policy_writes::build_firefox_search_plugin_xml_impl(name, url_template, suggest_template)
}

pub(crate) fn prepare_profile_chromium_extensions_impl(
    state: &State<'_, AppState>,
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Result<(), String> {
    profile_extensions::prepare_profile_extensions_for_launch(state.inner(), profile, profile_root)
}

pub(crate) fn apply_librewolf_website_filter_impl(
    state: &State<'_, AppState>,
    profile_id: &Uuid,
    binary_path: &std::path::Path,
) -> Result<(), String> {
    launch_policy_writes::apply_librewolf_website_filter_impl(state, profile_id, binary_path)
}

pub(crate) fn write_profile_blocked_domains_impl(
    state: &State<'_, AppState>,
    profile_id: &Uuid,
    profile_root: &std::path::Path,
) -> Result<(), std::io::Error> {
    launch_policy_writes::write_profile_blocked_domains_impl(state, profile_id, profile_root)
}

pub(crate) fn neutralize_librewolf_builtin_theme_impl(
    binary_path: &std::path::Path,
) -> Result<(), std::io::Error> {
    launch_policy_writes::neutralize_librewolf_builtin_theme_impl(binary_path)
}

