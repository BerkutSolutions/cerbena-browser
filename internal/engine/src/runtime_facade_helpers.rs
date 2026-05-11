#![allow(dead_code)]

use super::*;

pub(super) fn http_client() -> Result<Client, EngineError> {
    Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(60))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| EngineError::Download(e.to_string()))
}

pub(super) fn candidate_names(values: &[&str]) -> Vec<String> {
    runtime_binary::candidate_names_impl(values)
}

pub(super) fn find_first_match(root: &Path, candidates: &[String]) -> Option<PathBuf> {
    runtime_binary::find_first_match_impl(root, candidates)
}

pub(super) fn prefer_librewolf_browser_binary(current: &Path) -> PathBuf {
    runtime_binary::prefer_librewolf_browser_binary_impl(current)
}

pub(super) fn prefer_chromium_vendor_binary(current: &Path) -> PathBuf {
    runtime_binary::prefer_chromium_vendor_binary_impl(current)
}

pub(super) fn launch_args(
    engine: EngineKind,
    profile_root: &Path,
    start_page: Option<&str>,
    private_mode: bool,
    gateway_proxy_port: Option<u16>,
    runtime_hardening: bool,
) -> Result<Vec<String>, EngineError> {
    runtime_launch_policy::launch_args_impl(
        engine,
        profile_root,
        start_page,
        private_mode,
        gateway_proxy_port,
        runtime_hardening,
    )
}

pub(super) fn sanitize_librewolf_launch_args(args: &mut Vec<String>) {
    runtime_launch_firefox::sanitize_librewolf_launch_args_impl(args)
}

pub(super) fn reopen_args(
    engine: EngineKind,
    profile_root: &Path,
    url: &str,
) -> Result<Vec<String>, EngineError> {
    runtime_launch_policy::reopen_args_impl(engine, profile_root, url)
}

pub(super) fn load_identity_launch_policy(profile_root: &Path) -> Option<IdentityLaunchPolicy> {
    runtime_launch_policy::load_identity_launch_policy_impl(profile_root)
}

pub(super) fn apply_chromium_identity_args(
    args: &mut Vec<String>,
    profile_root: &Path,
) -> Result<(), EngineError> {
    runtime_launch_chromium::apply_chromium_identity_args_impl(profile_root, None, args)
}

pub(super) fn launch_environment(engine: EngineKind, profile_root: &Path) -> Vec<(String, String)> {
    runtime_launch_policy::launch_environment_impl(engine, profile_root)
}

#[cfg(target_os = "linux")]
pub(super) fn linux_requires_no_sandbox_for_binary(binary_path: &Path) -> bool {
    runtime_compat::linux_requires_no_sandbox_for_binary_impl(binary_path)
}

#[cfg(target_os = "linux")]
pub(super) fn linux_sandbox_probe_summary() -> String {
    runtime_compat::linux_sandbox_probe_summary_impl()
}

#[cfg(target_os = "linux")]
pub(super) fn linux_binary_allowlisted_in_cerbena_apparmor(binary_path: &Path) -> bool {
    runtime_compat::linux_binary_allowlisted_in_cerbena_apparmor_impl(binary_path)
}

pub(super) fn find_first_suffix_match(
    root: &Path,
    suffixes: &[&str],
    contains: Option<&str>,
) -> Option<PathBuf> {
    runtime_compat::find_first_suffix_match_impl(root, suffixes, contains)
}

pub(super) fn chromium_launch_environment(profile_root: &Path) -> Vec<(String, String)> {
    runtime_launch_policy::chromium_launch_environment_impl(profile_root)
}

pub(super) fn first_positive(primary: u32, fallback: u32) -> u32 {
    runtime_launch_policy::first_positive_impl(primary, fallback)
}

pub(super) fn normalize_primary_language(language: &str) -> Option<String> {
    runtime_launch_policy::normalize_primary_language_impl(language)
}

pub(super) fn normalize_accept_languages(primary: &str, languages: &[String]) -> Vec<String> {
    runtime_launch_policy::normalize_accept_languages_impl(primary, languages)
}

