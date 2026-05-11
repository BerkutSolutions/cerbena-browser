use super::*;

pub(crate) fn load_profile_extension_store_impl(
    profile_root_base: &Path,
    profiles: &[ProfileMetadata],
) -> Result<ProfileExtensionStore, String> {
    let mut store = ProfileExtensionStore::default();
    for profile in profiles {
        let path = profile_extension_store_file_impl(profile_root_base, &profile.id.to_string());
        if !path.exists() {
            continue;
        }
        let bytes = fs::read(&path)
            .map_err(|e| format!("read profile extension store {}: {e}", path.display()))?;
        let mut set: ProfileExtensionSet = serde_json::from_slice(&bytes)
            .map_err(|e| format!("parse profile extension store {}: {e}", path.display()))?;
        if set.profile_id.trim().is_empty() {
            set.profile_id = profile.id.to_string();
        }
        store.profiles.insert(profile.id.to_string(), set);
    }
    Ok(store)
}

pub(crate) fn persist_profile_extension_store_impl(
    profile_root_base: &Path,
    store: &ProfileExtensionStore,
) -> Result<(), String> {
    for (profile_id, set) in &store.profiles {
        let path = profile_extension_store_file_impl(profile_root_base, profile_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "create profile extension store parent {}: {e}",
                    parent.display()
                )
            })?;
        }
        let bytes = serde_json::to_vec_pretty(set).map_err(|e| {
            format!(
                "serialize profile extension store for profile {}: {e}",
                profile_id
            )
        })?;
        fs::write(&path, bytes)
            .map_err(|e| format!("write profile extension store {}: {e}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn sync_library_assignments_from_profile_store_impl(
    library: &mut ExtensionLibraryStore,
    store: &ProfileExtensionStore,
) {
    let mut assigned: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (profile_id, profile_set) in &store.profiles {
        for library_item_id in profile_set.items.keys() {
            assigned
                .entry(library_item_id.clone())
                .or_default()
                .insert(profile_id.clone());
        }
    }
    for (item_id, item) in &mut library.items {
        item.assigned_profile_ids = assigned
            .remove(item_id)
            .unwrap_or_default()
            .into_iter()
            .collect();
    }
}

pub(crate) fn profile_extension_store_file_impl(
    profile_root_base: &Path,
    profile_id: &str,
) -> PathBuf {
    profile_root_base
        .join(profile_id)
        .join("extensions")
        .join("managed")
        .join("store.json")
}

pub(crate) fn persist_all_impl(
    state: &AppState,
    store: &ProfileExtensionStore,
    library: &ExtensionLibraryStore,
) -> Result<(), String> {
    persist_profile_extension_store_impl(&state.profile_root, store)?;
    let library_path = state.extension_library_path(&state.app_handle)?;
    crate::state::persist_extension_library_store(&library_path, library)
}
