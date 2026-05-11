use super::*;

pub(crate) fn set_library_item_profile_assignments_impl(
    state: &AppState,
    library_item_id: &str,
    assigned_profile_ids: &[String],
) -> Result<(), String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let Some(library_item) = library.items.get(library_item_id).cloned() else {
        return Err("extension not found".to_string());
    };
    let manager = state
        .manager
        .lock()
        .map_err(|_| "profile manager lock poisoned".to_string())?;
    let profiles = manager.list_profiles().map_err(|e| e.to_string())?;
    drop(manager);
    let mut store = state
        .profile_extension_store
        .lock()
        .map_err(|_| "profile extension store lock poisoned".to_string())?;
    let assigned = assigned_profile_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for profile in profiles {
        let profile_key = profile.id.to_string();
        let set = store
            .profiles
            .entry(profile_key.clone())
            .or_insert_with(|| ProfileExtensionSet {
                profile_id: profile_key.clone(),
                items: BTreeMap::new(),
            });
        if assigned.contains(&profile_key) {
            if !engine_scope_matches_profile(&library_item.engine_scope, profile.engine.clone()) {
                continue;
            }
            let mut existing_enabled = true;
            if let Some(existing) = set.items.get(library_item_id) {
                existing_enabled = existing.enabled;
            }
            let entry = materialize_profile_extension_impl(
                &state.profile_root,
                &profile,
                &library_item,
                existing_enabled,
                set.items
                    .get(library_item_id)
                    .and_then(|item| item.browser_extension_id.as_deref()),
            )?;
            set.items.insert(library_item_id.to_string(), entry);
        } else if let Some(removed) = set.items.remove(library_item_id) {
            cleanup_profile_extension_artifacts(&removed);
        }
    }
    sync_library_assignments_from_profile_store(&mut library, &store);
    persist_all(state, &store, &library)
}

pub(crate) fn remove_library_item_from_profiles_impl(
    state: &AppState,
    library_item_id: &str,
) -> Result<(), String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let mut store = state
        .profile_extension_store
        .lock()
        .map_err(|_| "profile extension store lock poisoned".to_string())?;
    for set in store.profiles.values_mut() {
        if let Some(removed) = set.items.remove(library_item_id) {
            cleanup_profile_extension_artifacts(&removed);
        }
    }
    sync_library_assignments_from_profile_store(&mut library, &store);
    persist_all(state, &store, &library)
}

