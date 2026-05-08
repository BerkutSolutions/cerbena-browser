use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use browser_profile::{Engine, ProfileMetadata};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::state::{
    AppState, ExtensionLibraryItem, ExtensionLibraryStore, ExtensionPackageVariant,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExtensionStore {
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileExtensionSet>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExtensionSet {
    pub profile_id: String,
    #[serde(default)]
    pub items: BTreeMap<String, ProfileInstalledExtension>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInstalledExtension {
    pub library_item_id: String,
    pub display_name: String,
    pub engine_scope: String,
    pub version: String,
    pub enabled: bool,
    #[serde(default)]
    pub browser_extension_id: Option<String>,
    #[serde(default)]
    pub source_package_path: Option<String>,
    #[serde(default)]
    pub profile_package_path: Option<String>,
    #[serde(default)]
    pub profile_unpacked_path: Option<String>,
    #[serde(default)]
    pub package_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExtensionSelection {
    pub library_item_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExtensionView {
    pub library_item_id: String,
    pub display_name: String,
    pub engine_scope: String,
    pub version: String,
    pub enabled: bool,
    pub browser_extension_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExtensionListPayload {
    pub profile_id: String,
    pub extensions: Vec<ProfileExtensionView>,
}

pub fn load_profile_extension_store(
    profile_root_base: &Path,
    profiles: &[ProfileMetadata],
) -> Result<ProfileExtensionStore, String> {
    let mut store = ProfileExtensionStore::default();
    for profile in profiles {
        let path = profile_extension_store_file(profile_root_base, &profile.id.to_string());
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

pub fn persist_profile_extension_store(
    profile_root_base: &Path,
    store: &ProfileExtensionStore,
) -> Result<(), String> {
    for (profile_id, set) in &store.profiles {
        let path = profile_extension_store_file(profile_root_base, profile_id);
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

pub fn sync_library_assignments_from_profile_store(
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

pub fn migrate_legacy_profile_extensions(
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
            discover_legacy_profile_extension_candidate_ids(&profile_root, profile)?;
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
        apply_profile_extension_selections_with_root(
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

pub fn list_profile_extensions_json(state: &AppState, profile_id: &str) -> Result<String, String> {
    let profile = {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "profile manager lock poisoned".to_string())?;
        let profile_id =
            uuid::Uuid::parse_str(profile_id).map_err(|e| format!("profile id: {e}"))?;
        manager.get_profile(profile_id).map_err(|e| e.to_string())?
    };
    sync_profile_extensions_from_browser(state, &profile)?;
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

pub fn save_profile_extensions(
    state: &AppState,
    profile_id: &str,
    selections: Vec<ProfileExtensionSelection>,
) -> Result<(), String> {
    let profile_uuid = uuid::Uuid::parse_str(profile_id).map_err(|e| format!("profile id: {e}"))?;
    let profile = {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "profile manager lock poisoned".to_string())?;
        manager
            .get_profile(profile_uuid)
            .map_err(|e| e.to_string())?
    };
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let mut store = state
        .profile_extension_store
        .lock()
        .map_err(|_| "profile extension store lock poisoned".to_string())?;
    apply_profile_extension_selections(state, &mut store, &mut library, &profile, selections)?;
    persist_all(state, &store, &library)
}

pub fn set_library_item_profile_assignments(
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
            let entry = materialize_profile_extension(
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

pub fn remove_library_item_from_profiles(
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

pub fn prepare_profile_extensions_for_launch(
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
    let _ = hydrate_profile_extensions_from_profile_storage(state, profile, &library, set)?;
    let items = set.items.clone();
    let mut external_manifests_keep = BTreeSet::new();
    for (library_item_id, current) in items {
        let Some(library_item) = library.items.get(&library_item_id).cloned() else {
            continue;
        };
        let next = materialize_profile_extension(
            &state.profile_root,
            profile,
            &library_item,
            current.enabled,
            current.browser_extension_id.as_deref(),
        )?;
        let next = preserve_browser_extension_binding(current, next);
        if profile.engine.is_chromium_family() {
            eprintln!(
                "[profile-extensions] chromium launch materialized profile={} library_item={} browser_extension_id={:?} unpacked_path={:?}",
                profile.id,
                library_item_id,
                next.browser_extension_id,
                next.profile_unpacked_path
            );
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
    if matches!(profile.engine, Engine::Librewolf) {
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

pub fn collect_active_profile_extensions(
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

pub fn sync_profile_extensions_from_browser(
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
    let hydrated = hydrate_profile_extensions_from_profile_storage(state, profile, &library, set)?;
    let changed = match profile.engine {
        Engine::Chromium | Engine::UngoogledChromium => {
            sync_chromium_store_from_browser(state, profile, set)?
        }
        Engine::Librewolf => sync_firefox_store_from_browser(state, profile, set)?,
    };
    if hydrated || changed {
        sync_library_assignments_from_profile_store(&mut library, &store);
        persist_all(state, &store, &library)?;
    }
    Ok(())
}

fn apply_profile_extension_selections(
    state: &AppState,
    store: &mut ProfileExtensionStore,
    library: &mut ExtensionLibraryStore,
    profile: &ProfileMetadata,
    selections: Vec<ProfileExtensionSelection>,
) -> Result<(), String> {
    apply_profile_extension_selections_with_root(
        &state.profile_root,
        store,
        library,
        profile,
        selections,
    )
}

fn apply_profile_extension_selections_with_root(
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
        let entry = materialize_profile_extension(
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

fn materialize_profile_extension(
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
        if browser_extension_id.is_none() {
            eprintln!(
                "[profile-extensions] chromium package missing manifest.key profile={} library_item={} unpacked_path={}",
                profile.id,
                library_item.id,
                stable_dir.display()
            );
        }
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

fn preserve_browser_extension_binding(
    previous: ProfileInstalledExtension,
    mut next: ProfileInstalledExtension,
) -> ProfileInstalledExtension {
    if next.browser_extension_id.is_none() {
        next.browser_extension_id = previous.browser_extension_id;
    }
    next
}

fn hydrate_profile_extensions_from_profile_storage(
    state: &AppState,
    profile: &ProfileMetadata,
    library: &ExtensionLibraryStore,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    let mut changed = false;
    for library_item_id in discover_profile_extension_candidate_ids(state, profile)? {
        if set.items.contains_key(&library_item_id) {
            continue;
        }
        let Some(library_item) = library.items.get(&library_item_id).cloned() else {
            continue;
        };
        if !engine_scope_matches_profile(&library_item.engine_scope, profile.engine.clone()) {
            continue;
        }
        let entry =
            materialize_profile_extension(&state.profile_root, profile, &library_item, true, None)?;
        set.items.insert(library_item_id, entry);
        changed = true;
    }
    Ok(changed)
}

fn discover_profile_extension_candidate_ids(
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

fn discover_legacy_profile_extension_candidate_ids(
    profile_root: &Path,
    profile: &ProfileMetadata,
) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    if profile.engine.is_chromium_family() {
        collect_extension_dir_names(
            &profile_root.join("policy").join("chromium-extensions"),
            &mut ids,
        )?;
    }
    Ok(ids)
}

fn collect_extension_dir_names(root: &Path, ids: &mut BTreeSet<String>) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)
        .map_err(|e| format!("read profile extension dir {}: {e}", root.display()))?
    {
        let entry = entry.map_err(|e| format!("read profile extension entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        ids.insert(name.to_string());
    }
    Ok(())
}

fn suppress_dark_reader_install_tab(unpacked_dir: &Path) -> Result<(), String> {
    let background_path = unpacked_dir.join("background").join("index.js");
    if !background_path.is_file() {
        return Ok(());
    }
    let source = fs::read_to_string(&background_path)
        .map_err(|e| format!("read dark-reader background script: {e}"))?;
    if source.contains("cerbena_dark_reader_install_tab_suppressed") {
        let marker = unpacked_dir.join(".cerbena-prefer-external-manifest");
        let _ = fs::write(&marker, b"1");
        return Ok(());
    }
    let mut updated = source.clone();
    if let Some(help_call_pos) = source.find("chrome.tabs.create({url: getHelpURL()});") {
        let mut start = help_call_pos;
        while start > 0 && source.as_bytes()[start - 1].is_ascii_whitespace() {
            start -= 1;
        }
        let indent = &source[start..help_call_pos];
        let replacement = format!(
            "{}/* cerbena_dark_reader_install_tab_suppressed */",
            indent
        );
        updated.replace_range(
            help_call_pos..(help_call_pos + "chrome.tabs.create({url: getHelpURL()});".len()),
            &replacement,
        );
    } else if let Some(help_call_pos) = source.find("chrome.tabs.create({url:getHelpURL()})") {
        let replacement = "/* cerbena_dark_reader_install_tab_suppressed */";
        updated.replace_range(
            help_call_pos..(help_call_pos + "chrome.tabs.create({url:getHelpURL()})".len()),
            replacement,
        );
    }
    if updated == source {
        eprintln!(
            "[profile-extensions] dark-reader install tab patch skipped path={} reason=pattern_not_found",
            background_path.display()
        );
        return Ok(());
    }
    fs::write(&background_path, updated)
        .map_err(|e| format!("write patched dark-reader background script: {e}"))?;
    let marker = unpacked_dir.join(".cerbena-prefer-external-manifest");
    fs::write(&marker, b"1").map_err(|e| {
        format!(
            "write dark-reader external-manifest marker {}: {e}",
            marker.display()
        )
    })?;
    eprintln!(
        "[profile-extensions] dark-reader install tab suppressed path={}",
        background_path.display()
    );
    Ok(())
}

fn collect_profile_package_stems(root: &Path, ids: &mut BTreeSet<String>) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)
        .map_err(|e| format!("read profile package dir {}: {e}", root.display()))?
    {
        let entry = entry.map_err(|e| format!("read profile package entry: {e}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        ids.insert(stem.to_string());
    }
    Ok(())
}

fn resolve_variant_for_engine(
    item: &ExtensionLibraryItem,
    engine: Engine,
) -> Option<ExtensionPackageVariant> {
    let mut variants = item.package_variants.clone();
    if variants.is_empty() {
        variants.push(ExtensionPackageVariant {
            engine_scope: item.engine_scope.clone(),
            version: item.version.clone(),
            source_kind: item.source_kind.clone(),
            source_value: item.source_value.clone(),
            logo_url: item.logo_url.clone(),
            store_url: item.store_url.clone(),
            package_path: item.package_path.clone(),
            package_file_name: item.package_file_name.clone(),
        });
    }
    variants
        .into_iter()
        .find(|variant| engine_scope_matches_profile(&variant.engine_scope, engine.clone()))
}

fn engine_scope_matches_profile(engine_scope: &str, engine: Engine) -> bool {
    match normalize_engine_scope(engine_scope).as_str() {
        "firefox" => matches!(engine, Engine::Librewolf),
        "chromium" => engine.is_chromium_family(),
        "chromium/firefox" => true,
        _ => true,
    }
}

fn normalize_engine_scope(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "librewolf" => "firefox".to_string(),
        "ungoogled-chromium" => "chromium".to_string(),
        "" => "chromium/firefox".to_string(),
        other => other.to_string(),
    }
}

fn package_extension(package_path: &str, package_file_name: Option<&str>) -> String {
    Path::new(package_file_name.unwrap_or(package_path))
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "zip".to_string())
}

fn read_extension_manifest(path: &Path) -> Result<serde_json::Value, String> {
    let bytes =
        fs::read(path).map_err(|e| format!("read extension package {}: {e}", path.display()))?;
    let archive_bytes = if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("crx"))
        .unwrap_or(false)
    {
        extract_crx_zip_bytes(&bytes)?
    } else {
        bytes
    };
    let cursor = Cursor::new(archive_bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("open extension archive: {e}"))?;
    let manifest = read_manifest_from_archive(&mut archive)?;
    serde_json::from_str(&manifest).map_err(|e| format!("parse extension manifest: {e}"))
}

fn read_manifest_from_archive<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<String, String> {
    if let Ok(mut manifest) = archive.by_name("manifest.json") {
        let mut text = String::new();
        manifest
            .read_to_string(&mut text)
            .map_err(|e| format!("read manifest.json: {e}"))?;
        return Ok(text);
    }
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("read archive entry: {e}"))?;
        let name = entry.name().replace('\\', "/");
        if name.ends_with("/manifest.json") {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|e| format!("read nested manifest.json: {e}"))?;
            return Ok(text);
        }
    }
    Err("extension package manifest.json not found".to_string())
}

fn extract_crx_zip_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let signature = b"PK\x03\x04";
    let Some(offset) = bytes
        .windows(signature.len())
        .position(|window| window == signature)
    else {
        return Err("embedded zip payload not found in CRX package".to_string());
    };
    Ok(bytes[offset..].to_vec())
}

fn chromium_extension_id_from_manifest(manifest: &serde_json::Value) -> Option<String> {
    let key = manifest.get("key")?.as_str()?.trim();
    if key.is_empty() {
        return None;
    }
    let der = BASE64_STANDARD.decode(key.as_bytes()).ok()?;
    let hash = Sha256::digest(&der);
    let alphabet = b"abcdefghijklmnop";
    let mut id = String::with_capacity(32);
    for byte in hash.iter().take(16) {
        id.push(alphabet[((byte >> 4) & 0x0f) as usize] as char);
        id.push(alphabet[(byte & 0x0f) as usize] as char);
    }
    Some(id)
}

fn firefox_extension_id_from_manifest(manifest: &serde_json::Value) -> Option<String> {
    manifest
        .get("browser_specific_settings")
        .and_then(|value| value.get("gecko"))
        .and_then(|value| value.get("id"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| {
            manifest
                .get("applications")
                .and_then(|value| value.get("gecko"))
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
}

fn sync_unpacked_chromium_extension(package_path: &Path, destination: &Path) -> Result<(), String> {
    let package_hash = format!(
        "{:x}",
        Sha256::digest(
            &fs::read(package_path).map_err(|e| format!("read extension package: {e}"))?
        )
    );
    let marker_path = destination.join(".cerbena-package-sha256");
    if destination.is_dir()
        && destination.join("manifest.json").is_file()
        && fs::read_to_string(&marker_path).unwrap_or_default().trim() == package_hash
    {
        return Ok(());
    }
    if destination.exists() {
        let _ = fs::remove_dir_all(destination);
    }
    fs::create_dir_all(destination).map_err(|e| format!("create chromium extension dir: {e}"))?;
    unpack_extension_archive(package_path, destination)?;
    fs::write(marker_path, package_hash.as_bytes())
        .map_err(|e| format!("write chromium extension marker: {e}"))?;
    Ok(())
}

fn unpack_extension_archive(package_path: &Path, destination: &Path) -> Result<(), String> {
    let bytes = fs::read(package_path).map_err(|e| format!("read extension package: {e}"))?;
    let archive_bytes = if package_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("crx"))
        .unwrap_or(false)
    {
        extract_crx_zip_bytes(&bytes)?
    } else {
        bytes
    };
    let cursor = Cursor::new(archive_bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("open extension archive: {e}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("read extension archive entry: {e}"))?;
        let Some(relative_path) = entry.enclosed_name().map(|value| value.to_path_buf()) else {
            continue;
        };
        let output_path = destination.join(relative_path);
        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(|e| format!("create extension dir: {e}"))?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create extension parent dir: {e}"))?;
        }
        let mut buffer = Vec::new();
        entry
            .read_to_end(&mut buffer)
            .map_err(|e| format!("read extension file: {e}"))?;
        fs::write(output_path, buffer).map_err(|e| format!("write extension file: {e}"))?;
    }
    Ok(())
}

fn sync_firefox_engine_profile_extensions(
    set: &ProfileExtensionSet,
    profile_root: &Path,
) -> Result<(), String> {
    let runtime_root = profile_root.join("engine-profile").join("extensions");
    fs::create_dir_all(&runtime_root).map_err(|e| format!("create firefox extensions dir: {e}"))?;
    let managed_ids = set
        .items
        .values()
        .filter_map(|item| {
            item.browser_extension_id
                .clone()
                .map(|id| (id, item.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    eprintln!(
        "[profile-extensions] firefox runtime sync profile_root={} managed_extensions={}",
        profile_root.display(),
        managed_ids.len()
    );
    for (browser_id, item) in &managed_ids {
        let Some(source_path) = item.profile_package_path.as_deref() else {
            continue;
        };
        let runtime_path = runtime_root.join(format!("{browser_id}.xpi"));
        fs::copy(source_path, &runtime_path)
            .map_err(|e| format!("copy firefox profile extension: {e}"))?;
        eprintln!(
            "[profile-extensions] firefox runtime copy browser_id={} source={} target={}",
            browser_id,
            source_path,
            runtime_path.display()
        );
    }
    for entry in
        fs::read_dir(&runtime_root).map_err(|e| format!("read firefox extensions dir: {e}"))?
    {
        let entry = entry.map_err(|e| format!("read firefox extension entry: {e}"))?;
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !file_name.ends_with(".xpi") {
            continue;
        }
        let browser_id = file_name.trim_end_matches(".xpi");
        if !managed_ids.contains_key(browser_id) {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn cleanup_chromium_external_extension_manifests(
    profile_root: &Path,
    keep_file_names: &BTreeSet<String>,
) -> Result<(), String> {
    let external_root = profile_root
        .join("engine-profile")
        .join("External Extensions");
    if !external_root.is_dir() {
        return Ok(());
    }
    let mut removed = 0usize;
    for entry in fs::read_dir(&external_root)
        .map_err(|e| format!("read chromium external extensions dir: {e}"))?
    {
        let entry = entry.map_err(|e| format!("read chromium external extension entry: {e}"))?;
        let path = entry.path();
        if path.is_file() {
            let keep = path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| keep_file_names.contains(value))
                .unwrap_or(false);
            if keep {
                continue;
            }
            fs::remove_file(&path)
                .map_err(|e| format!("remove chromium external extension manifest: {e}"))?;
            removed += 1;
        }
    }
    eprintln!(
        "[profile-extensions] chromium external manifests cleanup profile_root={} removed={} kept={}",
        profile_root.display(),
        removed,
        keep_file_names.len()
    );
    Ok(())
}

fn register_chromium_external_manifest_for_item(
    profile_root: &Path,
    item: &ProfileInstalledExtension,
) -> Result<(), String> {
    let Some(browser_id) = item.browser_extension_id.as_deref() else {
        return Ok(());
    };
    let Some(version) = (!item.version.trim().is_empty()).then_some(item.version.trim()) else {
        return Ok(());
    };
    let Some(package_path) = item.profile_package_path.as_deref() else {
        return Ok(());
    };
    let external_root = profile_root
        .join("engine-profile")
        .join("External Extensions");
    fs::create_dir_all(&external_root).map_err(|e| {
        format!(
            "create chromium external extension root {}: {e}",
            external_root.display()
        )
    })?;
    let manifest_path = external_root.join(format!("{browser_id}.json"));
    let payload = serde_json::json!({
        "external_crx": package_path,
        "external_version": version,
    });
    let bytes = serde_json::to_vec_pretty(&payload)
        .map_err(|e| format!("serialize chromium external extension manifest: {e}"))?;
    fs::write(&manifest_path, bytes).map_err(|e| {
        format!(
            "write chromium external extension manifest {}: {e}",
            manifest_path.display()
        )
    })?;
    eprintln!(
        "[profile-extensions] chromium external manifest written browser_id={} package={} version={} path={}",
        browser_id,
        package_path,
        version,
        manifest_path.display()
    );
    Ok(())
}

fn cleanup_legacy_chromium_extension_root(profile_root: &Path) -> Result<(), String> {
    let legacy_root = profile_root.join("policy").join("chromium-extensions");
    if !legacy_root.exists() {
        return Ok(());
    }
    fs::remove_dir_all(&legacy_root).map_err(|e| {
        format!(
            "remove legacy chromium extension root {}: {e}",
            legacy_root.display()
        )
    })
}

fn sanitize_chromium_runtime_extension_state(profile_root: &Path) -> Result<(), String> {
    let secure_preferences_path = profile_root
        .join("engine-profile")
        .join("Default")
        .join("Secure Preferences");
    if !secure_preferences_path.exists() {
        return Ok(());
    }
    let raw = fs::read_to_string(&secure_preferences_path).map_err(|e| {
        format!(
            "read chromium secure preferences {}: {e}",
            secure_preferences_path.display()
        )
    })?;
    let mut value = serde_json::from_str::<serde_json::Value>(&raw).map_err(|e| {
        format!(
            "parse chromium secure preferences {}: {e}",
            secure_preferences_path.display()
        )
    })?;
    let Some(settings) = value
        .get_mut("extensions")
        .and_then(|value| value.get_mut("settings"))
        .and_then(|value| value.as_object_mut())
    else {
        return Ok(());
    };
    let legacy_root = profile_root.join("policy").join("chromium-extensions");
    let legacy_prefix = normalize_path_key(legacy_root.to_string_lossy().as_ref());
    let removed_ids = settings
        .iter()
        .filter_map(|(browser_id, item)| {
            let path = item
                .get("path")
                .and_then(|value| value.as_str())
                .map(normalize_path_key)?;
            path.starts_with(&legacy_prefix).then(|| browser_id.clone())
        })
        .collect::<Vec<_>>();
    if removed_ids.is_empty() {
        return Ok(());
    }
    settings.retain(|browser_id, _| !removed_ids.iter().any(|value| value == browser_id));
    fs::write(
        &secure_preferences_path,
        serde_json::to_vec(&value).map_err(|e| {
            format!(
                "serialize chromium secure preferences {}: {e}",
                secure_preferences_path.display()
            )
        })?,
    )
    .map_err(|e| {
        format!(
            "write chromium secure preferences {}: {e}",
            secure_preferences_path.display()
        )
    })?;
    cleanup_chromium_extension_runtime_dirs(profile_root, &removed_ids);
    eprintln!(
        "[profile-extensions] chromium runtime state sanitized profile_root={} removed_legacy_ids={:?}",
        profile_root.display(),
        removed_ids
    );
    Ok(())
}

fn cleanup_chromium_extension_runtime_dirs(profile_root: &Path, removed_ids: &[String]) {
    if removed_ids.is_empty() {
        return;
    }
    let roots = [
        profile_root
            .join("engine-profile")
            .join("Default")
            .join("Local Extension Settings"),
        profile_root
            .join("engine-profile")
            .join("Default")
            .join("Sync Extension Settings"),
        profile_root
            .join("engine-profile")
            .join("Default")
            .join("Managed Extension Settings"),
        profile_root
            .join("engine-profile")
            .join("Default")
            .join("Extensions"),
    ];
    for root in roots {
        for browser_id in removed_ids {
            let target = root.join(browser_id);
            if target.exists() {
                let _ = fs::remove_dir_all(&target);
            }
        }
    }
}

fn sync_chromium_store_from_browser(
    state: &AppState,
    profile: &ProfileMetadata,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    let secure_preferences_path = state
        .profile_root
        .join(profile.id.to_string())
        .join("engine-profile")
        .join("Default")
        .join("Secure Preferences");
    if !secure_preferences_path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(&secure_preferences_path)
        .map_err(|e| format!("read chromium secure preferences: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|e| format!("parse chromium secure preferences: {e}"))?;
    let Some(settings) = value
        .get("extensions")
        .and_then(|value| value.get("settings"))
        .and_then(|value| value.as_object())
    else {
        return Ok(false);
    };
    let mut observed_by_path = BTreeMap::new();
    let mut observed_by_id = BTreeMap::new();
    let managed_root = state
        .profile_root
        .join(profile.id.to_string())
        .join("extensions")
        .join("managed")
        .join("chromium-unpacked");
    let legacy_root = state
        .profile_root
        .join(profile.id.to_string())
        .join("policy")
        .join("chromium-extensions");
    let managed_prefix = normalize_path_key(managed_root.to_string_lossy().as_ref());
    let legacy_prefix = normalize_path_key(legacy_root.to_string_lossy().as_ref());
    let mut managed_entries = 0usize;
    let mut legacy_entries = 0usize;
    for (browser_id, item) in settings {
        let Some(path) = item.get("path").and_then(|value| value.as_str()) else {
            continue;
        };
        let enabled = item
            .get("disable_reasons")
            .and_then(|value| value.as_array())
            .map(|value| value.is_empty())
            .unwrap_or(true);
        let normalized = normalize_path_key(path);
        if normalized.starts_with(&managed_prefix) {
            managed_entries += 1;
        } else if normalized.starts_with(&legacy_prefix) {
            legacy_entries += 1;
        }
        observed_by_path.insert(normalized, (browser_id.clone(), enabled));
        observed_by_id.insert(browser_id.clone(), enabled);
    }
    eprintln!(
        "[profile-extensions] chromium browser sync profile={} secure_prefs_entries={} managed_entries={} legacy_entries={}",
        profile.id,
        observed_by_path.len(),
        managed_entries,
        legacy_entries
    );
    let mut changed = false;
    let current_ids = set.items.keys().cloned().collect::<Vec<_>>();
    for library_item_id in current_ids {
        let Some(item) = set.items.get_mut(&library_item_id) else {
            continue;
        };
        let matched = item
            .profile_unpacked_path
            .as_deref()
            .and_then(|path| observed_by_path.get(&normalize_path_key(path)).cloned())
            .or_else(|| {
                item.browser_extension_id
                    .as_deref()
                    .and_then(|browser_id| observed_by_id.get(browser_id).copied())
                    .map(|enabled| {
                        (
                            item.browser_extension_id.clone().unwrap_or_default(),
                            enabled,
                        )
                    })
            });
        let Some((browser_id, enabled)) = matched else {
            eprintln!(
                "[profile-extensions] chromium browser sync missing runtime entry profile={} library_item={} path={:?} browser_extension_id={:?}",
                profile.id,
                library_item_id,
                item.profile_unpacked_path,
                item.browser_extension_id
            );
            set.items.remove(&library_item_id);
            changed = true;
            continue;
        };
        if item.browser_extension_id.as_deref() != Some(browser_id.as_str()) {
            eprintln!(
                "[profile-extensions] chromium browser id updated profile={} library_item={} old_id={:?} new_id={}",
                profile.id,
                library_item_id,
                item.browser_extension_id,
                browser_id
            );
            item.browser_extension_id = Some(browser_id.clone());
            changed = true;
        }
        if item.enabled != enabled {
            eprintln!(
                "[profile-extensions] chromium enabled state updated profile={} library_item={} old_enabled={} new_enabled={}",
                profile.id,
                library_item_id,
                item.enabled,
                enabled
            );
            item.enabled = enabled;
            changed = true;
        }
    }
    Ok(changed)
}

fn sync_firefox_store_from_browser(
    state: &AppState,
    profile: &ProfileMetadata,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    let extensions_json_path = state
        .profile_root
        .join(profile.id.to_string())
        .join("engine-profile")
        .join("extensions.json");
    if !extensions_json_path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(&extensions_json_path)
        .map_err(|e| format!("read firefox extensions.json: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|e| format!("parse firefox extensions.json: {e}"))?;
    let Some(addons) = value.get("addons").and_then(|value| value.as_array()) else {
        return Ok(false);
    };
    let mut observed = BTreeMap::new();
    for addon in addons {
        let Some(location) = addon.get("location").and_then(|value| value.as_str()) else {
            continue;
        };
        if location != "app-profile" {
            continue;
        }
        let Some(id) = addon.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        let enabled = addon
            .get("userDisabled")
            .and_then(|value| value.as_bool())
            .map(|value| !value)
            .unwrap_or(true);
        observed.insert(id.to_string(), enabled);
    }
    eprintln!(
        "[profile-extensions] firefox browser sync profile={} observed_addons={}",
        profile.id,
        observed.len()
    );
    let current_ids = set.items.keys().cloned().collect::<Vec<_>>();
    let mut changed = false;
    for library_item_id in current_ids {
        let Some(item) = set.items.get_mut(&library_item_id) else {
            continue;
        };
        let Some(browser_id) = item.browser_extension_id.as_deref() else {
            continue;
        };
        let Some(enabled) = observed.get(browser_id) else {
            set.items.remove(&library_item_id);
            changed = true;
            continue;
        };
        if item.enabled != *enabled {
            item.enabled = *enabled;
            changed = true;
        }
    }
    Ok(changed)
}

fn normalize_path_key(path: &str) -> String {
    path.replace('/', "\\").trim().to_ascii_lowercase()
}

fn cleanup_profile_extension_artifacts(item: &ProfileInstalledExtension) {
    if let Some(path) = item.profile_unpacked_path.as_deref() {
        let _ = fs::remove_dir_all(path);
    }
    if let Some(path) = item.profile_package_path.as_deref() {
        let _ = fs::remove_file(path);
    }
}

fn profile_extension_store_file(profile_root_base: &Path, profile_id: &str) -> PathBuf {
    profile_root_base
        .join(profile_id)
        .join("extensions")
        .join("managed")
        .join("store.json")
}

fn persist_all(
    state: &AppState,
    store: &ProfileExtensionStore,
    library: &ExtensionLibraryStore,
) -> Result<(), String> {
    persist_profile_extension_store(&state.profile_root, store)?;
    let library_path = state.extension_library_path(&state.app_handle)?;
    crate::state::persist_extension_library_store(&library_path, library)
}
