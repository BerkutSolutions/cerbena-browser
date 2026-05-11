use super::*;

pub(crate) fn updater_launch_mode_from_state(state: &AppState) -> Result<UpdaterLaunchMode, String> {
    updater_lifecycle::updater_launch_mode_from_state_impl(state)
}

#[cfg(test)]
pub(crate) fn default_auto_update_enabled() -> bool {
    updater_core_types::default_auto_update_enabled()
}

pub(crate) fn schedule_updater_window_close_for_apply(state: &AppState) {
    updater_lifecycle::schedule_updater_window_close_for_apply_impl(state)
}

pub(crate) fn should_auto_close_updater_after_ready_to_restart(launch_mode: UpdaterLaunchMode) -> bool {
    updater_lifecycle::should_auto_close_updater_after_ready_to_restart_impl(launch_mode)
}

pub(crate) fn run_updater_flow(state: &AppState, launch_mode: UpdaterLaunchMode) -> Result<(), String> {
    updater_lifecycle::run_updater_flow_impl(state, launch_mode)
}

pub(crate) fn should_launch_external_updater(store: &AppUpdateStore, candidate: &ReleaseCandidate) -> bool {
    updater_lifecycle::should_launch_external_updater_impl(store, candidate)
}

#[cfg(test)]
pub(crate) fn should_run_auto_update_check(store: &AppUpdateStore) -> bool {
    updater_lifecycle::should_run_auto_update_check_impl(store)
}

pub(crate) fn spawn_updater_process(app: &AppHandle, mode: UpdaterLaunchMode) -> Result<(), String> {
    updater_lifecycle::spawn_updater_process_impl(app, mode)
}

pub(crate) fn resolve_updater_executable_path(app: &AppHandle) -> Result<PathBuf, String> {
    updater_lifecycle::resolve_updater_executable_path_impl(app)
}

pub(crate) fn current_windows_install_mode() -> String {
    updater_lifecycle::current_windows_install_mode_impl()
}

pub fn launch_pending_update_on_exit(app: &AppHandle) {
    updater_lifecycle::launch_pending_update_on_exit_impl(app);
}

pub(crate) fn run_preview_updater_flow(state: &AppState) -> Result<(), String> {
    updater_flow::run_preview_updater_flow_impl(state)
}

pub(crate) fn run_live_updater_flow(state: &AppState) -> Result<(), String> {
    updater_flow::run_live_updater_flow_impl(state)
}

pub(crate) fn run_update_cycle(state: &AppState, manual: bool) -> Result<AppUpdateView, String> {
    updater_flow::run_update_cycle_impl(state, manual)
}

pub(crate) fn stage_release_if_needed(
    state: &AppState,
    store: &mut AppUpdateStore,
    candidate: &ReleaseCandidate,
) -> Result<(), String> {
    updater_transfer::stage_release_if_needed_impl(state, store, candidate)
}

pub(crate) fn clear_staged_update(store: &mut AppUpdateStore) {
    store.staged_version = None;
    store.staged_asset_name = None;
    store.staged_asset_path = None;
    store.selected_asset_type = None;
    store.selected_asset_reason = None;
    store.install_handoff_mode = None;
    store.pending_apply_on_exit = false;
}

pub(crate) fn stage_verified_release_asset(
    state: &AppState,
    candidate: &ReleaseCandidate,
    asset_bytes: &[u8],
) -> Result<PathBuf, String> {
    updater_transfer::stage_verified_release_asset_impl(state, candidate, asset_bytes)
}

pub(crate) fn update_updater_overview<F>(state: &AppState, mut change: F) -> Result<(), String>
where
    F: FnMut(&mut UpdaterOverview),
{
    updater_reporting::update_updater_overview_impl(state, |overview| change(overview))
}

pub(crate) fn progress_updater_step(
    state: &AppState,
    step_id: &str,
    status: &str,
    detail: &str,
) -> Result<(), String> {
    updater_reporting::progress_updater_step_impl(state, step_id, status, detail)
}

pub(crate) fn mark_remaining_updater_steps_skipped(
    state: &AppState,
    from_step_id: &str,
) -> Result<(), String> {
    updater_reporting::mark_remaining_updater_steps_skipped_impl(state, from_step_id)
}

pub(crate) fn finalize_updater_success(
    state: &AppState,
    status: &str,
    summary_key: &str,
    summary_detail: &str,
    close_label_key: &str,
) -> Result<(), String> {
    updater_reporting::finalize_updater_success_impl(
        state,
        status,
        summary_key,
        summary_detail,
        close_label_key,
    )
}

pub(crate) fn finalize_updater_failure(state: &AppState, error: &str) -> Result<(), String> {
    updater_reporting::finalize_updater_failure_impl(state, error)
}

pub(crate) fn fetch_latest_release(client: &Client) -> Result<ReleaseCandidate, String> {
    updater_discovery::fetch_latest_release_impl(client)
}