pub(crate) fn prepare_profile_extensions_for_launch_impl(
    state: &AppState,
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Result<(), String> {
    eprintln!(
        "[profile-extensions] prepare start profile={} engine={:?}",
        profile.id, profile.engine
    );
    if profile.engine.is_chromium_family() {
        sync_profile_extensions_from_browser(state, profile)?;
    }
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let mut store = state
        .profile_extension_store
        .lock()
        .map_err(|_| "profile extension store lock poisoned".to_string())?;
    let set = store
        .profiles
        .entry(profile.id.to_string())
        .or_insert_with(|| ProfileExtensionSet {
            profile_id: profile.id.to_string(),
            items: BTreeMap::new(),
        });
    let _ = hydrate_profile_extensions_from_profile_storage_impl(state, profile, &library, set)?;
    let items = set.items.clone();
    let mut external_manifests_keep = BTreeSet::new();
    for (library_item_id, current) in items {
        let Some(library_item) = library.items.get(&library_item_id).cloned() else {
            continue;
        };
        let next = materialize_profile_extension_impl(
            &state.profile_root,
            profile,
            &library_item,
            current.enabled,
            current.browser_extension_id.as_deref(),
        )?;
        let next = preserve_browser_extension_binding_impl(current, next);
        if profile.engine.is_chromium_family() {
            if library_item_id.eq_ignore_ascii_case("dark-reader")
                && next.enabled
                && next.browser_extension_id.is_some()
                && next.profile_package_path.is_some()
            {
                register_chromium_external_manifest_for_item(profile_root, &next)?;
                if let Some(browser_id) = next.browser_extension_id.as_ref() {
                    external_manifests_keep.insert(format!("{browser_id}.json"));
                }
            }
        }
        set.items.insert(library_item_id, next);
    }
    if profile.engine.is_chromium_family() {
        cleanup_chromium_external_extension_manifests(profile_root, &external_manifests_keep)?;
        sanitize_chromium_runtime_extension_state(profile_root)?;
        cleanup_legacy_chromium_extension_root(profile_root)?;
    }
    if matches!(profile.engine, Engine::Librewolf | Engine::FirefoxEsr) {
        sync_firefox_engine_profile_extensions(set, profile_root)?;
    }
    eprintln!(
        "[profile-extensions] prepare complete profile={} managed_items={}",
        profile.id,
        set.items.len()
    );
    sync_library_assignments_from_profile_store(&mut library, &store);
    persist_all(state, &store, &library)
}

pub(crate) fn collect_active_profile_extensions_impl(
    state: &AppState,
    profile: &ProfileMetadata,
) -> Result<Vec<ProfileInstalledExtension>, String> {
    let store = state
        .profile_extension_store
        .lock()
        .map_err(|_| "profile extension store lock poisoned".to_string())?;
    Ok(store
        .profiles
        .get(&profile.id.to_string())
        .map(|set| {
            set.items
                .values()
                .filter(|item| item.enabled)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default())
}

pub(crate) fn apply_profile_extension_selections_impl(
    state: &AppState,
    store: &mut ProfileExtensionStore,
    library: &mut ExtensionLibraryStore,
    profile: &ProfileMetadata,
    selections: Vec<ProfileExtensionSelection>,
) -> Result<(), String> {
    apply_profile_extension_selections_with_root_impl(
        &state.profile_root,
        store,
        library,
        profile,
        selections,
    )
}

pub(crate) fn apply_profile_extension_selections_with_root_impl(
    profile_root_base: &Path,
    store: &mut ProfileExtensionStore,
    library: &mut ExtensionLibraryStore,
    profile: &ProfileMetadata,
    selections: Vec<ProfileExtensionSelection>,
) -> Result<(), String> {
    let profile_key = profile.id.to_string();
    let set = store
        .profiles
        .entry(profile_key.clone())
        .or_insert_with(|| ProfileExtensionSet {
            profile_id: profile_key,
            items: BTreeMap::new(),
        });
    let desired = selections
        .into_iter()
        .map(|item| (item.library_item_id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let current_ids = set.items.keys().cloned().collect::<Vec<_>>();
    for library_item_id in current_ids {
        if !desired.contains_key(&library_item_id) {
            if let Some(removed) = set.items.remove(&library_item_id) {
                cleanup_profile_extension_artifacts(&removed);
            }
        }
    }
    for selection in desired.into_values() {
        let library_item = library
            .items
            .get(&selection.library_item_id)
            .cloned()
            .ok_or_else(|| format!("extension `{}` not found", selection.library_item_id))?;
        if !engine_scope_matches_profile(&library_item.engine_scope, profile.engine.clone()) {
            return Err(format!(
                "extension engine scope `{}` is incompatible with profile `{}`",
                library_item.engine_scope, profile.name
            ));
        }
        let entry = materialize_profile_extension_impl(
            profile_root_base,
            profile,
            &library_item,
            selection.enabled,
            None,
        )?;
        set.items.insert(selection.library_item_id, entry);
    }
    sync_library_assignments_from_profile_store(library, store);
    Ok(())
}

pub(crate) fn materialize_profile_extension_impl(
    profile_root_base: &Path,
    profile: &ProfileMetadata,
    library_item: &ExtensionLibraryItem,
    enabled: bool,
    existing_browser_extension_id: Option<&str>,
) -> Result<ProfileInstalledExtension, String> {
    let Some(variant) = resolve_variant_for_engine(library_item, profile.engine.clone()) else {
        return Err("extension package for profile engine is missing".to_string());
    };
    let source_package_path = variant
        .package_path
        .clone()
        .ok_or_else(|| "extension package path is missing".to_string())?;
    let profile_root = profile_root_base.join(profile.id.to_string());
    let managed_root = profile_root.join("extensions").join("managed");
    let packages_root = managed_root.join("packages");
    fs::create_dir_all(&packages_root).map_err(|e| format!("create profile package root: {e}"))?;
    let extension = package_extension(&source_package_path, variant.package_file_name.as_deref());
    let profile_package_path = packages_root.join(format!("{}.{}", library_item.id, extension));
    fs::copy(&source_package_path, &profile_package_path)
        .map_err(|e| format!("copy profile extension package: {e}"))?;
    let manifest = read_extension_manifest(&profile_package_path)?;
    let version = manifest
        .get("version")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| library_item.version.clone());
    let (browser_extension_id, profile_unpacked_path) = if profile.engine.is_chromium_family() {
        let unpacked_root = managed_root.join("chromium-unpacked");
        let stable_dir = unpacked_root.join(&library_item.id);
        sync_unpacked_chromium_extension(&profile_package_path, &stable_dir)?;
        if library_item.id.eq_ignore_ascii_case("dark-reader") {
            suppress_dark_reader_install_tab(&stable_dir)?;
        }
        let browser_extension_id = chromium_extension_id_from_manifest(&manifest)
            .or_else(|| existing_browser_extension_id.map(str::to_string));
        (
            browser_extension_id,
            Some(stable_dir.to_string_lossy().to_string()),
        )
    } else {
        (
            firefox_extension_id_from_manifest(&manifest).or_else(|| {
                library_item
                    .id
                    .contains('@')
                    .then(|| library_item.id.clone())
            }),
            None,
        )
    };
    Ok(ProfileInstalledExtension {
        library_item_id: library_item.id.clone(),
        display_name: library_item.display_name.clone(),
        engine_scope: variant.engine_scope.clone(),
        version,
        enabled,
        browser_extension_id,
        source_package_path: Some(source_package_path),
        profile_package_path: Some(profile_package_path.to_string_lossy().to_string()),
        profile_unpacked_path,
        package_file_name: variant.package_file_name.clone(),
    })
}

pub(crate) fn preserve_browser_extension_binding_impl(
    previous: ProfileInstalledExtension,
    mut next: ProfileInstalledExtension,
) -> ProfileInstalledExtension {
    if next.browser_extension_id.is_none() {
        next.browser_extension_id = previous.browser_extension_id;
    }
    next
}

pub(crate) fn hydrate_profile_extensions_from_profile_storage_impl(
    state: &AppState,
    profile: &ProfileMetadata,
    library: &ExtensionLibraryStore,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    let mut changed = false;
    for library_item_id in discover_profile_extension_candidate_ids_impl(state, profile)? {
        if set.items.contains_key(&library_item_id) {
            continue;
        }
        let Some(library_item) = library.items.get(&library_item_id).cloned() else {
            continue;
        };
        if !engine_scope_matches_profile(&library_item.engine_scope, profile.engine.clone()) {
            continue;
        }
        let entry = materialize_profile_extension_impl(
            &state.profile_root,
            profile,
            &library_item,
            true,
            None,
        )?;
        set.items.insert(library_item_id, entry);
        changed = true;
    }
    Ok(changed)
}

pub(crate) fn discover_profile_extension_candidate_ids_impl(
    state: &AppState,
    profile: &ProfileMetadata,
) -> Result<Vec<String>, String> {
    let profile_root = state.profile_root.join(profile.id.to_string());
    let mut ids = BTreeSet::new();
    if profile.engine.is_chromium_family() {
        collect_extension_dir_names(
            &profile_root
                .join("extensions")
                .join("managed")
                .join("chromium-unpacked"),
            &mut ids,
        )?;
        collect_profile_package_stems(
            &profile_root
                .join("extensions")
                .join("managed")
                .join("packages"),
            &mut ids,
        )?;
    } else {
        collect_profile_package_stems(
            &profile_root
                .join("extensions")
                .join("managed")
                .join("packages"),
            &mut ids,
        )?;
    }
    Ok(ids.into_iter().collect())
}
