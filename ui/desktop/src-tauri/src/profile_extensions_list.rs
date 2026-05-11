use super::*;

pub(crate) fn list_profile_extensions_json_impl(
    state: &AppState,
    profile_id: &str,
) -> Result<String, String> {
    let profile = {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "profile manager lock poisoned".to_string())?;
        let profile_id =
            uuid::Uuid::parse_str(profile_id).map_err(|e| format!("profile id: {e}"))?;
        manager.get_profile(profile_id).map_err(|e| e.to_string())?
    };
    super::sync_profile_extensions_from_browser(state, &profile)?;
    let payload = {
        let store = state
            .profile_extension_store
            .lock()
            .map_err(|_| "profile extension store lock poisoned".to_string())?;
        let set = store
            .profiles
            .get(&profile.id.to_string())
            .cloned()
            .unwrap_or_default();
        ProfileExtensionListPayload {
            profile_id: profile.id.to_string(),
            extensions: set
                .items
                .values()
                .map(|item| ProfileExtensionView {
                    library_item_id: item.library_item_id.clone(),
                    display_name: item.display_name.clone(),
                    engine_scope: item.engine_scope.clone(),
                    version: item.version.clone(),
                    enabled: item.enabled,
                    browser_extension_id: item.browser_extension_id.clone(),
                })
                .collect(),
        }
    };
    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}

pub(crate) fn migrate_legacy_profile_extensions_impl(
    profile_root_base: &Path,
    profiles: &[ProfileMetadata],
    store: &mut ProfileExtensionStore,
    library: &mut ExtensionLibraryStore,
) -> Result<bool, String> {
    let mut changed = false;
    for profile in profiles {
        let profile_key = profile.id.to_string();
        let profile_root = profile_root_base.join(&profile_key);
        let legacy_candidate_ids =
            discover_legacy_profile_extension_candidate_ids_impl(&profile_root, profile)?;
        let set = store
            .profiles
            .entry(profile_key.clone())
            .or_insert_with(|| ProfileExtensionSet {
                profile_id: profile_key.clone(),
                items: BTreeMap::new(),
            });
        if !set.items.is_empty() {
            if profile.engine.is_chromium_family() {
                cleanup_legacy_chromium_extension_root(&profile_root)?;
            }
            continue;
        }
        let mut selected = BTreeSet::new();
        let mut disabled = BTreeSet::new();
        for tag in &profile.tags {
            if let Some(value) = tag.strip_prefix("ext:") {
                selected.insert(value.to_string());
            }
            if let Some(value) = tag.strip_prefix("ext-disabled:") {
                selected.insert(value.to_string());
                disabled.insert(value.to_string());
            }
        }
        for item in library.items.values() {
            if item
                .assigned_profile_ids
                .iter()
                .any(|value| value == &profile_key)
            {
                selected.insert(item.id.clone());
            }
        }
        selected.extend(legacy_candidate_ids);
        if selected.is_empty() {
            if profile.engine.is_chromium_family() {
                cleanup_legacy_chromium_extension_root(&profile_root)?;
            }
            continue;
        }
        let selections = selected
            .into_iter()
            .filter(|library_item_id| library.items.contains_key(library_item_id))
            .map(|library_item_id| ProfileExtensionSelection {
                enabled: !disabled.contains(&library_item_id),
                library_item_id,
            })
            .collect::<Vec<_>>();
        if selections.is_empty() {
            if profile.engine.is_chromium_family() {
                cleanup_legacy_chromium_extension_root(&profile_root)?;
            }
            continue;
        }
        super::apply_profile_extension_selections_with_root(
            profile_root_base,
            store,
            library,
            profile,
            selections,
        )?;
        if profile.engine.is_chromium_family() {
            cleanup_legacy_chromium_extension_root(&profile_root)?;
        }
        changed = true;
    }
    if changed {
        sync_library_assignments_from_profile_store(library, store);
    }
    Ok(changed)
}

pub(crate) fn discover_legacy_profile_extension_candidate_ids_impl(
    profile_root: &Path,
    profile: &ProfileMetadata,
) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    if profile.engine.is_chromium_family() {
        collect_extension_dir_names(&profile_root.join("policy").join("chromium-extensions"), &mut ids)?;
    }
    collect_profile_package_stems(
        &profile_root
            .join("extensions")
            .join("managed")
            .join("packages"),
        &mut ids,
    )?;
    Ok(ids)
}
