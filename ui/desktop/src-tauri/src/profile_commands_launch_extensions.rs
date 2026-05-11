use super::*;

pub(crate) fn prepare_extensions_for_launch_impl(
    state: &State<'_, AppState>,
    profile: &ProfileMetadata,
    profile_root: &Path,
    engine: EngineKind,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    if engine.is_chromium_family() {
        super::emit_profile_launch_progress(
            app_handle,
            profile.id,
            "extensions",
            "profile.launchProgress.extensions",
            false,
            None,
        );
        super::prepare_profile_chromium_extensions_impl(state, profile, profile_root)?;
        super::emit_profile_launch_progress(
            app_handle,
            profile.id,
            "keepassxc",
            "profile.launchProgress.keepassxc",
            false,
            None,
        );
        ensure_keepassxc_bridge_for_profile(state.inner(), profile, profile_root)?;
        return Ok(());
    }
    if matches!(engine, EngineKind::Librewolf) {
        profile_extensions::prepare_profile_extensions_for_launch(state.inner(), profile, profile_root)?;
        super::emit_profile_launch_progress(
            app_handle,
            profile.id,
            "keepassxc",
            "profile.launchProgress.keepassxc",
            false,
            None,
        );
        ensure_keepassxc_bridge_for_profile(state.inner(), profile, profile_root)?;
    }
    Ok(())
}
