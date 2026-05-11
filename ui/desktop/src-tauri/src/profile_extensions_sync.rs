use super::*;

pub(crate) fn sync_profile_extensions_from_browser_impl(
    state: &AppState,
    profile: &ProfileMetadata,
) -> Result<(), String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let mut store = state
        .profile_extension_store
        .lock()
        .map_err(|_| "profile extension store lock poisoned".to_string())?;
    let profile_key = profile.id.to_string();
    let set = store
        .profiles
        .entry(profile_key.clone())
        .or_insert_with(|| ProfileExtensionSet {
            profile_id: profile_key,
            items: BTreeMap::new(),
        });
    let hydrated = super::hydrate_profile_extensions_from_profile_storage(state, profile, &library, set)?;
    let changed = match profile.engine {
        Engine::Chromium | Engine::UngoogledChromium => {
            super::sync_chromium_store_from_browser(state, profile, set)?
        }
        Engine::FirefoxEsr | Engine::Librewolf => {
            super::sync_firefox_store_from_browser(state, profile, set)?
        }
    };
    if hydrated || changed {
        super::sync_library_assignments_from_profile_store(&mut library, &store);
        super::persist_all(state, &store, &library)?;
    }
    Ok(())
}
