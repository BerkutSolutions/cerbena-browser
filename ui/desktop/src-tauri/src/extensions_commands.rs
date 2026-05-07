use crate::{
    envelope::{ok, UiEnvelope},
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
    process::Command,
    time::Duration,
};
use tauri::State;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileExtensionList {
    profile_id: String,
    extensions: Vec<ExtensionLibraryItem>,
}

#[tauri::command]
pub fn list_extensions(
    state: State<AppState>,
    profile_id: String,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let extensions = library
        .items
        .values()
        .filter(|item| item.assigned_profile_ids.iter().any(|id| id == &profile_id))
        .cloned()
        .map(|mut item| {
            sync_extension_item_legacy_fields(&mut item);
            item
        })
        .collect::<Vec<_>>();
    let json = serde_json::to_string_pretty(&ProfileExtensionList {
        profile_id,
        extensions,
    })
    .map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, json))
}

#[tauri::command]
pub fn list_extension_library(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let mut normalized = library.clone();
    for item in normalized.items.values_mut() {
        sync_extension_item_legacy_fields(item);
    }
    let json = serde_json::to_string_pretty(&normalized).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, json))
}

#[tauri::command]
pub fn import_extension_library_item(
    state: State<AppState>,
    request: ImportExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let imported_ids = import_extension_library_item_impl(&state, request)?;
    Ok(ok(
        correlation_id,
        if imported_ids.len() == 1 {
            imported_ids.into_iter().next().unwrap_or_default()
        } else {
            format!("imported:{}", imported_ids.len())
        },
    ))
}

