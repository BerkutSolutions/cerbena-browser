use super::*;

pub(crate) fn apply_launch_policies_impl(
    state: &State<'_, AppState>,
    profile: &ProfileMetadata,
    profile_root: &Path,
    engine: EngineKind,
    installation_binary_path: Option<&Path>,
) -> Result<(), String> {
    super::write_profile_blocked_domains_impl(state, &profile.id, profile_root)
        .map_err(|e| e.to_string())?;
    super::write_locked_app_policy(profile, profile_root).map_err(|e| e.to_string())?;
    if let Some(binary_path) = installation_binary_path {
        if matches!(engine, EngineKind::Librewolf | EngineKind::FirefoxEsr) {
            super::apply_librewolf_website_filter_impl(state, &profile.id, binary_path)
                .map_err(|e| e.to_string())?;
        }
        if matches!(engine, EngineKind::Librewolf) {
            super::neutralize_librewolf_builtin_theme_impl(binary_path)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
