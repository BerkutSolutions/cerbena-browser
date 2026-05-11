use crate::{
    envelope::{ok, UiEnvelope},
    platform::dialogs,
    profile_extensions::{self, ProfileExtensionSelection},
    state::{
        persist_extension_library_store, AppState, ExtensionLibraryItem, ExtensionLibraryStore,
        ExtensionPackageVariant,
    },
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use browser_extensions::{ExtensionPolicyEnforcer, OverrideGuardrails};
use browser_network_policy::{NetworkPolicy, PolicyRequest, RouteMode};
use browser_profile::Engine;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::State;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

#[path = "extensions_commands_library.rs"]
mod library;
#[path = "extensions_commands_cmd_list_library.rs"]
mod cmd_list_library;
#[path = "extensions_commands_cmd_policy_eval.rs"]
mod cmd_policy_eval;
#[path = "extensions_commands_cmd_remove_item.rs"]
mod cmd_remove_item;
#[path = "extensions_commands_cmd_set_profiles.rs"]
mod cmd_set_profiles;
#[path = "extensions_commands_cmd_update_item.rs"]
mod cmd_update_item;
#[path = "extensions_commands_first_launch.rs"]
mod first_launch;
#[path = "extensions_commands_import_source_local_file.rs"]
mod import_source_local_file;
#[path = "extensions_commands_import_source_local_folder.rs"]
mod import_source_local_folder;
#[path = "extensions_commands_import_source_store.rs"]
mod import_source_store;
#[path = "extensions_commands_profile.rs"]
mod profile;
#[path = "extensions_commands_refresh.rs"]
mod refresh;
#[path = "extensions_commands_transfer.rs"]
mod transfer;
#[path = "extensions_commands_transfer_export.rs"]
mod transfer_export;
#[path = "extensions_commands_transfer_import_manifest.rs"]
mod transfer_import_manifest;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExtensionLibraryRequest {
    pub source_kind: String,
    pub source_value: String,
    pub store_url: Option<String>,
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub logo_url: Option<String>,
    pub engine_scope: Option<String>,
    pub tags: Option<Vec<String>>,
    pub assigned_profile_ids: Vec<String>,
    pub auto_update_enabled: Option<bool>,
    pub preserve_on_panic_wipe: Option<bool>,
    pub protect_data_from_panic_wipe: Option<bool>,
    pub package_file_name: Option<String>,
    pub package_bytes_base64: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExtensionLibraryRequest {
    pub extension_id: String,
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub engine_scope: Option<String>,
    pub store_url: Option<String>,
    pub logo_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub auto_update_enabled: Option<bool>,
    pub preserve_on_panic_wipe: Option<bool>,
    pub protect_data_from_panic_wipe: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExtensionLibraryPreferencesRequest {
    pub auto_update_enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshExtensionLibraryUpdatesResponse {
    pub checked: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetExtensionProfilesRequest {
    pub extension_id: String,
    pub assigned_profile_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveExtensionRequest {
    pub extension_id: String,
    pub variant_engine_scope: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferExtensionLibraryRequest {
    pub mode: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferExtensionLibraryResponse {
    pub directory: String,
    pub file_name: Option<String>,
    pub imported: usize,
    pub exported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionLibraryTransferManifest {
    version: u32,
    mode: String,
    items: Vec<ExtensionLibraryTransferItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionLibraryTransferItem {
    display_name: String,
    version: String,
    engine_scope: String,
    source_kind: String,
    source_value: String,
    logo_url: Option<String>,
    store_url: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    auto_update_enabled: bool,
    #[serde(default)]
    preserve_on_panic_wipe: bool,
    #[serde(default)]
    protect_data_from_panic_wipe: bool,
    package_file_name: Option<String>,
    package_relative_path: Option<String>,
    #[serde(default)]
    variants: Vec<ExtensionLibraryTransferVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionLibraryTransferVariant {
    engine_scope: String,
    version: String,
    source_kind: String,
    source_value: String,
    logo_url: Option<String>,
    store_url: Option<String>,
    package_file_name: Option<String>,
    package_relative_path: Option<String>,
}

const EXTENSION_LINKS_FILE_NAME: &str = "cerbena-extensions-links.json";
const EXTENSION_ARCHIVE_DIR_NAME: &str = "cerbena-extensions-archive";
const EXTENSION_ARCHIVE_MANIFEST_FILE_NAME: &str = "manifest.json";
const EXTENSION_ARCHIVE_PACKAGES_DIR_NAME: &str = "packages";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRequestInput {
    pub has_profile_context: bool,
    pub vpn_up: bool,
    pub target_domain: String,
    pub target_service: Option<String>,
    pub tor_up: bool,
    pub dns_over_tor: bool,
    pub active_route: RouteMode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateExtensionPolicyRequest {
    pub policy: NetworkPolicy,
    pub request: PolicyRequestInput,
    pub extension_override_allowed: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProfileExtensionsRequest {
    pub profile_id: String,
    pub items: Vec<ProfileExtensionSelection>,
}

#[tauri::command]
pub fn list_extensions(
    state: State<AppState>,
    profile_id: String,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    profile::list_extensions_cmd(state, profile_id, correlation_id)
}

#[tauri::command]
pub fn save_profile_extensions(
    state: State<AppState>,
    request: SaveProfileExtensionsRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    profile::save_profile_extensions_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn list_extension_library(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    cmd_list_library::list_extension_library_cmd(state, correlation_id)
}

#[tauri::command]
pub fn import_extension_library_item(
    state: State<AppState>,
    request: ImportExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    transfer::import_extension_library_item_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn update_extension_library_item(
    state: State<AppState>,
    request: UpdateExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    cmd_update_item::update_extension_library_item_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn export_extension_library(
    state: State<AppState>,
    request: TransferExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<TransferExtensionLibraryResponse>, String> {
    transfer::export_extension_library_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn import_extension_library(
    state: State<AppState>,
    request: TransferExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<TransferExtensionLibraryResponse>, String> {
    transfer::import_extension_library_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn update_extension_library_preferences(
    state: State<AppState>,
    request: UpdateExtensionLibraryPreferencesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    library::update_extension_library_preferences_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn refresh_extension_library_updates(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<RefreshExtensionLibraryUpdatesResponse>, String> {
    library::refresh_extension_library_updates_cmd(state, correlation_id)
}

#[tauri::command]
pub fn set_extension_profiles(
    state: State<AppState>,
    request: SetExtensionProfilesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    cmd_set_profiles::set_extension_profiles_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn remove_extension_library_item(
    state: State<AppState>,
    request: RemoveExtensionRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    cmd_remove_item::remove_extension_library_item_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn install_extension(
    state: State<AppState>,
    request: ImportExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    library::install_extension_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn enable_extension(
    state: State<AppState>,
    request: SetExtensionProfilesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    library::enable_extension_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn disable_extension(
    state: State<AppState>,
    request: RemoveExtensionRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    library::disable_extension_cmd(state, request, correlation_id)
}

#[tauri::command]
pub fn process_first_launch_extensions(
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    library::process_first_launch_extensions_cmd(correlation_id)
}

#[tauri::command]
pub fn evaluate_extension_policy(
    request: EvaluateExtensionPolicyRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    cmd_policy_eval::evaluate_extension_policy_cmd(request, correlation_id)
}

pub(crate) fn refresh_extension_library_updates_impl(
    state: &AppState,
    profile_filter: Option<&str>,
) -> Result<RefreshExtensionLibraryUpdatesResponse, String> {
    refresh::refresh_extension_library_updates_impl(state, profile_filter)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransferMode {
    File,
    Archive,
}
#[path = "extensions_commands_util.rs"]
mod util;
pub(crate) use util::*;

#[cfg(test)]
#[path = "extensions_commands_tests.rs"]
mod tests;