#[tauri::command]
pub fn update_extension_library_item(
    state: State<AppState>,
    request: UpdateExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let item = library
        .items
        .get_mut(&request.extension_id)
        .ok_or_else(|| "extension not found".to_string())?;
    if let Some(display_name) = request
        .display_name
        .filter(|value| !value.trim().is_empty())
    {
        item.display_name = display_name;
    }
    if let Some(version) = request.version.filter(|value| !value.trim().is_empty()) {
        item.version = version;
    }
    if let Some(engine_scope) = request
        .engine_scope
        .filter(|value| !value.trim().is_empty())
    {
        validate_assigned_profiles(&state, &engine_scope, &item.assigned_profile_ids)?;
        item.engine_scope = engine_scope;
    }
    item.store_url = request.store_url.filter(|value| !value.trim().is_empty());
    item.logo_url = request
        .logo_url
        .filter(|value| !value.trim().is_empty())
        .or(item.logo_url.clone());
    if let Some(tags) = request.tags {
        item.tags = normalize_tags(tags);
    }
    if let Some(auto_update_enabled) = request.auto_update_enabled {
        item.auto_update_enabled = auto_update_enabled;
    }
    if let Some(preserve_on_panic_wipe) = request.preserve_on_panic_wipe {
        item.preserve_on_panic_wipe = preserve_on_panic_wipe;
    }
    if let Some(protect_data_from_panic_wipe) = request.protect_data_from_panic_wipe {
        item.protect_data_from_panic_wipe = protect_data_from_panic_wipe;
    }
    sync_extension_item_legacy_fields(item);
    persist_library(&state, &library)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn export_extension_library(
    state: State<AppState>,
    request: TransferExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<TransferExtensionLibraryResponse>, String> {
    let directory = pick_folder()?;
    let response = match normalize_transfer_mode(&request.mode)? {
        TransferMode::File => export_extension_links_file(&state, &directory)?,
        TransferMode::Archive => export_extension_archive_folder(&state, &directory)?,
    };
    Ok(ok(correlation_id, response))
}

#[tauri::command]
pub fn import_extension_library(
    state: State<AppState>,
    request: TransferExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<TransferExtensionLibraryResponse>, String> {
    let mode = normalize_transfer_mode(&request.mode)?;
    let selection = pick_import_source(mode)?;
    let response = match mode {
        TransferMode::File => import_extension_links_file(&state, &selection)?,
        TransferMode::Archive => import_extension_archive_folder(&state, &selection)?,
    };
    Ok(ok(correlation_id, response))
}

#[tauri::command]
pub fn update_extension_library_preferences(
    state: State<AppState>,
    request: UpdateExtensionLibraryPreferencesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    library.auto_update_enabled = request.auto_update_enabled;
    persist_library(&state, &library)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn refresh_extension_library_updates(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<RefreshExtensionLibraryUpdatesResponse>, String> {
    let summary = refresh_extension_library_updates_impl(&state, None)?;
    Ok(ok(correlation_id, summary))
}

#[tauri::command]
pub fn set_extension_profiles(
    state: State<AppState>,
    request: SetExtensionProfilesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let item = library
        .items
        .get_mut(&request.extension_id)
        .ok_or_else(|| "extension not found".to_string())?;
    validate_assigned_profiles(&state, &item.engine_scope, &request.assigned_profile_ids)?;
    item.assigned_profile_ids = request.assigned_profile_ids;
    persist_library(&state, &library)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn remove_extension_library_item(
    state: State<AppState>,
    request: RemoveExtensionRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let removed = if let Some(variant_engine_scope) = request
        .variant_engine_scope
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized_scope = normalize_engine_scope(variant_engine_scope);
        let mut remove_item = false;
        let removed_variant_path;
        {
            let item = library
                .items
                .get_mut(&request.extension_id)
                .ok_or_else(|| "extension not found".to_string())?;
            let mut variants = normalized_extension_variants(item);
            let before = variants.len();
            let mut removed_variant = None;
            variants.retain(|variant| {
                let matches = normalize_engine_scope(&variant.engine_scope) == normalized_scope;
                if matches && removed_variant.is_none() {
                    removed_variant = Some(variant.clone());
                }
                !matches
            });
            if before == variants.len() {
                return Err("extension variant not found".to_string());
            }
            removed_variant_path = removed_variant.and_then(|variant| variant.package_path);
            if variants.is_empty() {
                remove_item = true;
            } else {
                item.package_variants = variants;
                sync_extension_item_legacy_fields(item);
            }
        }
        if remove_item {
            let removed = library.items.remove(&request.extension_id);
            if removed.is_none() {
                delete_extension_package(removed_variant_path.as_deref());
            }
            removed
        } else {
            delete_extension_package(removed_variant_path.as_deref());
            None
        }
    } else {
        library.items.remove(&request.extension_id)
    };
    persist_library(&state, &library)?;
    if let Some(item) = removed {
        for variant in normalized_extension_variants(&item) {
            delete_extension_package(variant.package_path.as_deref());
        }
    }
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn install_extension(
    state: State<AppState>,
    request: ImportExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    import_extension_library_item(state, request, correlation_id)
}

#[tauri::command]
pub fn enable_extension(
    state: State<AppState>,
    request: SetExtensionProfilesRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    set_extension_profiles(state, request, correlation_id)
}

#[tauri::command]
pub fn disable_extension(
    state: State<AppState>,
    request: RemoveExtensionRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    remove_extension_library_item(state, request, correlation_id)
}

#[tauri::command]
pub fn process_first_launch_extensions(
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    Ok(ok(correlation_id, "[]".to_string()))
}

#[tauri::command]
pub fn evaluate_extension_policy(
    request: EvaluateExtensionPolicyRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let runtime_request = PolicyRequest {
        has_profile_context: request.request.has_profile_context,
        vpn_up: request.request.vpn_up,
        target_domain: request.request.target_domain,
        target_service: request.request.target_service,
        tor_up: request.request.tor_up,
        dns_over_tor: request.request.dns_over_tor,
        active_route: request.request.active_route,
    };
    let enforcer = ExtensionPolicyEnforcer::default();
    let guardrails = OverrideGuardrails {
        require_explicit_allow: true,
        allow_service_override: true,
    };
    let decision = enforcer.evaluate(
        &request.policy,
        &runtime_request,
        request.extension_override_allowed,
        &guardrails,
    );
    let payload = serde_json::to_string_pretty(&decision).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, payload))
}

fn import_extension_library_item_impl(
    state: &AppState,
    request: ImportExtensionLibraryRequest,
) -> Result<Vec<String>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let normalized_store_url = request
        .store_url
        .clone()
        .filter(|value| !value.trim().is_empty());
    let package_metadata_batch =
        derive_extension_metadata_batch(&request, normalized_store_url.as_deref())?;
    let display_name_override = request
        .display_name
        .clone()
        .filter(|value| !value.trim().is_empty());
    let version_override = request
        .version
        .clone()
        .filter(|value| !value.trim().is_empty());
    let engine_scope_override = request
        .engine_scope
        .clone()
        .filter(|value| !value.trim().is_empty());
    let logo_url_override = request
        .logo_url
        .clone()
        .filter(|value| !value.trim().is_empty());
    let normalized_tags = normalize_tags(request.tags.clone().unwrap_or_default());
    let is_single_import = package_metadata_batch.len() == 1;
    let mut imported_ids = Vec::new();
    let normalized_store_url = normalized_store_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let logo_url_override = logo_url_override.as_deref().map(str::trim).filter(|value| !value.is_empty());

    for package_metadata in package_metadata_batch {
        let seed = display_name_override
            .as_deref()
            .filter(|_| imported_ids.is_empty())
            .or(package_metadata.stable_id.as_deref())
            .or(package_metadata.display_name.as_deref())
            .or(normalized_store_url.as_deref())
            .unwrap_or(&request.source_value);
        let inferred_name = display_name_override
            .clone()
            .filter(|_| is_single_import)
            .or(package_metadata.display_name.clone())
            .unwrap_or_else(|| {
                infer_extension_name(normalized_store_url, &request.source_value)
            });
        let inferred_engine = engine_scope_override
            .clone()
            .or(package_metadata.engine_scope.clone())
            .unwrap_or_else(|| {
                infer_engine_scope(normalized_store_url, &request.source_value)
            });
        validate_assigned_profiles(state, &inferred_engine, &request.assigned_profile_ids)?;
        let merge_target_id =
            find_merge_target_extension_id(&library, &inferred_name, &inferred_engine);
        let persist_id = merge_target_id
            .clone()
            .unwrap_or_else(|| build_extension_id(seed, &library));
        let (package_path, package_file_name) = persist_extension_package(
            state,
            &persist_id,
            package_metadata.package_bytes.as_deref(),
            package_metadata.package_extension.as_deref(),
            package_metadata.package_file_name.as_deref(),
        )?;
        let version = version_override
            .clone()
            .filter(|_| is_single_import)
            .or(package_metadata.version.clone())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
        let variant = build_package_variant(
            &request,
            &package_metadata,
            &inferred_engine,
            &version,
            package_path,
            package_file_name,
            normalized_store_url,
            logo_url_override,
        );
        if let Some(existing_id) = merge_target_id {
            let item = library
                .items
                .get_mut(&existing_id)
                .ok_or_else(|| "extension not found during merge".to_string())?;
            if !display_name_override.as_deref().unwrap_or("").trim().is_empty() || item.display_name.trim().is_empty() {
                item.display_name = inferred_name.clone();
            }
            item.tags = normalize_tags([item.tags.clone(), normalized_tags.clone()].concat());
            item.assigned_profile_ids = {
                let mut merged = item.assigned_profile_ids.clone();
                merged.extend(request.assigned_profile_ids.clone());
                merged.sort();
                merged.dedup();
                merged
            };
            item.auto_update_enabled = request.auto_update_enabled.unwrap_or(item.auto_update_enabled);
            item.preserve_on_panic_wipe =
                request.preserve_on_panic_wipe.unwrap_or(item.preserve_on_panic_wipe);
            item.protect_data_from_panic_wipe =
                request.protect_data_from_panic_wipe.unwrap_or(item.protect_data_from_panic_wipe);
            let mut variants = normalized_extension_variants(item);
            variants.retain(|existing| {
                normalize_engine_scope(&existing.engine_scope)
                    != normalize_engine_scope(&variant.engine_scope)
            });
            variants.push(variant);
            item.package_variants = variants;
            sync_extension_item_legacy_fields(item);
            imported_ids.push(existing_id);
            continue;
        }

        let id = persist_id;
        let mut item = ExtensionLibraryItem {
            id: id.clone(),
            display_name: inferred_name,
            version,
            engine_scope: inferred_engine,
            source_kind: request.source_kind.clone(),
            source_value: request.source_value.clone(),
            logo_url: logo_url_override
                .map(str::to_string)
                .or(package_metadata.logo_url),
            store_url: normalized_store_url.map(str::to_string),
            tags: normalized_tags.clone(),
            assigned_profile_ids: request.assigned_profile_ids.clone(),
            auto_update_enabled: request.auto_update_enabled.unwrap_or(false),
            preserve_on_panic_wipe: request.preserve_on_panic_wipe.unwrap_or(false),
            protect_data_from_panic_wipe: request.protect_data_from_panic_wipe.unwrap_or(false),
            package_path: None,
            package_file_name: None,
            package_variants: vec![variant],
        };
        sync_extension_item_legacy_fields(&mut item);
        library.items.insert(id.clone(), item);
        imported_ids.push(id);
    }
    persist_library(state, &library)?;
    Ok(imported_ids)
}

fn persist_library(state: &AppState, store: &ExtensionLibraryStore) -> Result<(), String> {
    let path = state.extension_library_path(&state.app_handle)?;
    persist_extension_library_store(&path, store)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransferMode {
    File,
    Archive,
}

fn normalize_transfer_mode(value: &str) -> Result<TransferMode, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "file" => Ok(TransferMode::File),
        "archive" => Ok(TransferMode::Archive),
        _ => Err("unsupported transfer mode".to_string()),
    }
}

fn pick_folder() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.ShowNewFolderButton = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  $dialog.SelectedPath | ConvertTo-Json -Compress
}
"#;
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", script])
            .output()
            .map_err(|e| format!("folder picker failed: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                "folder picker failed".to_string()
            } else {
                format!("folder picker failed: {stderr}")
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            return Err("folder selection was cancelled".to_string());
        }
        return serde_json::from_str::<String>(&stdout)
            .map_err(|e| format!("folder picker parse failed: {e}"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("folder picker is not supported on this platform".to_string())
    }
}

fn pick_import_source(mode: TransferMode) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let (filter, title) = match mode {
            TransferMode::File => (
                "Cerbena extension links (cerbena-extensions-links.json)|cerbena-extensions-links.json|JSON files (*.json)|*.json",
                "Select Cerbena extension links file",
            ),
            TransferMode::Archive => (
                "Cerbena archive manifest (manifest.json)|manifest.json|JSON files (*.json)|*.json",
                "Select Cerbena archive manifest file",
            ),
        };
        let script = format!(
            r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Filter = '{filter}'
$dialog.Title = '{title}'
$dialog.Multiselect = $false
$dialog.CheckFileExists = $true
$dialog.CheckPathExists = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{
  $dialog.FileName | ConvertTo-Json -Compress
}}
"#
        );
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .map_err(|e| format!("import picker failed: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                "import picker failed".to_string()
            } else {
                format!("import picker failed: {stderr}")
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            return Err("import selection was cancelled".to_string());
        }
        return serde_json::from_str::<String>(&stdout)
            .map_err(|e| format!("import picker parse failed: {e}"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = mode;
        Err("import picker is not supported on this platform".to_string())
    }
}

fn export_extension_links_file(
    state: &AppState,
    directory: &str,
) -> Result<TransferExtensionLibraryResponse, String> {
    let library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?
        .clone();
    let mut skipped = 0usize;
    let manifest = ExtensionLibraryTransferManifest {
        version: 1,
        mode: "file".to_string(),
        items: library
            .items
            .values()
            .filter_map(|item| {
                let linkable = normalized_extension_variants(item).iter().any(|variant| {
                    variant
                        .store_url
                        .as_deref()
                        .map(str::trim)
                        .is_some_and(|value| !value.is_empty())
                        || variant.source_value.trim().starts_with("http://")
                        || variant.source_value.trim().starts_with("https://")
                });
                if !linkable {
                    skipped += 1;
                    return None;
                }
                Some(transfer_item_from_library_item(
                    item.clone(),
                    normalized_extension_variants(item)
                        .into_iter()
                        .map(|variant| ExtensionLibraryTransferVariant {
                            engine_scope: variant.engine_scope,
                            version: variant.version,
                            source_kind: variant.source_kind,
                            source_value: variant.source_value,
                            logo_url: variant.logo_url,
                            store_url: variant.store_url,
                            package_file_name: variant.package_file_name,
                            package_relative_path: None,
                        })
                        .collect(),
                ))
            })
            .collect(),
    };
    let target_dir = PathBuf::from(directory);
    fs::create_dir_all(&target_dir).map_err(|e| format!("create export dir: {e}"))?;
    let file_path = target_dir.join(EXTENSION_LINKS_FILE_NAME);
    let bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("serialize export: {e}"))?;
    fs::write(&file_path, bytes).map_err(|e| format!("write export file: {e}"))?;
    Ok(TransferExtensionLibraryResponse {
        directory: target_dir.to_string_lossy().to_string(),
        file_name: Some(EXTENSION_LINKS_FILE_NAME.to_string()),
        imported: 0,
        exported: manifest.items.len(),
        skipped,
        errors: Vec::new(),
    })
}

fn export_extension_archive_folder(
    state: &AppState,
    directory: &str,
) -> Result<TransferExtensionLibraryResponse, String> {
    let library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?
        .clone();
    let target_root = PathBuf::from(directory).join(EXTENSION_ARCHIVE_DIR_NAME);
    let packages_dir = target_root.join(EXTENSION_ARCHIVE_PACKAGES_DIR_NAME);
    fs::create_dir_all(&packages_dir).map_err(|e| format!("create archive dir: {e}"))?;
    let mut errors = Vec::new();
    let mut manifest_items = Vec::new();

    for item in library.items.values() {
        let variant_transfers = normalized_extension_variants(item)
            .into_iter()
            .map(|variant| {
                let package_relative_path = variant.package_path.as_deref().and_then(|package_path| {
                    let source_path = PathBuf::from(package_path);
                    if !source_path.is_file() {
                        errors.push(format!("{}: package file is missing", item.display_name));
                        return None;
                    }
                    let file_name = variant
                        .package_file_name
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| {
                            source_path
                                .file_name()
                                .and_then(|value| value.to_str())
                                .unwrap_or("extension.zip")
                                .to_string()
                        });
                    let safe_name = sanitize_file_name(&file_name);
                    let unique_name = unique_archive_file_name(
                        &packages_dir,
                        &format!("{}-{}", item.id, normalize_engine_scope(&variant.engine_scope)),
                        &safe_name,
                    );
                    let dest_path = packages_dir.join(&unique_name);
                    match fs::copy(&source_path, &dest_path) {
                        Ok(_) => Some(format!(
                            "{EXTENSION_ARCHIVE_PACKAGES_DIR_NAME}/{unique_name}"
                        )),
                        Err(error) => {
                            errors.push(format!(
                                "{}: copy package failed: {error}",
                                item.display_name
                            ));
                            None
                        }
                    }
                });
                ExtensionLibraryTransferVariant {
                    engine_scope: normalize_engine_scope(&variant.engine_scope),
                    version: variant.version,
                    source_kind: variant.source_kind,
                    source_value: variant.source_value,
                    logo_url: variant.logo_url,
                    store_url: variant.store_url,
                    package_file_name: variant.package_file_name,
                    package_relative_path,
                }
            })
            .collect::<Vec<_>>();
        manifest_items.push(transfer_item_from_library_item(
            item.clone(),
            variant_transfers,
        ));
    }

    let manifest = ExtensionLibraryTransferManifest {
        version: 1,
        mode: "archive".to_string(),
        items: manifest_items,
    };
    let manifest_path = target_root.join(EXTENSION_ARCHIVE_MANIFEST_FILE_NAME);
    let bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("serialize archive: {e}"))?;
    fs::write(&manifest_path, bytes).map_err(|e| format!("write archive manifest: {e}"))?;

    Ok(TransferExtensionLibraryResponse {
        directory: target_root.to_string_lossy().to_string(),
        file_name: Some(EXTENSION_ARCHIVE_MANIFEST_FILE_NAME.to_string()),
        imported: 0,
        exported: manifest.items.len(),
        skipped: 0,
        errors,
    })
}

