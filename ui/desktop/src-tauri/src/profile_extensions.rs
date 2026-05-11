use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use browser_profile::{Engine, ProfileMetadata};
use serde::{Deserialize, Serialize};

use crate::state::{
    AppState, ExtensionLibraryItem, ExtensionLibraryStore, ExtensionPackageVariant,
};
#[path = "profile_extensions_assign.rs"]
mod assign;
#[path = "profile_extensions_browser.rs"]
mod browser;
#[path = "profile_extensions_list.rs"]
mod list;
#[path = "profile_extensions_save.rs"]
mod save;
#[path = "profile_extensions_store.rs"]
mod store;
#[path = "profile_extensions_support.rs"]
mod support;
#[path = "profile_extensions_sync.rs"]
mod sync;

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
    store::load_profile_extension_store_impl(profile_root_base, profiles)
}

pub fn persist_profile_extension_store(
    profile_root_base: &Path,
    store: &ProfileExtensionStore,
) -> Result<(), String> {
    store::persist_profile_extension_store_impl(profile_root_base, store)
}

pub fn sync_library_assignments_from_profile_store(
    library: &mut ExtensionLibraryStore,
    store: &ProfileExtensionStore,
) {
    store::sync_library_assignments_from_profile_store_impl(library, store)
}

pub fn migrate_legacy_profile_extensions(
    profile_root_base: &Path,
    profiles: &[ProfileMetadata],
    store: &mut ProfileExtensionStore,
    library: &mut ExtensionLibraryStore,
) -> Result<bool, String> {
    list::migrate_legacy_profile_extensions_impl(profile_root_base, profiles, store, library)
}

pub fn list_profile_extensions_json(state: &AppState, profile_id: &str) -> Result<String, String> {
    list::list_profile_extensions_json_impl(state, profile_id)
}

pub fn save_profile_extensions(
    state: &AppState,
    profile_id: &str,
    selections: Vec<ProfileExtensionSelection>,
) -> Result<(), String> {
    save::save_profile_extensions_impl(state, profile_id, selections)
}

pub fn set_library_item_profile_assignments(
    state: &AppState,
    library_item_id: &str,
    assigned_profile_ids: &[String],
) -> Result<(), String> {
    assign::set_library_item_profile_assignments_impl(state, library_item_id, assigned_profile_ids)
}

pub fn remove_library_item_from_profiles(
    state: &AppState,
    library_item_id: &str,
) -> Result<(), String> {
    assign::remove_library_item_from_profiles_impl(state, library_item_id)
}

pub fn prepare_profile_extensions_for_launch(
    state: &AppState,
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Result<(), String> {
    assign::prepare_profile_extensions_for_launch_impl(state, profile, profile_root)
}

pub fn collect_active_profile_extensions(
    state: &AppState,
    profile: &ProfileMetadata,
) -> Result<Vec<ProfileInstalledExtension>, String> {
    assign::collect_active_profile_extensions_impl(state, profile)
}

pub fn sync_profile_extensions_from_browser(
    state: &AppState,
    profile: &ProfileMetadata,
) -> Result<(), String> {
    sync::sync_profile_extensions_from_browser_impl(state, profile)
}

fn apply_profile_extension_selections(
    state: &AppState,
    store: &mut ProfileExtensionStore,
    library: &mut ExtensionLibraryStore,
    profile: &ProfileMetadata,
    selections: Vec<ProfileExtensionSelection>,
) -> Result<(), String> {
    assign::apply_profile_extension_selections_impl(state, store, library, profile, selections)
}

fn apply_profile_extension_selections_with_root(
    profile_root_base: &Path,
    store: &mut ProfileExtensionStore,
    library: &mut ExtensionLibraryStore,
    profile: &ProfileMetadata,
    selections: Vec<ProfileExtensionSelection>,
) -> Result<(), String> {
    assign::apply_profile_extension_selections_with_root_impl(
        profile_root_base,
        store,
        library,
        profile,
        selections,
    )
}

fn hydrate_profile_extensions_from_profile_storage(
    state: &AppState,
    profile: &ProfileMetadata,
    library: &ExtensionLibraryStore,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    assign::hydrate_profile_extensions_from_profile_storage_impl(state, profile, library, set)
}

fn collect_extension_dir_names(root: &Path, ids: &mut BTreeSet<String>) -> Result<(), String> {
    support::collect_extension_dir_names(root, ids)
}

fn suppress_dark_reader_install_tab(unpacked_dir: &Path) -> Result<(), String> {
    support::suppress_dark_reader_install_tab(unpacked_dir)
}

fn collect_profile_package_stems(root: &Path, ids: &mut BTreeSet<String>) -> Result<(), String> {
    support::collect_profile_package_stems(root, ids)
}

fn resolve_variant_for_engine(
    item: &ExtensionLibraryItem,
    engine: Engine,
) -> Option<ExtensionPackageVariant> {
    support::resolve_variant_for_engine(item, engine)
}

fn engine_scope_matches_profile(engine_scope: &str, engine: Engine) -> bool {
    support::engine_scope_matches_profile(engine_scope, engine)
}

fn package_extension(package_path: &str, package_file_name: Option<&str>) -> String {
    support::package_extension(package_path, package_file_name)
}

fn read_extension_manifest(path: &Path) -> Result<serde_json::Value, String> {
    support::read_extension_manifest(path)
}

fn chromium_extension_id_from_manifest(manifest: &serde_json::Value) -> Option<String> {
    support::chromium_extension_id_from_manifest(manifest)
}

fn firefox_extension_id_from_manifest(manifest: &serde_json::Value) -> Option<String> {
    support::firefox_extension_id_from_manifest(manifest)
}

fn sync_unpacked_chromium_extension(package_path: &Path, destination: &Path) -> Result<(), String> {
    support::sync_unpacked_chromium_extension(package_path, destination)
}

fn sync_firefox_engine_profile_extensions(
    set: &ProfileExtensionSet,
    profile_root: &Path,
) -> Result<(), String> {
    support::sync_firefox_engine_profile_extensions(set, profile_root)
}

fn cleanup_chromium_external_extension_manifests(
    profile_root: &Path,
    keep_file_names: &BTreeSet<String>,
) -> Result<(), String> {
    support::cleanup_chromium_external_extension_manifests(profile_root, keep_file_names)
}

fn register_chromium_external_manifest_for_item(
    profile_root: &Path,
    item: &ProfileInstalledExtension,
) -> Result<(), String> {
    support::register_chromium_external_manifest_for_item(profile_root, item)
}

fn cleanup_legacy_chromium_extension_root(profile_root: &Path) -> Result<(), String> {
    support::cleanup_legacy_chromium_extension_root(profile_root)
}

fn sanitize_chromium_runtime_extension_state(profile_root: &Path) -> Result<(), String> {
    support::sanitize_chromium_runtime_extension_state(profile_root)
}

fn cleanup_profile_extension_artifacts(item: &ProfileInstalledExtension) {
    support::cleanup_profile_extension_artifacts(item)
}

fn sync_chromium_store_from_browser(
    state: &AppState,
    profile: &ProfileMetadata,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    support::sync_chromium_store_from_browser(state, profile, set)
}

fn sync_firefox_store_from_browser(
    state: &AppState,
    profile: &ProfileMetadata,
    set: &mut ProfileExtensionSet,
) -> Result<bool, String> {
    support::sync_firefox_store_from_browser(state, profile, set)
}

fn persist_all(
    state: &AppState,
    store: &ProfileExtensionStore,
    library: &ExtensionLibraryStore,
) -> Result<(), String> {
    support::persist_all(state, store, library)
}


