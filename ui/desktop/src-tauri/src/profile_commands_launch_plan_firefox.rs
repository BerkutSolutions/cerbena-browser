use super::*;

pub(crate) fn build_firefox_family_launch_plan_impl(
    explicit_launch_url: Option<String>,
    has_restore_session: bool,
) -> LaunchPlan {
    let post_launch_url = has_restore_session
        .then_some(explicit_launch_url.clone())
        .flatten();
    LaunchPlan {
        start_page: explicit_launch_url,
        post_launch_url,
    }
}