fn import_extension_links_file(
    state: &AppState,
    manifest_file: &str,
) -> Result<TransferExtensionLibraryResponse, String> {
    let manifest_path = PathBuf::from(manifest_file);
    let manifest = read_transfer_manifest(&manifest_path)?;
    let base_dir = manifest_path
        .parent()
        .ok_or_else(|| "import file parent directory could not be resolved".to_string())?;
    import_transfer_manifest(state, &manifest, base_dir, TransferMode::File)
}

fn import_extension_archive_folder(
    state: &AppState,
    manifest_file: &str,
) -> Result<TransferExtensionLibraryResponse, String> {
    let chosen = PathBuf::from(manifest_file);
    let archive_root = chosen
        .parent()
        .ok_or_else(|| "archive manifest parent directory could not be resolved".to_string())?
        .to_path_buf();
    let manifest_path = archive_root.join(EXTENSION_ARCHIVE_MANIFEST_FILE_NAME);
    let manifest = read_transfer_manifest(&manifest_path)?;
    import_transfer_manifest(state, &manifest, &archive_root, TransferMode::Archive)
}

fn read_transfer_manifest(path: &Path) -> Result<ExtensionLibraryTransferManifest, String> {
    let bytes = fs::read(path).map_err(|e| format!("read manifest {}: {e}", path.display()))?;
    serde_json::from_slice::<ExtensionLibraryTransferManifest>(&bytes)
        .map_err(|e| format!("parse manifest {}: {e}", path.display()))
}

fn import_transfer_manifest(
    state: &AppState,
    manifest: &ExtensionLibraryTransferManifest,
    base_dir: &Path,
    mode: TransferMode,
) -> Result<TransferExtensionLibraryResponse, String> {
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();

    for item in &manifest.items {
        let import_requests = match build_import_requests_from_transfer_item(item, base_dir, mode) {
            Ok(requests) if requests.is_empty() => {
                skipped += 1;
                continue;
            }
            Ok(requests) => requests,
            Err(error) => {
                errors.push(format!("{}: {error}", item.display_name));
                skipped += 1;
                continue;
            }
        };
        for import_request in import_requests {
            match import_extension_library_item_impl(state, import_request) {
                Ok(ids) => imported += ids.len(),
                Err(error) => errors.push(format!("{}: {error}", item.display_name)),
            }
        }
    }

    Ok(TransferExtensionLibraryResponse {
        directory: base_dir.to_string_lossy().to_string(),
        file_name: Some(match mode {
            TransferMode::File => EXTENSION_LINKS_FILE_NAME.to_string(),
            TransferMode::Archive => EXTENSION_ARCHIVE_MANIFEST_FILE_NAME.to_string(),
        }),
        imported,
        exported: 0,
        skipped,
        errors,
    })
}

fn transfer_item_from_library_item(
    item: ExtensionLibraryItem,
    variants: Vec<ExtensionLibraryTransferVariant>,
) -> ExtensionLibraryTransferItem {
    ExtensionLibraryTransferItem {
        display_name: item.display_name,
        version: item.version,
        engine_scope: item.engine_scope,
        source_kind: item.source_kind,
        source_value: item.source_value,
        logo_url: item.logo_url,
        store_url: item.store_url,
        tags: normalize_tags(item.tags),
        auto_update_enabled: item.auto_update_enabled,
        preserve_on_panic_wipe: item.preserve_on_panic_wipe,
        protect_data_from_panic_wipe: item.protect_data_from_panic_wipe,
        package_file_name: item.package_file_name,
        package_relative_path: variants
            .first()
            .and_then(|variant| variant.package_relative_path.clone()),
        variants,
    }
}

fn build_import_requests_from_transfer_item(
    item: &ExtensionLibraryTransferItem,
    base_dir: &Path,
    mode: TransferMode,
) -> Result<Vec<ImportExtensionLibraryRequest>, String> {
    let variants = if item.variants.is_empty() {
        vec![ExtensionLibraryTransferVariant {
            engine_scope: item.engine_scope.clone(),
            version: item.version.clone(),
            source_kind: item.source_kind.clone(),
            source_value: item.source_value.clone(),
            logo_url: item.logo_url.clone(),
            store_url: item.store_url.clone(),
            package_file_name: item.package_file_name.clone(),
            package_relative_path: item.package_relative_path.clone(),
        }]
    } else {
        item.variants.clone()
    };

    let mut requests = Vec::new();
    for variant in variants {
        let package_bytes_base64 = match mode {
            TransferMode::Archive => match variant.package_relative_path.as_deref() {
                Some(relative_path) => {
                    let package_path = base_dir.join(relative_path.replace('/', "\\"));
                    if !package_path.is_file() {
                        None
                    } else {
                        let bytes = fs::read(&package_path)
                            .map_err(|e| format!("read package {}: {e}", package_path.display()))?;
                        Some(BASE64_STANDARD.encode(bytes))
                    }
                }
                None => None,
            },
            TransferMode::File => None,
        };

        let source_kind = if package_bytes_base64.is_some() {
            "local_file".to_string()
        } else {
            variant.source_kind.clone()
        };
        let source_value = if package_bytes_base64.is_some() {
            variant
                .package_file_name
                .clone()
                .unwrap_or_else(|| item.display_name.clone())
        } else if let Some(store_url) = variant
            .store_url
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            store_url
        } else if variant.source_value.trim().starts_with("http://")
            || variant.source_value.trim().starts_with("https://")
        {
            variant.source_value.clone()
        } else {
            continue;
        };

        requests.push(ImportExtensionLibraryRequest {
            source_kind,
            source_value: source_value.clone(),
            store_url: variant.store_url.clone(),
            display_name: Some(item.display_name.clone()),
            version: Some(variant.version.clone()),
            logo_url: variant.logo_url.clone().or_else(|| item.logo_url.clone()),
            engine_scope: Some(variant.engine_scope.clone()),
            tags: Some(item.tags.clone()),
            assigned_profile_ids: Vec::new(),
            auto_update_enabled: Some(item.auto_update_enabled),
            preserve_on_panic_wipe: Some(item.preserve_on_panic_wipe),
            protect_data_from_panic_wipe: Some(item.protect_data_from_panic_wipe),
            package_file_name: variant.package_file_name.clone(),
            package_bytes_base64,
        });
    }

    Ok(requests)
}