pub(crate) fn fetch_latest_release_from_url(
    client: &Client,
    latest_release_url: &str,
) -> Result<ReleaseCandidate, String> {
    updater_discovery::fetch_latest_release_from_url_impl(client, latest_release_url)
}

#[allow(dead_code)]
pub(crate) fn pick_release_asset(assets: &[GithubReleaseAsset]) -> Option<SelectedReleaseAsset<'_>> {
    updater_discovery::pick_release_asset_impl(assets)
}

#[allow(dead_code)]
pub(crate) fn pick_release_asset_for_context<'a>(
    assets: &'a [GithubReleaseAsset],
    os: &str,
    windows_install_mode: Option<&str>,
) -> Option<SelectedReleaseAsset<'a>> {
    updater_discovery::pick_release_asset_for_context_impl(assets, os, windows_install_mode)
}

#[allow(dead_code)]
pub(crate) fn classify_release_asset<'a>(
    os: &str,
    asset: &'a GithubReleaseAsset,
) -> Option<SelectedReleaseAsset<'a>> {
    updater_discovery::classify_release_asset_impl(os, asset)
}

#[allow(dead_code)]
pub(crate) fn verify_release_candidate(
    candidate: &ReleaseCandidate,
    asset_bytes: &[u8],
) -> Result<(), String> {
    updater_verification::verify_release_candidate_impl(candidate, asset_bytes)
}

pub(crate) struct VerifiedReleaseSecurityBundle {
    pub(crate) checksums_text: String,
}

#[cfg(test)]
pub(crate) fn release_signing_public_keys() -> Vec<String> {
    updater_verification::release_signing_public_keys_impl()
}

#[cfg(test)]
pub(crate) fn signature_verification_variants(checksums_bytes: &[u8]) -> Vec<Vec<u8>> {
    updater_verification::signature_verification_variants_impl(checksums_bytes)
}

pub(crate) fn verify_release_security_bundle(
    candidate: &ReleaseCandidate,
) -> Result<VerifiedReleaseSecurityBundle, String> {
    updater_verification::verify_release_security_bundle_impl(candidate)
}

pub(crate) fn ensure_asset_matches_verified_checksum(
    security_bundle: &VerifiedReleaseSecurityBundle,
    asset_name: &str,
    asset_bytes: &[u8],
) -> Result<(), String> {
    updater_verification::ensure_asset_matches_verified_checksum_impl(
        security_bundle,
        asset_name,
        asset_bytes,
    )
}

pub(crate) fn download_release_bytes(client: &Client, url: &str, label: &str) -> Result<Vec<u8>, String> {
    updater_transfer::download_release_bytes_impl(client, url, label)
}

pub(crate) fn build_release_http_client(
    timeout: Duration,
    disable_auto_decompression: bool,
) -> Result<Client, String> {
    let mut builder = Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(20))
        .user_agent(USER_AGENT);
    if disable_auto_decompression {
        builder = builder.no_gzip().no_brotli().no_deflate();
    }
    builder
        .build()
        .map_err(|e| format!("build release http client: {e}"))
}

pub(crate) fn describe_http_failure(response: Response, context: &str, url: Option<&str>) -> String {
    let status = response.status();
    let headers = response.headers().clone();
    let request_id = headers
        .get("x-github-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .trim()
        .to_string();
    let content_type = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .trim()
        .to_string();
    let body = response.text().unwrap_or_default();
    let body_snippet = sanitize_http_error_body(&body);
    let mut parts = vec![format!("{context} failed with HTTP {status}")];
    if let Some(target) = url.filter(|value| !value.trim().is_empty()) {
        parts.push(format!("url={target}"));
    }
    if !request_id.is_empty() {
        parts.push(format!("request_id={request_id}"));
    }
    if !content_type.is_empty() {
        parts.push(format!("content_type={content_type}"));
    }
    if !body_snippet.is_empty() {
        parts.push(format!("body={body_snippet}"));
    }
    parts.join(" | ")
}

pub(crate) fn sanitize_http_error_body(body: &str) -> String {
    let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut snippet = trimmed.chars().take(600).collect::<String>();
    if trimmed.chars().count() > 600 {
        snippet.push_str("...");
    }
    snippet
}

#[allow(dead_code)]
pub(crate) fn verify_release_checksums_signature(
    checksums_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<(), String> {
    updater_verification::verify_release_checksums_signature_impl(checksums_bytes, signature_bytes)
}

#[allow(dead_code)]
pub(crate) fn extract_checksum_for_asset<'a>(checksums_text: &'a str, asset_name: &str) -> Option<&'a str> {
    updater_verification::extract_checksum_for_asset_impl(checksums_text, asset_name)
}

pub(crate) fn asset_rank(kind: SelectedAssetKind) -> u8 {
    match kind {
        SelectedAssetKind::WindowsMsi => 0,
        SelectedAssetKind::WindowsZip => 1,
        SelectedAssetKind::WindowsExe => 2,
        SelectedAssetKind::LinuxTarGz => 0,
        SelectedAssetKind::LinuxZip => 1,
        SelectedAssetKind::MacZip => 0,
    }
}