pub(super) fn identity_uses_native_user_agent(identity: &IdentityLaunchPolicy) -> bool {
    runtime_launch_policy::identity_uses_native_user_agent_impl(identity)
}

pub(super) fn build_accept_language_header(languages: &[String]) -> String {
    runtime_launch_policy::build_accept_language_header_impl(languages)
}

pub(super) fn write_chromium_language_preferences(
    profile_root: &Path,
    languages: &[String],
) -> Result<(), EngineError> {
    runtime_launch_policy::write_chromium_language_preferences_impl(profile_root, languages)
}

pub(super) fn write_chromium_local_state_locale(
    profile_root: &Path,
    languages: &[String],
) -> Result<(), EngineError> {
    runtime_launch_policy::write_chromium_local_state_locale_impl(profile_root, languages)
}

pub(super) fn chromium_host_resolver_rules(profile_root: &Path, max_len: usize) -> Option<String> {
    runtime_launch_policy::chromium_host_resolver_rules_impl(profile_root, max_len)
}

pub(super) fn chromium_extension_version(raw: &str) -> String {
    runtime_injection::chromium_extension_version_impl(raw)
}

pub(super) fn prepare_chromium_blocking_extension(
    profile_root: &Path,
) -> Result<Option<PathBuf>, EngineError> {
    runtime_injection::prepare_chromium_blocking_extension_impl(profile_root)
}

pub(super) fn load_locked_app_config(profile_root: &Path) -> Result<Option<LockedAppConfig>, EngineError> {
    runtime_injection::load_locked_app_config_impl(profile_root)
}

pub(super) fn resolve_locked_app_target_url(config: &LockedAppConfig, requested_url: &str) -> String {
    runtime_injection::resolve_locked_app_target_url_impl(config, requested_url)
}

pub(super) fn prepare_chromium_extension_dirs(profile_root: &Path) -> Result<Vec<PathBuf>, EngineError> {
    runtime_injection::prepare_chromium_extension_dirs_impl(profile_root)
}

pub(super) fn blocked_domains_for_profile(profile_root: &Path) -> Result<Vec<String>, EngineError> {
    runtime_injection::blocked_domains_for_profile_impl(profile_root)
}

pub(super) fn now_epoch_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub(super) fn ungoogled_chromium_releases_url() -> Result<&'static str, EngineError> {
    artifacts::ungoogled_chromium_releases_url_impl()
}

pub(super) fn ungoogled_chromium_asset_suffixes() -> Result<Vec<String>, EngineError> {
    artifacts::ungoogled_chromium_asset_suffixes_impl()
}

pub(super) fn ungoogled_chromium_asset_candidates(version: &str) -> Result<Vec<String>, EngineError> {
    artifacts::ungoogled_chromium_asset_candidates_impl(version)
}

pub(super) fn select_ungoogled_chromium_asset(
    release: &GithubRelease,
) -> Result<Option<GithubAsset>, EngineError> {
    artifacts::select_ungoogled_chromium_asset_impl(release)
}

#[cfg(unix)]
pub(super) fn ensure_engine_binary_executable(path: &Path) -> Result<(), EngineError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|error| {
        EngineError::Install(format!(
            "read runtime binary metadata failed for {}: {error}",
            path.display()
        ))
    })?;
    let mut permissions = metadata.permissions();
    let current_mode = permissions.mode();
    let desired_mode = current_mode | 0o111;
    if current_mode != desired_mode {
        permissions.set_mode(desired_mode);
        fs::set_permissions(path, permissions).map_err(|error| {
            EngineError::Install(format!(
                "mark runtime binary executable failed for {}: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn ensure_engine_helpers_executable(
    engine: EngineKind,
    binary_path: &Path,
) -> Result<(), EngineError> {
    if !matches!(engine, EngineKind::Chromium | EngineKind::UngoogledChromium) {
        return Ok(());
    }
    let Some(parent) = binary_path.parent() else {
        return Ok(());
    };
    for helper in ["chrome_crashpad_handler", "chrome-sandbox"] {
        let helper_path = parent.join(helper);
        if helper_path.exists() {
            ensure_engine_binary_executable(&helper_path)?;
        }
    }
    Ok(())
}