fn sanitize_file_name(value: &str) -> String {
    let mut cleaned = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    while cleaned.contains("__") {
        cleaned = cleaned.replace("__", "_");
    }
    let trimmed = cleaned.trim_matches('_').trim().to_string();
    if trimmed.is_empty() {
        "extension.zip".to_string()
    } else {
        trimmed
    }
}

fn unique_archive_file_name(
    packages_dir: &Path,
    extension_id: &str,
    original_name: &str,
) -> String {
    let extension = Path::new(original_name)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("zip");
    let stem = Path::new(original_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("extension");
    let base = format!(
        "{}-{}",
        sanitize_file_name(extension_id),
        sanitize_file_name(stem)
    );
    let mut candidate = format!("{base}.{extension}");
    let mut counter = 2usize;
    while packages_dir.join(&candidate).exists() {
        candidate = format!("{base}-{counter}.{extension}");
        counter += 1;
    }
    candidate
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    let mut seen = BTreeSet::new();
    for value in tags {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = trimmed.to_ascii_lowercase();
        if seen.insert(normalized) {
            unique.push(trimmed.to_string());
        }
    }
    unique
}

pub fn refresh_extension_library_updates_impl(
    state: &AppState,
    profile_filter: Option<&str>,
) -> Result<RefreshExtensionLibraryUpdatesResponse, String> {
    let snapshot = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?
        .clone();
    if !snapshot.auto_update_enabled {
        return Ok(RefreshExtensionLibraryUpdatesResponse {
            checked: 0,
            updated: 0,
            skipped: snapshot.items.len(),
            errors: Vec::new(),
        });
    }

    let items = snapshot
        .items
        .values()
        .filter(|item| item.auto_update_enabled)
        .filter(|item| {
            item.store_url
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
        })
        .filter(|item| {
            profile_filter
                .map(|profile_id| item.assigned_profile_ids.iter().any(|id| id == profile_id))
                .unwrap_or(true)
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut summary = RefreshExtensionLibraryUpdatesResponse {
        checked: items.len(),
        updated: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    for item in items {
        match refresh_extension_library_item(state, &item.id) {
            Ok(updated) => {
                if updated {
                    summary.updated += 1;
                } else {
                    summary.skipped += 1;
                }
            }
            Err(error) => {
                summary
                    .errors
                    .push(format!("{}: {error}", item.display_name));
            }
        }
    }

    Ok(summary)
}

fn refresh_extension_library_item(state: &AppState, extension_id: &str) -> Result<bool, String> {
    let item_snapshot = {
        let library = state
            .extension_library
            .lock()
            .map_err(|_| "extension library lock poisoned".to_string())?;
        library
            .items
            .get(extension_id)
            .cloned()
            .ok_or_else(|| "extension not found".to_string())?
    };
    let mut refreshed_variants = Vec::new();
    let mut refreshed_any = false;
    for variant in normalized_extension_variants(&item_snapshot) {
        let store_url = variant
            .store_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(store_url) = store_url {
            let metadata = download_store_extension_metadata(store_url)?;
            let (package_path, package_file_name) = persist_extension_package(
                state,
                &item_snapshot.id,
                metadata.package_bytes.as_deref(),
                metadata.package_extension.as_deref(),
                metadata.package_file_name.as_deref(),
            )?;
            let engine_scope = metadata
                .engine_scope
                .clone()
                .unwrap_or_else(|| variant.engine_scope.clone());
            refreshed_variants.push(ExtensionPackageVariant {
                engine_scope,
                version: metadata
                    .version
                    .clone()
                    .unwrap_or_else(|| variant.version.clone()),
                source_kind: variant.source_kind.clone(),
                source_value: variant.source_value.clone(),
                logo_url: metadata.logo_url.clone().or_else(|| variant.logo_url.clone()),
                store_url: Some(store_url.to_string()),
                package_path: package_path.or_else(|| variant.package_path.clone()),
                package_file_name: package_file_name.or_else(|| variant.package_file_name.clone()),
            });
            refreshed_any = true;
        } else {
            refreshed_variants.push(variant);
        }
    }
    if !refreshed_any {
        return Err("store URL is not configured".to_string());
    }

    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let item = library
        .items
        .get_mut(extension_id)
        .ok_or_else(|| "extension not found".to_string())?;

    let mut changed = false;
    if normalized_extension_variants(item) != refreshed_variants {
        item.package_variants = refreshed_variants;
        changed = true;
    }
    let before_display_name = item.display_name.clone();
    sync_extension_item_legacy_fields(item);
    if item.display_name != before_display_name {
        item.display_name = before_display_name;
    }

    if changed {
        persist_library(state, &library)?;
    }
    Ok(changed)
}

fn build_extension_id(seed: &str, library: &ExtensionLibraryStore) -> String {
    let mut base = seed
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if base.is_empty() {
        base = "extension".to_string();
    }
    let mut candidate = base.clone();
    let mut index = 2u32;
    while library.items.contains_key(&candidate) {
        candidate = format!("{base}-{index}");
        index += 1;
    }
    candidate
}

fn infer_extension_name(store_url: Option<&str>, source_value: &str) -> String {
    let seed = store_url.unwrap_or(source_value);
    seed.rsplit('/')
        .find(|segment| !segment.trim().is_empty())
        .map(|segment| segment.replace('-', " ").replace('_', " "))
        .filter(|segment| !segment.trim().is_empty())
        .unwrap_or_else(|| "Extension".to_string())
}

fn infer_engine_scope(store_url: Option<&str>, source_value: &str) -> String {
    let store = store_url.unwrap_or_default().to_lowercase();
    let source = source_value.to_lowercase();
    if store.contains("addons.mozilla.org") || source.ends_with(".xpi") {
        "firefox".to_string()
    } else if store.contains("chromewebstore.google.com")
        || store.contains("chrome.google.com")
        || source.ends_with(".crx")
    {
        "chromium".to_string()
    } else {
        "chromium/firefox".to_string()
    }
}

#[derive(Default)]
struct DerivedExtensionMetadata {
    stable_id: Option<String>,
    display_name: Option<String>,
    version: Option<String>,
    engine_scope: Option<String>,
    logo_url: Option<String>,
    package_bytes: Option<Vec<u8>>,
    package_extension: Option<String>,
    package_file_name: Option<String>,
}

fn normalized_extension_variants(item: &ExtensionLibraryItem) -> Vec<ExtensionPackageVariant> {
    let mut variants = item
        .package_variants
        .iter()
        .filter_map(|variant| {
            let engine_scope = normalize_engine_scope(&variant.engine_scope);
            let version = variant.version.trim().to_string();
            if engine_scope.is_empty() || version.is_empty() {
                return None;
            }
            Some(ExtensionPackageVariant {
                engine_scope,
                version,
                source_kind: variant.source_kind.trim().to_string(),
                source_value: variant.source_value.trim().to_string(),
                logo_url: variant
                    .logo_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                store_url: variant
                    .store_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                package_path: variant
                    .package_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                package_file_name: variant
                    .package_file_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
            })
        })
        .collect::<Vec<_>>();

    if variants.is_empty() {
        variants.push(ExtensionPackageVariant {
            engine_scope: normalize_engine_scope(&item.engine_scope),
            version: item.version.trim().to_string(),
            source_kind: item.source_kind.trim().to_string(),
            source_value: item.source_value.trim().to_string(),
            logo_url: item
                .logo_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            store_url: item
                .store_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            package_path: item
                .package_path
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            package_file_name: item
                .package_file_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        });
    }

    variants.sort_by(|left, right| left.engine_scope.cmp(&right.engine_scope));
    variants.dedup_by(|left, right| left.engine_scope == right.engine_scope);
    variants
}

fn package_variant_for_engine(
    item: &ExtensionLibraryItem,
    engine_scope: &str,
) -> Option<ExtensionPackageVariant> {
    let expected = normalize_engine_scope(engine_scope);
    normalized_extension_variants(item)
        .into_iter()
        .find(|variant| normalize_engine_scope(&variant.engine_scope) == expected)
}

fn primary_extension_variant(item: &ExtensionLibraryItem) -> Option<ExtensionPackageVariant> {
    let variants = normalized_extension_variants(item);
    if variants.is_empty() {
        return None;
    }
    variants
        .iter()
        .find(|variant| normalize_engine_scope(&variant.engine_scope) == "chromium/firefox")
        .cloned()
        .or_else(|| {
            variants
                .iter()
                .find(|variant| normalize_engine_scope(&variant.engine_scope) == "chromium")
                .cloned()
        })
        .or_else(|| variants.iter().find(|variant| normalize_engine_scope(&variant.engine_scope) == "firefox").cloned())
        .or_else(|| variants.into_iter().next())
}

fn sync_extension_item_legacy_fields(item: &mut ExtensionLibraryItem) {
    let variants = normalized_extension_variants(item);
    item.package_variants = variants.clone();
    item.engine_scope = combined_engine_scope_from_variants(&variants);
    if let Some(primary) = primary_extension_variant(item) {
        item.version = primary.version;
        item.source_kind = primary.source_kind;
        item.source_value = primary.source_value;
        item.logo_url = primary.logo_url;
        item.store_url = primary.store_url;
        item.package_path = primary.package_path;
        item.package_file_name = primary.package_file_name;
    }
}

fn combined_engine_scope_from_variants(variants: &[ExtensionPackageVariant]) -> String {
    let has_chromium = variants
        .iter()
        .any(|variant| normalize_engine_scope(&variant.engine_scope) == "chromium");
    let has_firefox = variants
        .iter()
        .any(|variant| normalize_engine_scope(&variant.engine_scope) == "firefox");
    if has_chromium && has_firefox {
        "chromium/firefox".to_string()
    } else if has_firefox {
        "firefox".to_string()
    } else if has_chromium {
        "chromium".to_string()
    } else {
        "chromium/firefox".to_string()
    }
}

fn build_package_variant(
    request: &ImportExtensionLibraryRequest,
    metadata: &DerivedExtensionMetadata,
    engine_scope: &str,
    version: &str,
    package_path: Option<String>,
    package_file_name: Option<String>,
    normalized_store_url: Option<&str>,
    logo_url_override: Option<&str>,
) -> ExtensionPackageVariant {
    ExtensionPackageVariant {
        engine_scope: normalize_engine_scope(engine_scope),
        version: version.to_string(),
        source_kind: request.source_kind.clone(),
        source_value: request.source_value.clone(),
        logo_url: logo_url_override
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .or_else(|| metadata.logo_url.clone()),
        store_url: normalized_store_url.map(str::to_string),
        package_path,
        package_file_name,
    }
}

fn normalized_extension_match_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn find_merge_target_extension_id(
    library: &ExtensionLibraryStore,
    display_name: &str,
    requested_engine_scope: &str,
) -> Option<String> {
    let expected_name = normalized_extension_match_name(display_name);
    let expected_scope = normalize_engine_scope(requested_engine_scope);
    if expected_name.is_empty() {
        return None;
    }

    library
        .items
        .values()
        .find(|item| {
            normalized_extension_match_name(&item.display_name) == expected_name
                && package_variant_for_engine(item, &expected_scope).is_none()
        })
        .map(|item| item.id.clone())
        .or_else(|| {
            library
                .items
                .values()
                .find(|item| normalized_extension_match_name(&item.display_name) == expected_name)
                .map(|item| item.id.clone())
        })
}

fn validate_assigned_profiles(
    state: &AppState,
    engine_scope: &str,
    assigned_profile_ids: &[String],
) -> Result<(), String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| "profile manager lock poisoned".to_string())?;
    for profile_id in assigned_profile_ids {
        let uuid = uuid::Uuid::parse_str(profile_id)
            .map_err(|e| format!("profile id parse failed: {e}"))?;
        let profile = manager
            .get_profile(uuid)
            .map_err(|e| format!("profile lookup failed: {e}"))?;
        if !engine_scope_matches_profile(engine_scope, profile.engine) {
            return Err(format!(
                "extension engine scope `{engine_scope}` is incompatible with profile `{}`",
                profile.name
            ));
        }
    }
    Ok(())
}

fn engine_scope_matches_profile(engine_scope: &str, engine: Engine) -> bool {
    match normalize_engine_scope(engine_scope).as_str() {
        "firefox" => matches!(engine, Engine::Librewolf),
        "chromium" => engine.is_chromium_family(),
        _ => true,
    }
}

fn normalize_engine_scope(value: &str) -> String {
    let normalized = value.trim().to_lowercase();
    if normalized == "firefox" {
        "firefox".to_string()
    } else if normalized == "chromium" {
        "chromium".to_string()
    } else {
        "chromium/firefox".to_string()
    }
}

fn derive_extension_metadata_batch(
    request: &ImportExtensionLibraryRequest,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    if let Some(raw_bytes) = request
        .package_bytes_base64
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let bytes = BASE64_STANDARD
            .decode(raw_bytes)
            .map_err(|e| format!("decode extension package: {e}"))?;
        let file_name = request
            .package_file_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&request.source_value);
        return read_extension_archive_metadata_batch_from_bytes(&bytes, file_name, store_url);
    }

    let lower_kind = request.source_kind.to_lowercase();
    if lower_kind == "store_url" {
        return download_store_extension_metadata(
            store_url.unwrap_or(request.source_value.as_str()),
        )
        .map(|metadata| vec![metadata]);
    }

    if matches!(lower_kind.as_str(), "local_folder" | "local_folder_picker" | "dropped_folder") {
        let folder = if lower_kind == "local_folder_picker" {
            PathBuf::from(pick_folder()?)
        } else {
            PathBuf::from(request.source_value.trim())
        };
        return read_extension_directory_metadata_batch(&folder, store_url);
    }

    if matches!(lower_kind.as_str(), "local_file" | "dropped_file") {
        let path = Path::new(&request.source_value);
        if path.exists() {
            return read_extension_archive_metadata_batch(path, store_url);
        }
    }

    Ok(vec![DerivedExtensionMetadata {
        stable_id: store_url_fallback_id(store_url),
        engine_scope: Some(infer_engine_scope(store_url, &request.source_value)),
        ..DerivedExtensionMetadata::default()
    }])
}