pub(crate) fn can_auto_apply_asset(name: &str) -> bool {
    can_auto_apply_asset_for_os(std::env::consts::OS, name)
}

pub(crate) fn can_auto_apply_asset_for_os(os: &str, name: &str) -> bool {
    if os != "windows" {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".msi")
}

pub(crate) fn uses_msi_installer(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".msi")
}

pub(crate) fn resolve_latest_release_api_url() -> String {
    updater_state::resolve_latest_release_api_url_impl()
}

pub(crate) fn resolve_update_asset_root(state: &AppState, asset_name: &str) -> Result<PathBuf, String> {
    if uses_msi_installer(asset_name) {
        let app_root = app_local_data_root(&state.app_handle)
            .map_err(|e| format!("resolve app local data root for msi staging: {e}"))?;
        let namespace_hash = sha256_hex(app_root.to_string_lossy().as_bytes());
        let namespace = &namespace_hash[..16];
        return Ok(std::env::temp_dir()
            .join("cerbena-browser-updates")
            .join(namespace));
    }
    state
        .app_update_root_path(&state.app_handle)
        .map_err(|e| format!("resolve update root path: {e}"))
}

pub(crate) fn launch_zip_apply_helper(
    pid: u32,
    archive_path: &Path,
    install_root: &Path,
    relaunch_executable: Option<&Path>,
    runtime_log_path: Option<&str>,
) -> Result<(), String> {
    updater_apply::launch_zip_apply_helper_impl(
        pid,
        archive_path,
        install_root,
        relaunch_executable,
        runtime_log_path,
    )
}

#[allow(dead_code)]
pub(crate) fn build_zip_apply_helper_script(
    pid: u32,
    archive_path: &Path,
    install_root: &Path,
    relaunch_executable: Option<&Path>,
    runtime_log_path: Option<&str>,
) -> String {
    updater_apply::build_zip_apply_helper_script_impl(
        pid,
        archive_path,
        install_root,
        relaunch_executable,
        runtime_log_path,
    )
}

pub(crate) fn launch_msi_apply_helper(
    pid: u32,
    msi_path: &Path,
    target_install_root: Option<&Path>,
    update_store_path: Option<&str>,
    target_version: Option<&str>,
    runtime_log_path: Option<&str>,
) -> Result<(), String> {
    updater_apply::launch_msi_apply_helper_impl(
        pid,
        msi_path,
        target_install_root,
        update_store_path,
        target_version,
        runtime_log_path,
    )
}

#[allow(dead_code)]
pub(crate) fn build_msi_apply_helper_script(
    pid: u32,
    msi_path: &Path,
    target_install_root: Option<&Path>,
    update_store_path: Option<&str>,
    target_version: Option<&str>,
    runtime_log_path: Option<&str>,
) -> String {
    updater_apply::build_msi_apply_helper_script_impl(
        pid,
        msi_path,
        target_install_root,
        update_store_path,
        target_version,
        runtime_log_path,
    )
}

pub(crate) fn resolve_relaunch_executable_path(install_root: &Path) -> Option<PathBuf> {
    updater_apply::resolve_relaunch_executable_path_impl(install_root)
}

pub(crate) fn persist_update_store_from_state(state: &AppState, store: &AppUpdateStore) -> Result<(), String> {
    updater_state::persist_update_store_from_state_impl(state, store)
}

pub(crate) fn refresh_update_store_snapshot(state: &AppState) -> Result<AppUpdateStore, String> {
    updater_state::refresh_update_store_snapshot_impl(state)
}

#[allow(dead_code)]
pub(crate) fn reconcile_update_store_with_current_version(store: &mut AppUpdateStore) {
    updater_state::reconcile_update_store_with_current_version_impl(store)
}

pub(crate) fn write_update_store_snapshot(state: &AppState, store: &AppUpdateStore) -> Result<(), String> {
    updater_state::write_update_store_snapshot_impl(state, store)
}

pub(crate) fn to_view(store: &AppUpdateStore) -> AppUpdateView {
    updater_state::to_view_impl(store)
}

pub(crate) fn now_epoch_ms() -> u128 {
    updater_state::now_epoch_ms_impl()
}

pub(crate) fn now_iso() -> String {
    updater_state::now_iso_impl()
}

pub(crate) fn normalize_version(raw: &str) -> String {
    updater_state::normalize_version_impl(raw)
}

pub(crate) fn is_version_newer(candidate: &str, current: &str) -> bool {
    updater_state::is_version_newer_impl(candidate, current)
}

#[allow(dead_code)]
pub(crate) fn parse_version_parts(value: &str) -> Vec<u64> {
    updater_state::parse_version_parts_impl(value)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    updater_state::sha256_hex_impl(bytes)
}

pub(crate) fn powershell_quote(path: &Path) -> String {
    updater_state::powershell_quote_impl(path)
}
#[cfg(test)]
#[path = "update_commands_tests.rs"]
mod tests;

