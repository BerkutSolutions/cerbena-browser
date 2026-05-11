use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Instant,
};

use browser_api_local::{
    HomePageService, LaunchHookService, PanicWipeService, PipPolicyService, SearchProviderRegistry,
    SecurityGuardrails,
};
use browser_network_policy::ServiceCatalog;
use browser_profile::ProfileManager;
use browser_sync_client::SnapshotManager;
use tauri::AppHandle;
#[cfg(not(target_os = "windows"))]
use tauri::Manager;
use uuid::Uuid;

use crate::device_posture::{load_device_posture_store, DevicePostureStore};
use crate::launch_sessions::{load_launch_session_store, LaunchSessionStore};
use crate::network_sandbox::{
    load_network_sandbox_store, migrate_network_sandbox_store, NetworkSandboxStore,
};
use crate::network_sandbox_lifecycle::NetworkSandboxLifecycleState;
use crate::profile_extensions::{
    load_profile_extension_store, migrate_legacy_profile_extensions,
    sync_library_assignments_from_profile_store, ProfileExtensionStore,
};
use crate::profile_runtime_logs::{load_profile_log_store, ProfileLogStore};
use crate::route_runtime::RouteRuntimeState;
use crate::sensitive_store::derive_app_secret_material;
use crate::service_catalog_seed::build_service_catalog;
use crate::shell_commands::{load_shell_preference_store, ShellPreferenceStore};
use crate::traffic_gateway::{load_rules_store, load_traffic_log, TrafficGatewayState};
use crate::update_commands::{AppUpdateStore, UpdaterLaunchMode, UpdaterRuntimeState};
#[path = "state_store_load.rs"]
mod store_load;
#[path = "state_store_persist.rs"]
mod store_persist;
#[path = "state_defaults.rs"]
mod defaults;

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

#[path = "state_core_types.rs"]
mod state_core_types;