#[allow(dead_code)]
fn read_extension_archive_metadata(
    path: &Path,
    store_url: Option<&str>,
) -> Result<DerivedExtensionMetadata, String> {
    let mut batch = read_extension_archive_metadata_batch(path, store_url)?;
    let first = batch
        .drain(..)
        .next()
        .ok_or_else(|| "extension package manifest.json not found".to_string());
    first
}

fn read_extension_archive_metadata_batch(
    path: &Path,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    let bytes = fs::read(path).map_err(|e| format!("read extension package: {e}"))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("extension.zip");
    read_extension_archive_metadata_batch_from_bytes(&bytes, file_name, store_url)
}

fn read_extension_directory_metadata_batch(
    path: &Path,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    if !path.exists() {
        return Err(format!("extension directory not found: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!(
            "extension directory path is not a folder: {}",
            path.display()
        ));
    }

    let roots = discover_extension_directory_roots(path)?;
    if roots.is_empty() {
        return Err(format!(
            "extension directory manifest.json not found under {}",
            path.display()
        ));
    }

    let mut batch = Vec::new();
    for root in roots {
        let zip_bytes = package_extension_directory(&root)?;
        let base_name = root
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("extension");
        let provisional_name = format!("{base_name}.zip");
        let mut metadata =
            read_extension_archive_metadata_from_bytes(&zip_bytes, &provisional_name, store_url)?;
        let package_extension = match metadata.engine_scope.as_deref() {
            Some("firefox") => "xpi",
            _ => "zip",
        };
        metadata.package_bytes = Some(zip_bytes);
        metadata.package_extension = Some(package_extension.to_string());
        metadata.package_file_name = Some(package_display_name(
            &format!("{base_name}.{package_extension}"),
            package_extension,
        ));
        batch.push(metadata);
    }
    Ok(batch)
}

