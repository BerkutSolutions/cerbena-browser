use super::{
    launch_args, select_ungoogled_chromium_asset, EngineError, EngineKind, EngineRuntime,
    GithubAsset, GithubRelease, CHROMIUM_POLICY_EXTENSION_VERSION,
};

pub(super) fn chromium_extension_version(raw: &str) -> String {
    super::runtime_injection::chromium_extension_version_impl(raw)
}

pub(super) fn prepare_chromium_blocking_extension(profile_root: &std::path::Path) -> Result<Option<std::path::PathBuf>, EngineError> {
    super::runtime_injection::prepare_chromium_blocking_extension_impl(profile_root)
}

pub(super) fn blocked_domains_for_profile(profile_root: &std::path::Path) -> Result<Vec<String>, EngineError> {
    super::runtime_injection::blocked_domains_for_profile_impl(profile_root)
}

pub(super) fn build_accept_language_header(languages: &[String]) -> String {
    super::runtime_launch_policy::build_accept_language_header_impl(languages)
}

pub(super) fn chromium_launch_environment(profile_root: &std::path::Path) -> Vec<(String, String)> {
    super::runtime_launch_policy::chromium_launch_environment_impl(profile_root)
}

pub(super) fn extract_librewolf_download_url(html: &str, asset_marker: &str) -> Option<String> {
    super::runtime_compat::extract_librewolf_download_url_impl(html, asset_marker)
}

pub(super) fn parse_librewolf_version_from_file_name(file_name: &str) -> Option<String> {
    super::runtime_compat::parse_librewolf_version_from_file_name_impl(file_name)
}

pub(super) fn prefer_chromium_vendor_binary(current: &std::path::Path) -> std::path::PathBuf {
    super::runtime_binary::prefer_chromium_vendor_binary_impl(current)
}


#[path = "runtime_tests_cases.rs"]
mod runtime_tests_cases;
