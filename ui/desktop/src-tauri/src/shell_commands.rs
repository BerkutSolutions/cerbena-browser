use std::{fs, path::PathBuf, process::Command, sync::atomic::Ordering};

use serde::{Deserialize, Serialize};
use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, Wry,
};

use crate::{
    envelope::{ok, UiEnvelope},
    install_registration,
    state::AppState,
};

const TRAY_ID: &str = "main-tray";
const TRAY_MENU_SHOW_ID: &str = "tray-show";
const TRAY_MENU_EXIT_ID: &str = "tray-exit";
const CLOSE_PROMPT_EVENT: &str = "app-close-requested";

fn default_check_default_browser_on_startup() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellPreferenceStore {
    #[serde(default = "default_check_default_browser_on_startup")]
    pub check_default_browser_on_startup: bool,
    #[serde(default)]
    pub default_browser_prompt_decided: bool,
    #[serde(default)]
    pub minimize_to_tray_enabled: bool,
    #[serde(default)]
    pub close_to_tray_prompt_declined: bool,
}

impl Default for ShellPreferenceStore {
    fn default() -> Self {
        Self {
            check_default_browser_on_startup: true,
            default_browser_prompt_decided: false,
            minimize_to_tray_enabled: false,
            close_to_tray_prompt_declined: false,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellPreferenceUpdateRequest {
    pub check_default_browser_on_startup: Option<bool>,
    pub default_browser_prompt_decided: Option<bool>,
    pub minimize_to_tray_enabled: Option<bool>,
    pub close_to_tray_prompt_declined: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellPreferencesState {
    pub check_default_browser_on_startup: bool,
    pub default_browser_prompt_decided: bool,
    pub minimize_to_tray_enabled: bool,
    pub close_to_tray_prompt_declined: bool,
    pub is_default_browser: bool,
    pub should_prompt_default_browser_preference: bool,
    pub should_prompt_default_link_profile: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseRequestAction {
    AllowExit,
    HideToTray,
    PromptToEnableTray,
}

pub(crate) fn load_shell_preference_store(path: &PathBuf) -> Result<ShellPreferenceStore, String> {
    if !path.exists() {
        return Ok(ShellPreferenceStore::default());
    }
    let raw = fs::read(path).map_err(|e| format!("read shell preference store: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("parse shell preference store: {e}"))
}

pub(crate) fn persist_shell_preference_store(
    path: &PathBuf,
    store: &ShellPreferenceStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create shell preference dir: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|e| format!("serialize shell preference store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write shell preference store: {e}"))
}

pub fn setup_system_tray(app: &tauri::App<Wry>) -> Result<(), String> {
    let menu = MenuBuilder::new(app)
        .text(TRAY_MENU_SHOW_ID, "Open Cerbena")
        .text(TRAY_MENU_EXIT_ID, "Exit Cerbena")
        .build()
        .map_err(|e| format!("build tray menu: {e}"))?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Cerbena Browser")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_SHOW_ID => {
                let _ = restore_main_window(app);
            }
            TRAY_MENU_EXIT_ID => {
                let _ = request_exit(app);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = restore_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder
        .build(app)
        .map_err(|e| format!("build tray icon: {e}"))?;
    Ok(())
}

pub fn resolve_close_request(app: &AppHandle) -> Result<CloseRequestAction, String> {
    let state = app.state::<AppState>();
    if state.allow_exit_once.swap(false, Ordering::SeqCst) {
        return Ok(CloseRequestAction::AllowExit);
    }

    let store = state
        .shell_preference_store
        .lock()
        .map_err(|_| "shell preference store lock poisoned".to_string())?;
    if store.minimize_to_tray_enabled {
        return Ok(CloseRequestAction::HideToTray);
    }
    if store.close_to_tray_prompt_declined {
        return Ok(CloseRequestAction::AllowExit);
    }
    Ok(CloseRequestAction::PromptToEnableTray)
}

pub fn emit_close_to_tray_prompt(app: &AppHandle) {
    let _ = app.emit(
        CLOSE_PROMPT_EVENT,
        serde_json::json!({ "reason": "tray-offer" }),
    );
}

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.hide().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn restore_main_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    Ok(())
}

pub fn request_exit(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.allow_exit_once.store(true, Ordering::SeqCst);
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.close().map_err(|e| e.to_string())
}

fn build_shell_preferences_state(state: &AppState) -> Result<ShellPreferencesState, String> {
    let store = state
        .shell_preference_store
        .lock()
        .map_err(|_| "shell preference store lock poisoned".to_string())?
        .clone();
    let link_store = state
        .link_routing_store
        .lock()
        .map_err(|_| "link routing store lock poisoned".to_string())?;
    let is_default_browser = install_registration::is_default_browser();
    let should_prompt_default_link_profile = store.check_default_browser_on_startup
        && is_default_browser
        && link_store.global_profile_id.is_none();
    Ok(ShellPreferencesState {
        check_default_browser_on_startup: store.check_default_browser_on_startup,
        default_browser_prompt_decided: store.default_browser_prompt_decided,
        minimize_to_tray_enabled: store.minimize_to_tray_enabled,
        close_to_tray_prompt_declined: store.close_to_tray_prompt_declined,
        is_default_browser,
        should_prompt_default_browser_preference: !store.default_browser_prompt_decided,
        should_prompt_default_link_profile,
    })
}

fn persist_shell_preferences(state: &AppState) -> Result<(), String> {
    let path = state.shell_preference_store_path(&state.app_handle)?;
    let store = state
        .shell_preference_store
        .lock()
        .map_err(|_| "shell preference store lock poisoned".to_string())?;
    persist_shell_preference_store(&path, &store)
}

#[tauri::command]
pub fn get_shell_preferences_state(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<ShellPreferencesState>, String> {
    Ok(ok(correlation_id, build_shell_preferences_state(&state)?))
}

#[tauri::command]
pub fn save_shell_preferences(
    state: State<AppState>,
    request: ShellPreferenceUpdateRequest,
    correlation_id: String,
) -> Result<UiEnvelope<ShellPreferencesState>, String> {
    {
        let mut store = state
            .shell_preference_store
            .lock()
            .map_err(|_| "shell preference store lock poisoned".to_string())?;
        if let Some(value) = request.check_default_browser_on_startup {
            store.check_default_browser_on_startup = value;
        }
        if let Some(value) = request.default_browser_prompt_decided {
            store.default_browser_prompt_decided = value;
        }
        if let Some(value) = request.minimize_to_tray_enabled {
            store.minimize_to_tray_enabled = value;
            if value {
                store.close_to_tray_prompt_declined = false;
            }
        }
        if let Some(value) = request.close_to_tray_prompt_declined {
            store.close_to_tray_prompt_declined = value;
        }
    }
    persist_shell_preferences(&state)?;
    Ok(ok(correlation_id, build_shell_preferences_state(&state)?))
}

#[tauri::command]
pub fn window_hide_to_tray(
    app: AppHandle,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    hide_main_window(&app)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn window_restore_from_tray(
    app: AppHandle,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    restore_main_window(&app)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn confirm_app_exit(
    app: AppHandle,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    request_exit(&app)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn open_default_apps_settings(correlation_id: String) -> Result<UiEnvelope<bool>, String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", "ms-settings:defaultapps"])
            .spawn()
            .map_err(|e| format!("open default apps settings: {e}"))?;
        return Ok(ok(correlation_id, true));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = correlation_id;
        Err("opening default apps settings is only supported on Windows".to_string())
    }
}