fn read_extension_archive_metadata_from_bytes(
    bytes: &[u8],
    file_name: &str,
    store_url: Option<&str>,
) -> Result<DerivedExtensionMetadata, String> {
    let mut batch = read_extension_archive_metadata_batch_from_bytes(bytes, file_name, store_url)?;
    let first = batch
        .drain(..)
        .next()
        .ok_or_else(|| "extension package manifest.json not found".to_string());
    first
}

fn read_extension_archive_metadata_batch_from_bytes(
    bytes: &[u8],
    file_name: &str,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    let package_extension = infer_package_extension(file_name, store_url);
    let archive_bytes = if package_extension.eq_ignore_ascii_case("crx") {
        extract_embedded_zip_bytes(bytes)?
    } else {
        bytes.to_vec()
    };
    let cursor = Cursor::new(archive_bytes);
    let mut zip = ZipArchive::new(cursor).map_err(|e| format!("open extension archive: {e}"))?;
    let Some(manifest) = read_zip_text(&mut zip, "manifest.json") else {
        if package_extension.eq_ignore_ascii_case("zip") {
            let roots = discover_nested_extension_roots(&mut zip)?;
            if roots.is_empty() {
                return Err("extension package manifest.json not found".to_string());
            }
            let mut batch = Vec::new();
            for root in roots {
                let nested_bytes = repackage_nested_extension(bytes, &root)?;
                let nested_file_name = package_display_name(&format!("{root}.zip"), "zip");
                let mut metadata = read_extension_archive_metadata_from_bytes(
                    &nested_bytes,
                    &nested_file_name,
                    store_url,
                )?;
                metadata.package_bytes = Some(nested_bytes);
                metadata.package_extension = Some("zip".to_string());
                metadata.package_file_name = Some(nested_file_name);
                batch.push(metadata);
            }
            return Ok(batch);
        }
        return Err("extension package manifest.json not found".to_string());
    };
    let manifest_json = serde_json::from_str::<serde_json::Value>(&manifest)
        .map_err(|e| format!("parse manifest: {e}"))?;

    let display_name = manifest_localized_string(
        &mut zip,
        &manifest_json,
        manifest_json.get("name").and_then(|value| value.as_str()),
    );
    let version = manifest_json
        .get("version")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let engine_scope = manifest_engine_scope(&manifest_json, file_name, store_url);
    let logo_url = manifest_logo_data_url(&mut zip, &manifest_json);

    Ok(vec![DerivedExtensionMetadata {
        stable_id: manifest_stable_id(&manifest_json).or_else(|| store_url_fallback_id(store_url)),
        display_name,
        version,
        engine_scope,
        logo_url,
        package_bytes: Some(bytes.to_vec()),
        package_extension: Some(package_extension.clone()),
        package_file_name: Some(package_display_name(file_name, &package_extension)),
    }])
}

fn extract_embedded_zip_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let signature = b"PK\x03\x04";
    let Some(offset) = bytes
        .windows(signature.len())
        .position(|window| window == signature)
    else {
        return Err("embedded zip payload not found in CRX package".to_string());
    };
    Ok(bytes[offset..].to_vec())
}

fn read_zip_text<R: Read + std::io::Seek>(zip: &mut ZipArchive<R>, name: &str) -> Option<String> {
    let mut file = zip.by_name(name).ok()?;
    let mut out = String::new();
    file.read_to_string(&mut out).ok()?;
    Some(out)
}

fn discover_nested_extension_roots<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
) -> Result<Vec<String>, String> {
    let mut roots = BTreeSet::new();
    for index in 0..zip.len() {
        let entry_name = zip
            .by_index(index)
            .map_err(|e| format!("scan extension archive: {e}"))?
            .name()
            .replace('\\', "/");
        if !entry_name.ends_with("/manifest.json") {
            continue;
        }
        let root = entry_name.trim_end_matches("/manifest.json");
        if root.is_empty() {
            continue;
        }
        let segments = root
            .split('/')
            .filter(|segment| !segment.trim().is_empty())
            .collect::<Vec<_>>();
        if segments.len() == 1 {
            roots.insert(segments[0].to_string());
        }
    }
    Ok(roots.into_iter().collect())
}

fn discover_extension_directory_roots(path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.join("manifest.json").is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut roots = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current)
            .map_err(|error| format!("read extension directory {}: {error}", current.display()))?;
        for entry in entries {
            let entry = entry
                .map_err(|error| format!("read extension directory entry: {error}"))?;
            let child = entry.path();
            if !child.is_dir() {
                continue;
            }
            if child.join("manifest.json").is_file() {
                roots.push(child);
            } else {
                stack.push(child);
            }
        }
    }

    roots.sort();
    roots.dedup();
    Ok(roots)
}

fn package_extension_directory(path: &Path) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::<u8>::new());
    let mut writer = ZipWriter::new(&mut output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_directory_to_zip(path, path, &mut writer, options)?;
    writer
        .finish()
        .map_err(|error| format!("finalize extension directory archive: {error}"))?;
    Ok(output.into_inner())
}

fn write_directory_to_zip(
    root: &Path,
    current: &Path,
    writer: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
    options: SimpleFileOptions,
) -> Result<(), String> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| format!("read extension directory {}: {error}", current.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read extension directory entries: {error}"))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("strip extension directory prefix: {error}"))?
            .to_string_lossy()
            .replace('\\', "/");
        if relative.is_empty() {
            continue;
        }
        if path.is_dir() {
            writer
                .add_directory(format!("{relative}/"), options)
                .map_err(|error| format!("write extension directory entry: {error}"))?;
            write_directory_to_zip(root, &path, writer, options)?;
            continue;
        }
        writer
            .start_file(relative, options)
            .map_err(|error| format!("write extension file header: {error}"))?;
        let bytes =
            fs::read(&path).map_err(|error| format!("read extension file {}: {error}", path.display()))?;
        writer
            .write_all(&bytes)
            .map_err(|error| format!("write extension file {}: {error}", path.display()))?;
    }
    Ok(())
}

fn repackage_nested_extension(bytes: &[u8], root: &str) -> Result<Vec<u8>, String> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut source = ZipArchive::new(cursor).map_err(|e| format!("open extension archive: {e}"))?;
    let mut output = Cursor::new(Vec::<u8>::new());
    let mut writer = ZipWriter::new(&mut output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let prefix = format!("{root}/");

    for index in 0..source.len() {
        let mut file = source
            .by_index(index)
            .map_err(|e| format!("read nested extension archive entry: {e}"))?;
        let entry_name = file.name().replace('\\', "/");
        if !entry_name.starts_with(&prefix) {
            continue;
        }
        let relative = entry_name[prefix.len()..].to_string();
        if relative.is_empty() {
            continue;
        }
        if file.is_dir() {
            writer
                .add_directory(relative, options)
                .map_err(|e| format!("write nested extension directory: {e}"))?;
            continue;
        }
        writer
            .start_file(relative, options)
            .map_err(|e| format!("write nested extension file header: {e}"))?;
        let mut file_bytes = Vec::new();
        file.read_to_end(&mut file_bytes)
            .map_err(|e| format!("read nested extension file: {e}"))?;
        writer
            .write_all(&file_bytes)
            .map_err(|e| format!("write nested extension file: {e}"))?;
    }

    writer
        .finish()
        .map_err(|e| format!("finalize nested extension archive: {e}"))?;
    Ok(output.into_inner())
}

fn manifest_engine_scope(
    manifest_json: &serde_json::Value,
    file_name: &str,
    store_url: Option<&str>,
) -> Option<String> {
    if manifest_json
        .get("browser_specific_settings")
        .and_then(|value| value.get("gecko"))
        .is_some()
        || manifest_json
            .get("applications")
            .and_then(|value| value.get("gecko"))
            .is_some()
    {
        return Some("firefox".to_string());
    }
    if manifest_json.get("minimum_chrome_version").is_some() {
        return Some("chromium".to_string());
    }
    let lower = file_name.to_lowercase();
    Some(if lower.ends_with(".xpi") {
        "firefox".to_string()
    } else if lower.ends_with(".crx") {
        "chromium".to_string()
    } else if let Some(url) = store_url {
        infer_engine_scope(Some(url), file_name)
    } else {
        "chromium/firefox".to_string()
    })
}

