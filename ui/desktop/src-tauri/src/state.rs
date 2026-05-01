use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use browser_api_local::{
    HomePageService, LaunchHookService, PanicWipeService, PipPolicyService, SearchProviderRegistry,
    SecurityGuardrails,
};
use browser_fingerprint::IdentityPreset;
use browser_network_policy::{DnsTabPayload, ServiceCatalog, VpnProxyTabPayload};
use browser_profile::{CreateProfileInput, Engine, ProfileManager};
use browser_sync_client::{BackupSnapshot, ConflictViewItem, SnapshotManager, SyncControlsModel};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
#[cfg(not(target_os = "windows"))]
use tauri::Manager;
use uuid::Uuid;

use crate::device_posture::{load_device_posture_store, DevicePostureStore};
use crate::launch_sessions::{load_launch_session_store, LaunchSessionStore};
use crate::route_runtime::RouteRuntimeState;
use crate::sensitive_store::derive_app_secret_material;
use crate::service_catalog_seed::build_service_catalog;
use crate::traffic_gateway::{load_rules_store, load_traffic_log, TrafficGatewayState};
use crate::update_commands::{AppUpdateStore, UpdaterLaunchMode, UpdaterRuntimeState};

pub(crate) fn app_local_data_root(app: &AppHandle) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .ok_or_else(|| "LOCALAPPDATA is not set".to_string())?;
        let folder_name = if cfg!(debug_assertions) {
            "dev.browser.launcher"
        } else {
            "Cerbena Browser"
        };
        let _ = app;
        return Ok(PathBuf::from(local_app_data).join(folder_name));
    }

    #[cfg(not(target_os = "windows"))]
    {
        app.path()
            .app_local_data_dir()
            .map_err(|e| format!("app_local_data_dir: {e}"))
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct IdentityStore {
    pub items: BTreeMap<String, IdentityPreset>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStore {
    pub vpn_proxy: BTreeMap<String, VpnProxyTabPayload>,
    pub dns: BTreeMap<String, DnsTabPayload>,
    pub connection_templates: BTreeMap<String, ConnectionTemplate>,
    pub profile_template_selection: BTreeMap<String, String>,
    #[serde(default)]
    pub global_route_settings: NetworkGlobalRouteSettings,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkGlobalRouteSettings {
    pub global_vpn_enabled: bool,
    pub block_without_vpn: bool,
    pub default_template_id: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SyncStore {
    pub controls: BTreeMap<String, SyncControlsModel>,
    pub conflicts: BTreeMap<String, Vec<ConflictViewItem>>,
    pub snapshots: BTreeMap<String, Vec<BackupSnapshot>>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRoutingStore {
    pub global_profile_id: Option<String>,
    #[serde(default)]
    pub type_bindings: BTreeMap<String, String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionLibraryStore {
    #[serde(default)]
    pub auto_update_enabled: bool,
    pub items: BTreeMap<String, ExtensionLibraryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionNode {
    pub id: String,
    pub connection_type: String,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub bridges: Option<String>,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub nodes: Vec<ConnectionNode>,
    #[serde(default)]
    pub connection_type: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub bridges: Option<String>,
    pub updated_at_epoch_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionLibraryItem {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub engine_scope: String,
    pub source_kind: String,
    pub source_value: String,
    pub logo_url: Option<String>,
    pub store_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub assigned_profile_ids: Vec<String>,
    #[serde(default)]
    pub auto_update_enabled: bool,
    #[serde(default)]
    pub preserve_on_panic_wipe: bool,
    #[serde(default)]
    pub protect_data_from_panic_wipe: bool,
    #[serde(default)]
    pub package_path: Option<String>,
    #[serde(default)]
    pub package_file_name: Option<String>,
}

pub struct AppState {
    pub app_handle: AppHandle,
    pub profile_root: PathBuf,
    pub engine_runtime_root: PathBuf,
    pub network_runtime_root: PathBuf,
    pub sensitive_store_secret: String,
    pub manager: Mutex<ProfileManager>,
    pub identity_store: Mutex<IdentityStore>,
    pub network_store: Mutex<NetworkStore>,
    pub service_catalog: Mutex<ServiceCatalog>,
    pub sync_store: Mutex<SyncStore>,
    pub link_routing_store: Mutex<LinkRoutingStore>,
    pub launch_session_store: Mutex<LaunchSessionStore>,
    pub extension_library: Mutex<ExtensionLibraryStore>,
    pub snapshot_manager: Mutex<SnapshotManager>,
    pub home_service: Mutex<HomePageService>,
    pub panic_service: Mutex<PanicWipeService>,
    pub launch_hook_service: Mutex<LaunchHookService>,
    pub pip_service: Mutex<PipPolicyService>,
    pub search_registry: Mutex<SearchProviderRegistry>,
    pub security_guardrails: Mutex<SecurityGuardrails>,
    pub device_posture_store: Mutex<DevicePostureStore>,
    pub app_update_store: Mutex<AppUpdateStore>,
    pub updater_runtime: Arc<Mutex<UpdaterRuntimeState>>,
    pub runtime_logs: Mutex<Vec<String>>,
    pub pending_external_link: Mutex<Option<String>>,
    pub launched_processes: Mutex<BTreeMap<Uuid, u32>>,
    pub active_panic_frames: Mutex<BTreeSet<Uuid>>,
    pub active_engine_downloads: Mutex<BTreeSet<String>>,
    pub cancelled_engine_downloads: Arc<Mutex<BTreeSet<String>>>,
    pub active_network_downloads: Mutex<BTreeSet<String>>,
    pub route_runtime: Mutex<RouteRuntimeState>,
    pub traffic_gateway: std::sync::Mutex<TrafficGatewayState>,
}

impl AppState {
    pub fn bootstrap(app: &AppHandle) -> Result<Self, String> {
        let app_data = app_local_data_root(app)?;
        let profile_root = app_data.join("profiles");
        let engine_runtime_root = app_data.join("engine-runtime");
        let network_runtime_root = app_data.join("network-runtime");
        fs::create_dir_all(&profile_root).map_err(|e| format!("create profile root: {e}"))?;
        fs::create_dir_all(&engine_runtime_root)
            .map_err(|e| format!("create engine runtime root: {e}"))?;
        fs::create_dir_all(&network_runtime_root)
            .map_err(|e| format!("create network runtime root: {e}"))?;
        let current_exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        let identifier = app.config().identifier.clone();
        let sensitive_store_secret =
            derive_app_secret_material(&app_data, &current_exe, &identifier)
                .map_err(|e| format!("derive sensitive store secret: {e}"))?;

        let manager =
            ProfileManager::new(&profile_root).map_err(|e| format!("manager init: {e}"))?;
        ensure_default_profiles(&manager)?;
        let identity_store = load_identity_store(&app_data.join("identity_store.json"))?;
        let network_store = load_network_store(&app_data.join("network_store.json"))?;
        let extension_library =
            load_extension_library_store(&app_data.join("extension_library.json"))?;
        let sync_store =
            load_sync_store(&app_data.join("sync_store.json"), &sensitive_store_secret)?;
        let link_routing_store = load_link_routing_store(
            &app_data.join("link_routing_store.json"),
            &sensitive_store_secret,
        )?;
        let launch_session_store = load_launch_session_store(
            &app_data.join("launch_session_store.json"),
            &sensitive_store_secret,
        )?;
        let device_posture_store =
            load_device_posture_store(&app_data.join("device_posture_store.json"))?;
        let app_update_store = load_app_update_store(&app_data.join("app_update_store.json"))?;
        let updater_launch_mode = UpdaterLaunchMode::from_args(std::env::args().skip(1));
        let traffic_rules = load_rules_store(&app_data.join("traffic_gateway_rules.json"))?;
        let traffic_log = load_traffic_log(&app_data.join("traffic_gateway_log.json"))?;

        Ok(Self {
            app_handle: app.clone(),
            profile_root,
            engine_runtime_root,
            network_runtime_root,
            sensitive_store_secret,
            manager: Mutex::new(manager),
            identity_store: Mutex::new(identity_store),
            network_store: Mutex::new(network_store),
            service_catalog: Mutex::new(build_service_catalog()),
            sync_store: Mutex::new(sync_store),
            link_routing_store: Mutex::new(link_routing_store),
            launch_session_store: Mutex::new(launch_session_store),
            extension_library: Mutex::new(extension_library),
            snapshot_manager: Mutex::new(SnapshotManager::default()),
            home_service: Mutex::new(HomePageService),
            panic_service: Mutex::new(PanicWipeService),
            launch_hook_service: Mutex::new(LaunchHookService),
            pip_service: Mutex::new(PipPolicyService),
            search_registry: Mutex::new(SearchProviderRegistry::default()),
            security_guardrails: Mutex::new(SecurityGuardrails::default()),
            device_posture_store: Mutex::new(device_posture_store),
            app_update_store: Mutex::new(app_update_store),
            updater_runtime: Arc::new(Mutex::new(UpdaterRuntimeState::new(updater_launch_mode))),
            runtime_logs: Mutex::new(Vec::new()),
            pending_external_link: Mutex::new(None),
            launched_processes: Mutex::new(BTreeMap::new()),
            active_panic_frames: Mutex::new(BTreeSet::new()),
            active_engine_downloads: Mutex::new(BTreeSet::new()),
            cancelled_engine_downloads: Arc::new(Mutex::new(BTreeSet::new())),
            active_network_downloads: Mutex::new(BTreeSet::new()),
            route_runtime: Mutex::new(RouteRuntimeState::default()),
            traffic_gateway: Mutex::new(TrafficGatewayState {
                listeners: BTreeMap::new(),
                traffic_log,
                rules: traffic_rules,
                route_health_cache: BTreeMap::new(),
            }),
        })
    }

    pub fn identity_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("identity_store.json"))
    }
    pub fn traffic_gateway_rules_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("traffic_gateway_rules.json"))
    }

    pub fn traffic_gateway_log_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("traffic_gateway_log.json"))
    }

    pub fn network_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("network_store.json"))
    }

    pub fn extension_library_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("extension_library.json"))
    }

    pub fn extension_packages_root(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("extension-packages"))
    }

    pub fn sync_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("sync_store.json"))
    }

    pub fn link_routing_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("link_routing_store.json"))
    }

    pub fn launch_session_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("launch_session_store.json"))
    }

    pub fn device_posture_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("device_posture_store.json"))
    }

    pub fn app_update_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("app_update_store.json"))
    }

    pub fn app_update_root_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("updates"))
    }

    pub fn global_security_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("global_security_store.json"))
    }

    pub fn global_security_legacy_path(&self) -> PathBuf {
        self.profile_root.join("_global-security.json")
    }
}