pub(crate) use state_core_types::*;

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
    pub profile_extension_store: Mutex<ProfileExtensionStore>,
    pub snapshot_manager: Mutex<SnapshotManager>,
    pub home_service: Mutex<HomePageService>,
    pub panic_service: Mutex<PanicWipeService>,
    pub launch_hook_service: Mutex<LaunchHookService>,
    pub pip_service: Mutex<PipPolicyService>,
    pub search_registry: Mutex<SearchProviderRegistry>,
    pub security_guardrails: Mutex<SecurityGuardrails>,
    pub device_posture_store: Mutex<DevicePostureStore>,
    pub app_update_store: Mutex<AppUpdateStore>,
    pub shell_preference_store: Mutex<ShellPreferenceStore>,
    pub hidden_default_profiles: Mutex<HiddenDefaultProfilesStore>,
    pub updater_runtime: Arc<Mutex<UpdaterRuntimeState>>,
    pub runtime_logs: Mutex<Vec<String>>,
    pub profile_logs: Mutex<ProfileLogStore>,
    pub pending_external_link: Mutex<Option<String>>,
    pub launched_processes: Mutex<BTreeMap<Uuid, u32>>,
    pub profile_launch_attempts: Mutex<BTreeMap<Uuid, Instant>>,
    pub active_panic_frames: Mutex<BTreeSet<Uuid>>,
    pub active_engine_downloads: Mutex<BTreeSet<String>>,
    pub cancelled_engine_downloads: Arc<Mutex<BTreeSet<String>>>,
    pub active_network_downloads: Mutex<BTreeSet<String>>,
    pub shutdown_cleanup_started: AtomicBool,
    pub allow_exit_once: AtomicBool,
    pub network_sandbox_store: Mutex<NetworkSandboxStore>,
    pub network_sandbox_lifecycle: Mutex<NetworkSandboxLifecycleState>,
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
        let hidden_default_profiles =
            load_hidden_default_profiles_store(&app_data.join("hidden_default_profiles.json"))?;
        ensure_default_profiles(&manager, &hidden_default_profiles.names)?;
        let identity_store = load_identity_store(&app_data.join("identity_store.json"))?;
        let network_store = load_network_store(&app_data.join("network_store.json"))?;
        let mut extension_library =
            load_extension_library_store(&app_data.join("extension_library.json"))?;
        let sync_store =
            load_sync_store(&app_data.join("sync_store.json"), &sensitive_store_secret)?;
        let mut network_sandbox_store =
            load_network_sandbox_store(&app_data.join("network_sandbox_store.json"))?;
        let sandbox_changed =
            migrate_network_sandbox_store(&mut network_sandbox_store, &network_store)?;
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
        let shell_preference_store =
            load_shell_preference_store(&app_data.join("shell_preference_store.json"))?;
        let updater_launch_mode = UpdaterLaunchMode::from_args(std::env::args().skip(1));
        let traffic_rules = load_rules_store(&app_data.join("traffic_gateway_rules.json"))?;
        let traffic_log = load_traffic_log(&app_data.join("traffic_gateway_log.json"))?;
        let profile_logs = load_profile_log_store(&app_data.join("profile_runtime_logs.json"))?;
        let profiles = manager.list_profiles().map_err(|e| e.to_string())?;
        let mut profile_extension_store = load_profile_extension_store(&profile_root, &profiles)?;
        let profile_extensions_changed = migrate_legacy_profile_extensions(
            &profile_root,
            &profiles,
            &mut profile_extension_store,
            &mut extension_library,
        )?;
        sync_library_assignments_from_profile_store(
            &mut extension_library,
            &profile_extension_store,
        );
        if sandbox_changed {
            persist_network_sandbox_store(
                &app_data.join("network_sandbox_store.json"),
                &network_sandbox_store,
            )?;
        }
        if profile_extensions_changed {
            crate::profile_extensions::persist_profile_extension_store(
                &profile_root,
                &profile_extension_store,
            )?;
            persist_extension_library_store(
                &app_data.join("extension_library.json"),
                &extension_library,
            )?;
        }

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
            profile_extension_store: Mutex::new(profile_extension_store),
            snapshot_manager: Mutex::new(SnapshotManager::default()),
            home_service: Mutex::new(HomePageService),
            panic_service: Mutex::new(PanicWipeService),
            launch_hook_service: Mutex::new(LaunchHookService),
            pip_service: Mutex::new(PipPolicyService),
            search_registry: Mutex::new(SearchProviderRegistry::default()),
            security_guardrails: Mutex::new(SecurityGuardrails::default()),
            device_posture_store: Mutex::new(device_posture_store),
            app_update_store: Mutex::new(app_update_store),
            shell_preference_store: Mutex::new(shell_preference_store),
            hidden_default_profiles: Mutex::new(hidden_default_profiles),
            updater_runtime: Arc::new(Mutex::new(UpdaterRuntimeState::new(updater_launch_mode))),
            runtime_logs: Mutex::new(Vec::new()),
            profile_logs: Mutex::new(profile_logs),
            pending_external_link: Mutex::new(None),
            launched_processes: Mutex::new(BTreeMap::new()),
            profile_launch_attempts: Mutex::new(BTreeMap::new()),
            active_panic_frames: Mutex::new(BTreeSet::new()),
            active_engine_downloads: Mutex::new(BTreeSet::new()),
            cancelled_engine_downloads: Arc::new(Mutex::new(BTreeSet::new())),
            active_network_downloads: Mutex::new(BTreeSet::new()),
            shutdown_cleanup_started: AtomicBool::new(false),
            allow_exit_once: AtomicBool::new(false),
            network_sandbox_store: Mutex::new(network_sandbox_store),
            network_sandbox_lifecycle: Mutex::new(NetworkSandboxLifecycleState::default()),
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

    pub fn profile_log_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        crate::profile_runtime_logs::profile_log_store_path(app)
    }

    pub fn network_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("network_store.json"))
    }

    pub fn network_sandbox_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("network_sandbox_store.json"))
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

    pub fn shell_preference_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("shell_preference_store.json"))
    }

    pub fn runtime_log_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("runtime_logs.log"))
    }

    pub fn hidden_default_profiles_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("hidden_default_profiles.json"))
    }

    pub fn app_update_root_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("updates"))
    }

    pub fn global_security_store_path(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("global_security_store.json"))
    }

    pub fn managed_certificates_root(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app_local_data_root(app)?;
        Ok(app_data.join("managed-certificates"))
    }

    pub fn global_security_legacy_path(&self) -> PathBuf {
        self.profile_root.join("_global-security.json")
    }
}

#[path = "state_core_store_access.rs"]
mod state_core_store_access;

pub(crate) use state_core_store_access::{
    load_app_update_store,
    load_extension_library_store,
    load_hidden_default_profiles_store,
    load_identity_store,
    load_link_routing_store,
    load_network_store,
    load_sync_store,
};

pub(crate) use defaults::{
    ensure_default_profiles_impl as ensure_default_profiles,
    is_builtin_default_profile_name_impl as is_builtin_default_profile_name,
    persist_hidden_default_profiles_store_impl as persist_hidden_default_profiles_store,
};

pub(crate) use store_persist::{
    persist_app_update_store_impl as persist_app_update_store,
    persist_extension_library_store_impl as persist_extension_library_store,
    persist_identity_store_impl as persist_identity_store,
    persist_link_routing_store_with_secret_impl as persist_link_routing_store_with_secret,
    persist_network_sandbox_store_impl as persist_network_sandbox_store,
    persist_network_store_impl as persist_network_store,
    persist_sync_store_with_secret_impl as persist_sync_store_with_secret,
};