fn manifest_logo_data_url<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    manifest_json: &serde_json::Value,
) -> Option<String> {
    let icons = manifest_json.get("icons")?.as_object()?;
    let icon_path = icons
        .iter()
        .filter_map(|(size, value)| Some((size.parse::<u32>().ok()?, value.as_str()?)))
        .max_by_key(|(size, _)| *size)
        .map(|(_, value)| value.to_string())?;
    let mut file = zip.by_name(&icon_path).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let icon_path_lower = icon_path.to_lowercase();
    let mime = if icon_path_lower.ends_with(".svg") {
        "image/svg+xml"
    } else if icon_path_lower.ends_with(".jpg") || icon_path_lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if icon_path_lower.ends_with(".webp") {
        "image/webp"
    } else {
        "image/png"
    };
    Some(format!(
        "data:{mime};base64,{}",
        BASE64_STANDARD.encode(bytes)
    ))
}

fn manifest_localized_string<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    manifest_json: &serde_json::Value,
    raw_value: Option<&str>,
) -> Option<String> {
    let value = raw_value?;
    if !value.starts_with("__MSG_") {
        return Some(value.to_string());
    }
    let key = value
        .strip_prefix("__MSG_")
        .and_then(|item| item.strip_suffix("__"))?;
    for locale in locale_candidates(
        manifest_json
            .get("default_locale")
            .and_then(|item| item.as_str()),
    ) {
        let path = format!("_locales/{locale}/messages.json");
        let Some(raw_messages) = read_zip_text(zip, &path) else {
            continue;
        };
        let json = serde_json::from_str::<serde_json::Value>(&raw_messages).ok()?;
        if let Some(message) = json
            .get(key)
            .and_then(|item| item.get("message"))
            .and_then(|item| item.as_str())
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
        {
            return Some(message);
        }
    }
    None
}

fn manifest_stable_id(manifest_json: &serde_json::Value) -> Option<String> {
    manifest_json
        .get("browser_specific_settings")
        .and_then(|value| value.get("gecko"))
        .and_then(|value| value.get("id"))
        .and_then(|value| value.as_str())
        .or_else(|| {
            manifest_json
                .get("applications")
                .and_then(|value| value.get("gecko"))
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
        })
        .map(|value| value.to_string())
}

fn locale_candidates(default_locale: Option<&str>) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(locale) = default_locale
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        candidates.push(locale.to_string());
        candidates.push(locale.replace('-', "_"));
        candidates.push(locale.replace('_', "-"));
        if let Some((language, _)) = locale.split_once(['-', '_']) {
            candidates.push(language.to_string());
        }
    }
    candidates.push("en".to_string());
    candidates.push("en_US".to_string());
    candidates.push("en-US".to_string());
    candidates.sort();
    candidates.dedup();
    candidates
}

fn infer_package_extension(file_name: &str, store_url: Option<&str>) -> String {
    let lower = file_name.to_lowercase();
    if lower.ends_with(".xpi") {
        return "xpi".to_string();
    }
    if lower.ends_with(".crx") {
        return "crx".to_string();
    }
    if lower.ends_with(".zip") {
        return "zip".to_string();
    }
    if let Some(url) = store_url {
        if url.to_lowercase().contains("addons.mozilla.org") {
            return "xpi".to_string();
        }
        if url.to_lowercase().contains("chromewebstore.google.com")
            || url.to_lowercase().contains("chrome.google.com")
        {
            return "crx".to_string();
        }
    }
    "zip".to_string()
}

fn package_display_name(file_name: &str, package_extension: &str) -> String {
    let fallback = format!("extension.{package_extension}");
    Path::new(file_name)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
}

fn store_url_fallback_id(store_url: Option<&str>) -> Option<String> {
    let url = store_url?;
    extract_chrome_web_store_id(url)
        .or_else(|| extract_amo_slug(url))
        .filter(|value| !value.trim().is_empty())
}

fn download_store_extension_metadata(store_url: &str) -> Result<DerivedExtensionMetadata, String> {
    let lower = store_url.to_lowercase();
    if lower.contains("addons.mozilla.org") {
        return download_amo_extension_metadata(store_url);
    }
    if lower.contains("chromewebstore.google.com") || lower.contains("chrome.google.com") {
        return download_chrome_extension_metadata(store_url);
    }
    Err("unsupported store URL".to_string())
}

fn download_amo_extension_metadata(store_url: &str) -> Result<DerivedExtensionMetadata, String> {
    let client = extension_http_client()?;
    let slug = extract_amo_slug(store_url).ok_or_else(|| "unsupported AMO URL".to_string())?;
    let api_url = format!("https://addons.mozilla.org/api/v5/addons/addon/{slug}/");
    let details = download_json(&client, &api_url)?;
    let file_url = details
        .get("current_version")
        .and_then(|value| value.get("file"))
        .and_then(|value| value.get("url"))
        .and_then(|value| value.as_str())
        .or_else(|| {
            details
                .get("current_version")
                .and_then(|value| value.get("files"))
                .and_then(|value| value.as_array())
                .and_then(|items| items.first())
                .and_then(|value| value.get("url"))
                .and_then(|value| value.as_str())
        })
        .ok_or_else(|| "AMO package download URL not found".to_string())?;
    let file_name = file_name_from_url(file_url).unwrap_or_else(|| format!("{slug}.xpi"));
    let package_bytes = download_binary(&client, file_url)?;
    let mut metadata =
        read_extension_archive_metadata_from_bytes(&package_bytes, &file_name, Some(store_url))?;
    metadata.stable_id = Some(slug);
    metadata.engine_scope = Some("firefox".to_string());
    if metadata.display_name.is_none() {
        metadata.display_name = localized_json_value(details.get("name"));
    }
    if metadata.version.is_none() {
        metadata.version = details
            .get("current_version")
            .and_then(|value| value.get("version"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());
    }
    if metadata.logo_url.is_none() {
        metadata.logo_url = amo_icon_data_url(&client, &details).or_else(|| {
            details
                .get("icon_url")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
    }
    Ok(metadata)
}

fn download_chrome_extension_metadata(store_url: &str) -> Result<DerivedExtensionMetadata, String> {
    let client = extension_http_client()?;
    let extension_id = extract_chrome_web_store_id(store_url)
        .ok_or_else(|| "unsupported Chrome Web Store URL".to_string())?;
    let file_name = format!("{extension_id}.crx");
    let package_bytes =
        download_binary(&client, &build_chrome_web_store_download_url(&extension_id))?;
    let mut metadata =
        read_extension_archive_metadata_from_bytes(&package_bytes, &file_name, Some(store_url))?;
    metadata.stable_id = Some(extension_id);
    metadata.engine_scope = Some("chromium".to_string());
    if metadata.display_name.is_none() || metadata.logo_url.is_none() {
        if let Ok(page_html) = download_text(&client, store_url) {
            if metadata.display_name.is_none() {
                metadata.display_name = parse_html_meta_content(&page_html, "og:title")
                    .or_else(|| parse_html_title(&page_html));
            }
            if metadata.logo_url.is_none() {
                metadata.logo_url = parse_html_meta_content(&page_html, "og:image")
                    .and_then(|url| download_data_url(&client, &url).or(Some(url)));
            }
        }
    }
    Ok(metadata)
}

fn extension_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Cerbena/"
                .to_string()
                + env!("CARGO_PKG_VERSION"),
        )
        .build()
        .map_err(|e| format!("extension http client: {e}"))
}

fn download_binary(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("download {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("download {url}: http {}", response.status()));
    }
    response
        .bytes()
        .map(|value| value.to_vec())
        .map_err(|e| e.to_string())
}

fn download_text(client: &Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("download {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("download {url}: http {}", response.status()));
    }
    response.text().map_err(|e| format!("download {url}: {e}"))
}

fn download_json(client: &Client, url: &str) -> Result<serde_json::Value, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("download {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("download {url}: http {}", response.status()));
    }
    response
        .json::<serde_json::Value>()
        .map_err(|e| format!("parse {url}: {e}"))
}

fn download_data_url(client: &Client, url: &str) -> Option<String> {
    let bytes = download_binary(client, url).ok()?;
    let mime = guess_mime_from_url(url);
    Some(format!(
        "data:{mime};base64,{}",
        BASE64_STANDARD.encode(bytes)
    ))
}

fn guess_mime_from_url(url: &str) -> &'static str {
    let lower = url.to_lowercase();
    if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/png"
    }
}