fn load_identity_store(path: &PathBuf) -> Result<IdentityStore, String> {
    if !path.exists() {
        return Ok(IdentityStore::default());
    }
    let raw = fs::read(path).map_err(|e| format!("read identity store: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("parse identity store: {e}"))
}

fn load_network_store(path: &PathBuf) -> Result<NetworkStore, String> {
    if !path.exists() {
        return Ok(NetworkStore::default());
    }
    let raw = fs::read(path).map_err(|e| format!("read network store: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("parse network store: {e}"))
}

fn load_extension_library_store(path: &PathBuf) -> Result<ExtensionLibraryStore, String> {
    if !path.exists() {
        return Ok(ExtensionLibraryStore::default());
    }
    let raw = fs::read(path).map_err(|e| format!("read extension library: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("parse extension library: {e}"))
}

fn load_app_update_store(path: &PathBuf) -> Result<AppUpdateStore, String> {
    if !path.exists() {
        return Ok(AppUpdateStore::default());
    }
    let raw = fs::read(path).map_err(|e| format!("read app update store: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("parse app update store: {e}"))
}

fn load_sync_store(path: &PathBuf, secret_material: &str) -> Result<SyncStore, String> {
    crate::sensitive_store::load_sensitive_json_or_default(path, "sync-store", secret_material)
}

fn load_link_routing_store(
    path: &PathBuf,
    secret_material: &str,
) -> Result<LinkRoutingStore, String> {
    crate::sensitive_store::load_sensitive_json_or_default(
        path,
        "link-routing-store",
        secret_material,
    )
}

pub(crate) fn ensure_default_profiles(manager: &ProfileManager) -> Result<(), String> {
    let existing = manager.list_profiles().map_err(|e| e.to_string())?;
    let has = |name: &str| existing.iter().any(|p| p.name == name);
    if !has("Chromium Default") {
        manager
            .create_profile(CreateProfileInput {
                name: "Chromium Default".to_string(),
                description: Some("Default isolated Chromium profile (Wayfern).".to_string()),
                tags: vec!["default".to_string(), "engine:wayfern".to_string()],
                engine: Engine::Wayfern,
                default_start_page: Some("https://duckduckgo.com".to_string()),
                default_search_provider: Some("duckduckgo".to_string()),
                ephemeral_mode: false,
                password_lock_enabled: false,
                panic_frame_enabled: false,
                panic_frame_color: None,
                panic_protected_sites: vec![],
                ephemeral_retain_paths: vec![],
            })
            .map_err(|e| e.to_string())?;
    }
    if !has("Firefox Default") {
        manager
            .create_profile(CreateProfileInput {
                name: "Firefox Default".to_string(),
                description: Some("Default isolated Firefox profile (Camoufox).".to_string()),
                tags: vec!["default".to_string(), "engine:camoufox".to_string()],
                engine: Engine::Camoufox,
                default_start_page: Some("https://duckduckgo.com".to_string()),
                default_search_provider: Some("duckduckgo".to_string()),
                ephemeral_mode: false,
                password_lock_enabled: false,
                panic_frame_enabled: false,
                panic_frame_color: None,
                panic_protected_sites: vec![],
                ephemeral_retain_paths: vec![],
            })
            .map_err(|e| e.to_string())?;
    }
    if !has("Chromium Private Memory") {
        manager
            .create_profile(CreateProfileInput {
                name: "Chromium Private Memory".to_string(),
                description: Some(
                    "Ephemeral memory-only Chromium profile for private sessions.".to_string(),
                ),
                tags: vec![
                    "default".to_string(),
                    "private".to_string(),
                    "engine:wayfern".to_string(),
                ],
                engine: Engine::Wayfern,
                default_start_page: Some("https://duckduckgo.com".to_string()),
                default_search_provider: Some("duckduckgo".to_string()),
                ephemeral_mode: true,
                password_lock_enabled: false,
                panic_frame_enabled: false,
                panic_frame_color: None,
                panic_protected_sites: vec![],
                ephemeral_retain_paths: vec![],
            })
            .map_err(|e| e.to_string())?;
    }
    if !has("Firefox Private Memory") {
        manager
            .create_profile(CreateProfileInput {
                name: "Firefox Private Memory".to_string(),
                description: Some(
                    "Ephemeral memory-only Firefox profile for private sessions.".to_string(),
                ),
                tags: vec![
                    "default".to_string(),
                    "private".to_string(),
                    "engine:camoufox".to_string(),
                ],
                engine: Engine::Camoufox,
                default_start_page: Some("https://duckduckgo.com".to_string()),
                default_search_provider: Some("duckduckgo".to_string()),
                ephemeral_mode: true,
                password_lock_enabled: false,
                panic_frame_enabled: false,
                panic_frame_color: None,
                panic_protected_sites: vec![],
                ephemeral_retain_paths: vec![],
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn persist_identity_store(path: &PathBuf, store: &IdentityStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create identity dir: {e}"))?;
    }
    let bytes =
        serde_json::to_vec_pretty(store).map_err(|e| format!("serialize identity store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write identity store: {e}"))
}

pub fn persist_network_store(path: &PathBuf, store: &NetworkStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create network dir: {e}"))?;
    }
    let bytes =
        serde_json::to_vec_pretty(store).map_err(|e| format!("serialize network store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write network store: {e}"))
}

pub fn persist_extension_library_store(
    path: &PathBuf,
    store: &ExtensionLibraryStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create extension library dir: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|e| format!("serialize extension library: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write extension library: {e}"))
}

pub fn persist_app_update_store(path: &PathBuf, store: &AppUpdateStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create app update dir: {e}"))?;
    }
    let bytes =
        serde_json::to_vec_pretty(store).map_err(|e| format!("serialize app update store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write app update store: {e}"))
}

pub fn persist_sync_store_with_secret(
    path: &PathBuf,
    secret_material: &str,
    store: &SyncStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create sync dir: {e}"))?;
    }
    crate::sensitive_store::persist_sensitive_json(path, "sync-store", secret_material, store)
}

pub fn persist_link_routing_store_with_secret(
    path: &PathBuf,
    secret_material: &str,
    store: &LinkRoutingStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create link routing dir: {e}"))?;
    }
    crate::sensitive_store::persist_sensitive_json(
        path,
        "link-routing-store",
        secret_material,
        store,
    )
}
