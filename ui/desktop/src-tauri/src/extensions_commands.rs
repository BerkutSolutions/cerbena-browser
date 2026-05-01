use crate::{
    envelope::{ok, UiEnvelope},
    state::{
        persist_extension_library_store, AppState, ExtensionLibraryItem, ExtensionLibraryStore,
    },
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use browser_extensions::{ExtensionPolicyEnforcer, OverrideGuardrails};
use browser_network_policy::{NetworkPolicy, PolicyRequest, RouteMode};
use browser_profile::Engine;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::State;
use zip::ZipArchive;

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
    pub assigned_profile_ids: Vec<String>,
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
}

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
    let json = serde_json::to_string_pretty(&*library).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, json))
}

#[tauri::command]
pub fn import_extension_library_item(
    state: State<AppState>,
    request: ImportExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let normalized_store_url = request
        .store_url
        .clone()
        .filter(|value| !value.trim().is_empty());
    let package_metadata = derive_extension_metadata(&request, normalized_store_url.as_deref())?;
    let id = build_extension_id(
        request
            .display_name
            .as_deref()
            .or(package_metadata.stable_id.as_deref())
            .or(package_metadata.display_name.as_deref())
            .or(normalized_store_url.as_deref())
            .unwrap_or(&request.source_value),
        &library,
    );
    let inferred_name = request
        .display_name
        .filter(|value| !value.trim().is_empty())
        .or(package_metadata.display_name)
        .unwrap_or_else(|| {
            infer_extension_name(normalized_store_url.as_deref(), &request.source_value)
        });
    let inferred_engine = request
        .engine_scope
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or(package_metadata.engine_scope)
        .unwrap_or_else(|| {
            infer_engine_scope(normalized_store_url.as_deref(), &request.source_value)
        });
    validate_assigned_profiles(&state, &inferred_engine, &request.assigned_profile_ids)?;
    let (package_path, package_file_name) = persist_extension_package(
        &state,
        &id,
        package_metadata.package_bytes.as_deref(),
        package_metadata.package_extension.as_deref(),
        package_metadata.package_file_name.as_deref(),
    )?;
    let item = ExtensionLibraryItem {
        id: id.clone(),
        display_name: inferred_name,
        version: request
            .version
            .filter(|value| !value.trim().is_empty())
            .or(package_metadata.version)
            .unwrap_or_else(|| "1.0.1".to_string()),
        engine_scope: inferred_engine,
        source_kind: request.source_kind,
        source_value: request.source_value,
        logo_url: request
            .logo_url
            .filter(|value| !value.trim().is_empty())
            .or(package_metadata.logo_url),
        store_url: normalized_store_url,
        assigned_profile_ids: request.assigned_profile_ids,
        package_path,
        package_file_name,
    };
    library.items.insert(id.clone(), item);
    persist_library(&state, &library)?;
    Ok(ok(correlation_id, id))
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
    persist_library(&state, &library)?;
    Ok(ok(correlation_id, true))
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
    let removed = library.items.remove(&request.extension_id);
    persist_library(&state, &library)?;
    if let Some(item) = removed {
        delete_extension_package(item.package_path.as_deref());
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

fn persist_library(state: &AppState, store: &ExtensionLibraryStore) -> Result<(), String> {
    let path = state.extension_library_path(&state.app_handle)?;
    persist_extension_library_store(&path, store)
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
        "firefox" => matches!(engine, Engine::Camoufox),
        "chromium" => matches!(engine, Engine::Wayfern),
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

fn derive_extension_metadata(
    request: &ImportExtensionLibraryRequest,
    store_url: Option<&str>,
) -> Result<DerivedExtensionMetadata, String> {
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
        return read_extension_archive_metadata_from_bytes(&bytes, file_name, store_url);
    }

    let lower_kind = request.source_kind.to_lowercase();
    if lower_kind == "store_url" {
        return download_store_extension_metadata(
            store_url.unwrap_or(request.source_value.as_str()),
        );
    }

    if matches!(lower_kind.as_str(), "local_file" | "dropped_file") {
        let path = Path::new(&request.source_value);
        if path.exists() {
            return read_extension_archive_metadata(path, store_url);
        }
    }

    Ok(DerivedExtensionMetadata {
        stable_id: store_url_fallback_id(store_url),
        engine_scope: Some(infer_engine_scope(store_url, &request.source_value)),
        ..DerivedExtensionMetadata::default()
    })
}

fn read_extension_archive_metadata(
    path: &Path,
    store_url: Option<&str>,
) -> Result<DerivedExtensionMetadata, String> {
    let bytes = fs::read(path).map_err(|e| format!("read extension package: {e}"))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("extension.zip");
    read_extension_archive_metadata_from_bytes(&bytes, file_name, store_url)
}

fn read_extension_archive_metadata_from_bytes(
    bytes: &[u8],
    file_name: &str,
    store_url: Option<&str>,
) -> Result<DerivedExtensionMetadata, String> {
    let package_extension = infer_package_extension(file_name, store_url);
    let archive_bytes = if package_extension.eq_ignore_ascii_case("crx") {
        extract_embedded_zip_bytes(bytes)?
    } else {
        bytes.to_vec()
    };
    let cursor = Cursor::new(archive_bytes);
    let mut zip = ZipArchive::new(cursor).map_err(|e| format!("open extension archive: {e}"))?;
    let manifest = read_zip_text(&mut zip, "manifest.json")
        .ok_or_else(|| "extension package manifest.json not found".to_string())?;
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

    Ok(DerivedExtensionMetadata {
        stable_id: manifest_stable_id(&manifest_json).or_else(|| store_url_fallback_id(store_url)),
        display_name,
        version,
        engine_scope,
        logo_url,
        package_bytes: Some(bytes.to_vec()),
        package_extension: Some(package_extension.clone()),
        package_file_name: Some(package_display_name(file_name, &package_extension)),
    })
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
        metadata.logo_url = amo_icon_data_url(&client, &details)
            .or_else(|| details.get("icon_url").and_then(|value| value.as_str()).map(str::to_string));
    }
    Ok(metadata)
}

fn download_chrome_extension_metadata(
    store_url: &str,
) -> Result<DerivedExtensionMetadata, String> {
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
             (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Cerbena/1.0.1",
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
    response.bytes().map(|value| value.to_vec()).map_err(|e| e.to_string())
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
    let end = html[start..].find("</title>").map(|offset| start + offset)?;
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
