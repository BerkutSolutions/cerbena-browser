use super::*;

pub(crate) fn build_chromium_launch_plan_impl(
    explicit_launch_url: Option<String>,
    profile_default_start_page: Option<&str>,
    has_restore_session: bool,
) -> LaunchPlan {
    let profile_start_page = super::normalize_optional_start_page_url_impl(profile_default_start_page);
    let start_page = explicit_launch_url
        .clone()
        .or_else(|| (!has_restore_session).then_some(profile_start_page).flatten());
    LaunchPlan {
        start_page,
        post_launch_url: None,
    }
}