fn localized_json_value(value: Option<&serde_json::Value>) -> Option<String> {
    match value {
        Some(serde_json::Value::String(text)) => {
            Some(text.trim().to_string()).filter(|item| !item.is_empty())
        }
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("en-US") || key.eq_ignore_ascii_case("en_us"))
            .and_then(|(_, value)| value.as_str())
            .or_else(|| map.values().find_map(|value| value.as_str()))
            .map(|value| value.trim().to_string())
            .filter(|item| !item.is_empty()),
        _ => None,
    }
}

fn amo_icon_data_url(client: &Client, details: &serde_json::Value) -> Option<String> {
    details
        .get("icon_url")
        .and_then(|value| value.as_str())
        .and_then(|url| download_data_url(client, url))
        .or_else(|| {
            details
                .get("icons")
                .and_then(|value| value.as_object())
                .and_then(|icons| {
                    icons
                        .iter()
                        .filter_map(|(size, value)| {
                            Some((size.parse::<u32>().ok()?, value.as_str()?))
                        })
                        .max_by_key(|(size, _)| *size)
                        .map(|(_, url)| url.to_string())
                })
                .and_then(|url| download_data_url(client, &url))
        })
}

fn extract_amo_slug(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let segments = parsed
        .path_segments()?
        .filter(|segment| !segment.trim().is_empty())
        .collect::<Vec<_>>();
    let addon_index = segments.iter().position(|segment| *segment == "addon")?;
    segments
        .get(addon_index + 1)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn extract_chrome_web_store_id(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let segments = parsed
        .path_segments()?
        .filter(|segment| !segment.trim().is_empty())
        .collect::<Vec<_>>();
    let detail_index = segments.iter().position(|segment| *segment == "detail")?;
    segments
        .get(detail_index + 2)
        .map(|value| value.trim().to_string())
        .filter(|value| value.len() >= 16)
}

fn build_chrome_web_store_download_url(extension_id: &str) -> String {
    format!(
        "https://clients2.google.com/service/update2/crx?response=redirect&prodversion=131.0.6778.86&acceptformat=crx2,crx3&x=id%3D{extension_id}%26installsource%3Dondemand%26uc"
    )
}

fn file_name_from_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    parsed
        .path_segments()?
        .filter(|segment| !segment.trim().is_empty())
        .next_back()
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
}

fn parse_html_meta_content(html: &str, marker: &str) -> Option<String> {
    for attr in ["property", "name"] {
        for quote in ['"', '\''] {
            let needle = format!("{attr}={quote}{marker}{quote}");
            let Some(index) = html.find(&needle) else {
                continue;
            };
            let tag_start = html[..index].rfind("<meta").unwrap_or(index);
            let tag_end = html[index..]
                .find('>')
                .map(|offset| index + offset)
                .unwrap_or(html.len());
            let fragment = &html[tag_start..tag_end];
            if let Some(content) = extract_html_attribute(fragment, "content") {
                return Some(html_entity_decode(&content));
            }
        }
    }
    None
}

fn parse_html_title(html: &str) -> Option<String> {
    let start = html.find("<title>")? + "<title>".len();
    let end = html[start..]
        .find("</title>")
        .map(|offset| start + offset)?;
    let title = html_entity_decode(html[start..end].trim());
    Some(
        title
            .trim_end_matches(" - Chrome Web Store")
            .trim_end_matches(" – Get this Extension for Firefox")
            .trim()
            .to_string(),
    )
    .filter(|value| !value.is_empty())
}

fn extract_html_attribute(fragment: &str, attr: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{attr}={quote}");
        let Some(start) = fragment.find(&needle).map(|value| value + needle.len()) else {
            continue;
        };
        let end = fragment[start..].find(quote).map(|offset| start + offset)?;
        return Some(fragment[start..end].to_string());
    }
    None
}

fn html_entity_decode(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn persist_extension_package(
    state: &AppState,
    extension_id: &str,
    package_bytes: Option<&[u8]>,
    package_extension: Option<&str>,
    package_file_name: Option<&str>,
) -> Result<(Option<String>, Option<String>), String> {
    let Some(bytes) = package_bytes else {
        return Ok((None, None));
    };
    let extension = package_extension
        .map(|value| value.trim().trim_start_matches('.').to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "zip".to_string());
    let root = state.extension_packages_root(&state.app_handle)?;
    fs::create_dir_all(&root).map_err(|e| format!("create extension package dir: {e}"))?;
    let package_path = root.join(format!("{extension_id}.{extension}"));
    fs::write(&package_path, bytes).map_err(|e| format!("write extension package: {e}"))?;
    Ok((
        Some(package_path.to_string_lossy().to_string()),
        Some(
            package_file_name
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(&format!("{extension_id}.{extension}"))
                .to_string(),
        ),
    ))
}

fn delete_extension_package(package_path: Option<&str>) {
    let Some(path) = package_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let _ = fs::remove_file(PathBuf::from(path));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn build_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut output = Cursor::new(Vec::<u8>::new());
        let mut writer = ZipWriter::new(&mut output);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, body) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        output.into_inner()
    }

    #[test]
    fn reads_single_extension_archive_metadata() {
        let archive = build_zip(&[(
            "manifest.json",
            r#"{"name":"Single","version":"1.2.3","minimum_chrome_version":"120"}"#,
        )]);
        let batch =
            read_extension_archive_metadata_batch_from_bytes(&archive, "single.zip", None).unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].display_name.as_deref(), Some("Single"));
        assert_eq!(batch[0].version.as_deref(), Some("1.2.3"));
        assert_eq!(batch[0].engine_scope.as_deref(), Some("chromium"));
    }

    #[test]
    fn reads_multi_extension_archive_metadata_from_nested_folders() {
        let archive = build_zip(&[
            (
                "alpha/manifest.json",
                r#"{"name":"Alpha","version":"1.0.0","minimum_chrome_version":"120"}"#,
            ),
            (
                "beta/manifest.json",
                r#"{"name":"Beta","version":"2.0.0","browser_specific_settings":{"gecko":{"id":"beta@example.com"}}}"#,
            ),
        ]);
        let batch =
            read_extension_archive_metadata_batch_from_bytes(&archive, "bundle.zip", None).unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].display_name.as_deref(), Some("Alpha"));
        assert_eq!(batch[1].display_name.as_deref(), Some("Beta"));
        assert_eq!(batch[0].package_file_name.as_deref(), Some("alpha.zip"));
        assert_eq!(batch[1].package_file_name.as_deref(), Some("beta.zip"));
    }

    #[test]
    fn reads_extension_directory_metadata_for_firefox_folder() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(
            temp.path().join("manifest.json"),
            r#"{"name":"Folder Fox","version":"3.1.0","browser_specific_settings":{"gecko":{"id":"folder-fox@example.com"}}}"#,
        )
        .expect("write manifest");
        fs::create_dir_all(temp.path().join("icons")).expect("create icons");
        fs::write(temp.path().join("icons").join("icon.png"), b"png").expect("write icon");

        let batch = read_extension_directory_metadata_batch(temp.path(), None).expect("read folder");
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].display_name.as_deref(), Some("Folder Fox"));
        assert_eq!(batch[0].engine_scope.as_deref(), Some("firefox"));
        assert_eq!(batch[0].package_extension.as_deref(), Some("xpi"));
        assert!(batch[0].package_bytes.as_ref().is_some_and(|value| !value.is_empty()));
    }

    #[test]
    fn reads_nested_extension_directories_when_root_has_no_manifest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let alpha = temp.path().join("alpha");
        let beta = temp.path().join("beta");
        fs::create_dir_all(&alpha).expect("create alpha");
        fs::create_dir_all(&beta).expect("create beta");
        fs::write(
            alpha.join("manifest.json"),
            r#"{"name":"Alpha Dir","version":"1.0.0","minimum_chrome_version":"120"}"#,
        )
        .expect("write alpha manifest");
        fs::write(
            beta.join("manifest.json"),
            r#"{"name":"Beta Dir","version":"2.0.0","browser_specific_settings":{"gecko":{"id":"beta-dir@example.com"}}}"#,
        )
        .expect("write beta manifest");

        let batch = read_extension_directory_metadata_batch(temp.path(), None).expect("read nested folders");
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].display_name.as_deref(), Some("Alpha Dir"));
        assert_eq!(batch[0].package_extension.as_deref(), Some("zip"));
        assert_eq!(batch[1].display_name.as_deref(), Some("Beta Dir"));
        assert_eq!(batch[1].package_extension.as_deref(), Some("xpi"));
    }
}
