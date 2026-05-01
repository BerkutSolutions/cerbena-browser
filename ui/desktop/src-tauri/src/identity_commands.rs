use browser_fingerprint::{
    apply_auto_geolocation, build_identity_preview, generate_auto_preset, validate_identity_preset,
    validate_identity_tab_save, AutoPlatform, GeoSource, IdentityPreset,
};
use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::{
    envelope::{ok, UiEnvelope},
    state::{persist_identity_store, AppState},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentitySaveRequest {
    pub profile_id: String,
    pub preset: IdentityPreset,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityGetRequest {
    pub profile_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoPresetRequest {
    pub platform: String,
    pub seed: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoAutoApplyRequest {
    pub preset: IdentityPreset,
    pub source: GeoSourceInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoSourceInput {
    pub timezone_iana: String,
    pub timezone_offset_minutes: i16,
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_meters: f32,
    pub language: String,
}

#[tauri::command]
pub fn generate_identity_auto_preset(
    request: AutoPresetRequest,
    correlation_id: String,
) -> Result<UiEnvelope<IdentityPreset>, String> {
    let platform = match request.platform.as_str() {
        "windows" => AutoPlatform::Windows,
        "windows8" => AutoPlatform::Windows8,
        "macos" => AutoPlatform::Macos,
        "linux" => AutoPlatform::Linux,
        "debian" => AutoPlatform::Debian,
        "ubuntu" => AutoPlatform::Ubuntu,
        "ios" => AutoPlatform::Ios,
        "android" => AutoPlatform::Android,
        _ => return Err(format!("unsupported platform: {}", request.platform)),
    };
    Ok(ok(
        correlation_id,
        generate_auto_preset(platform, request.seed),
    ))
}

#[tauri::command]
pub fn validate_identity_preset_command(
    preset: IdentityPreset,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    validate_identity_preset(&preset).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn preview_identity_preset(
    preset: IdentityPreset,
    active_route: Option<String>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let preview =
        build_identity_preview(&preset, active_route.as_deref()).map_err(|e| e.to_string())?;
    let payload = serde_json::to_string_pretty(&preview).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, payload))
}

#[tauri::command]
pub fn validate_identity_save(
    preset: IdentityPreset,
    active_route: Option<String>,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let outcome = validate_identity_tab_save(&preset, active_route.as_deref());
    let payload = serde_json::to_string_pretty(&outcome).map_err(|e| e.to_string())?;
    Ok(ok(correlation_id, payload))
}

#[tauri::command]
pub fn save_identity_profile(
    app: AppHandle,
    state: State<AppState>,
    request: IdentitySaveRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    validate_identity_preset(&request.preset).map_err(|e| e.to_string())?;
    let mut guard = state
        .identity_store
        .lock()
        .map_err(|_| "identity lock poisoned".to_string())?;
    guard.items.insert(request.profile_id, request.preset);
    let store_path = state.identity_store_path(&app)?;
    persist_identity_store(&store_path, &guard)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn get_identity_profile(
    state: State<AppState>,
    request: IdentityGetRequest,
    correlation_id: String,
) -> Result<UiEnvelope<Option<IdentityPreset>>, String> {
    let guard = state
        .identity_store
        .lock()
        .map_err(|_| "identity lock poisoned".to_string())?;
    let found = guard.items.get(&request.profile_id).cloned();
    Ok(ok(correlation_id, found))
}

#[tauri::command]
pub fn apply_identity_auto_geolocation(
    request: GeoAutoApplyRequest,
    correlation_id: String,
) -> Result<UiEnvelope<IdentityPreset>, String> {
    let mut preset = request.preset;
    let source = GeoSource {
        timezone_iana: request.source.timezone_iana,
        timezone_offset_minutes: request.source.timezone_offset_minutes,
        latitude: request.source.latitude,
        longitude: request.source.longitude,
        accuracy_meters: request.source.accuracy_meters,
        language: request.source.language,
    };
    apply_auto_geolocation(&mut preset, &source);
    Ok(ok(correlation_id, preset))
}
