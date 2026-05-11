use super::*;
#[path = "profile_commands_launch_plan_chromium.rs"]
mod chromium;
#[path = "profile_commands_launch_plan_firefox.rs"]
mod firefox;

#[derive(Debug, Clone)]
pub(crate) struct LaunchPlan {
    pub(crate) start_page: Option<String>,
    pub(crate) post_launch_url: Option<String>,
}

pub(crate) fn build_launch_plan_impl(
    engine: EngineKind,
    explicit_launch_url: Option<&str>,
    profile_default_start_page: Option<&str>,
    has_restore_session: bool,
) -> LaunchPlan {
    let explicit_launch_url = explicit_launch_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if matches!(engine, EngineKind::Librewolf | EngineKind::FirefoxEsr) {
        firefox::build_firefox_family_launch_plan_impl(explicit_launch_url, has_restore_session)
    } else {
        chromium::build_chromium_launch_plan_impl(
            explicit_launch_url,
            profile_default_start_page,
            has_restore_session,
        )
    }
}
