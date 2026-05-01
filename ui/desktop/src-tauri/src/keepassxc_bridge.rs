use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use browser_profile::ProfileMetadata;
use serde_json::Value;

use crate::{
    profile_security::tags_allow_keepassxc,
    state::{app_local_data_root, AppState},
};

const KEEPASSXC_PROXY_FILE: &str = "keepassxc-proxy.exe";
const KEEPASSXC_HOST_NAME: &str = "org.keepassxc.keepassxc_browser";
const KEEPASSXC_STORE_EXTENSION_ORIGIN: &str =
    "chrome-extension://oboonakemofpalcgghocfoadofidjkkk/";
const KEEPASSXC_DEBUG_FLAG_FILE: &str = "keepassxc-native-messaging-debug.flag";

fn keepassxc_debug_flag_path(profile_root: &Path) -> PathBuf {
    profile_root.join("policy").join(KEEPASSXC_DEBUG_FLAG_FILE)
}

pub fn ensure_keepassxc_bridge_for_profile(
    state: &AppState,
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (state, profile, profile_root);
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        if !tags_allow_keepassxc(&profile.tags) {
            clear_keepassxc_debug_flag(state, profile_root);
            return Ok(());
        }

        write_keepassxc_log(
            state,
            profile,
            "Preparing KeePassXC native messaging bridge for Wayfern.",
        );

        let Some(proxy_path) = discover_keepassxc_proxy_path() else {
            write_keepassxc_log(
                state,
                profile,
                "KeePassXC proxy executable was not found on disk.",
            );
            clear_keepassxc_debug_flag(state, profile_root);
            return Ok(());
        };
        write_keepassxc_log(
            state,
            profile,
            &format!("KeePassXC proxy resolved to {}", proxy_path.display()),
        );

        let app_data = app_local_data_root(&state.app_handle)?;
        let bridge_root = app_data.join("native-messaging").join("keepassxc");
        fs::create_dir_all(&bridge_root)
            .map_err(|e| format!("create KeePassXC bridge directory: {e}"))?;
        let manifest_path = bridge_root.join(format!("{KEEPASSXC_HOST_NAME}.json"));
        let allowed_origins = collect_keepassxc_allowed_origins(state, profile, profile_root);
        let manifest = serde_json::json!({
            "name": KEEPASSXC_HOST_NAME,
            "description": "KeePassXC bridge for Cerbena Browser / Wayfern",
            "path": proxy_path,
            "type": "stdio",
            "allowed_origins": allowed_origins
        });
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest)
                .map_err(|e| format!("serialize KeePassXC manifest: {e}"))?,
        )
        .map_err(|e| format!("write KeePassXC manifest: {e}"))?;
        write_keepassxc_log(
            state,
            profile,
            &format!(
                "KeePassXC manifest written to {} with allowed_origins={:?}",
                manifest_path.display(),
                manifest
                    .get("allowed_origins")
                    .and_then(|value| value.as_array())
                    .cloned()
                    .unwrap_or_default()
            ),
        );

        let registry_keys = [
            format!(r"HKCU\Software\Wayfern\NativeMessagingHosts\{KEEPASSXC_HOST_NAME}"),
            format!(r"HKCU\Software\Chromium\NativeMessagingHosts\{KEEPASSXC_HOST_NAME}"),
            format!(r"HKCU\Software\Google\Chrome\NativeMessagingHosts\{KEEPASSXC_HOST_NAME}"),
            format!(r"HKCU\Software\Vivaldi\NativeMessagingHosts\{KEEPASSXC_HOST_NAME}"),
        ];
        for key in registry_keys {
            register_native_host_key(state, profile, &key, &manifest_path);
        }

        if let Some(parent) = keepassxc_debug_flag_path(profile_root).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create KeePassXC debug flag dir: {e}"))?;
        }
        fs::write(
            keepassxc_debug_flag_path(profile_root),
            b"enable-native-messaging-debug\n",
        )
        .map_err(|e| format!("write KeePassXC debug flag: {e}"))?;
        write_keepassxc_log(
            state,
            profile,
            &format!(
                "KeePassXC native messaging debug flag enabled at {}",
                keepassxc_debug_flag_path(profile_root).display()
            ),
        );
        Ok(())
    }
}

fn collect_keepassxc_allowed_origins(
    state: &AppState,
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Vec<String> {
    let mut origins = BTreeSet::new();
    origins.insert(KEEPASSXC_STORE_EXTENSION_ORIGIN.to_string());
    for origin in read_keepassxc_origins_from_secure_preferences(state, profile, profile_root) {
        origins.insert(origin);
    }
    origins.into_iter().collect()
}

fn read_keepassxc_origins_from_secure_preferences(
    state: &AppState,
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Vec<String> {
    let secure_preferences_path = profile_root
        .join("engine-profile")
        .join("Default")
        .join("Secure Preferences");
    let keepassxc_path = profile_root
        .join("policy")
        .join("wayfern-extensions")
        .join("oboonakemofpalcgghocfoadofidjkkk");
    if !secure_preferences_path.exists() {
        write_keepassxc_log(
            state,
            profile,
            &format!(
                "KeePassXC secure preferences were not found yet at {}",
                secure_preferences_path.display()
            ),
        );
        return Vec::new();
    }

    let text = match fs::read_to_string(&secure_preferences_path) {
        Ok(value) => value,
        Err(error) => {
            write_keepassxc_log(
                state,
                profile,
                &format!(
                    "Failed to read KeePassXC secure preferences {}: {}",
                    secure_preferences_path.display(),
                    error
                ),
            );
            return Vec::new();
        }
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            write_keepassxc_log(
                state,
                profile,
                &format!(
                    "Failed to parse KeePassXC secure preferences {}: {}",
                    secure_preferences_path.display(),
                    error
                ),
            );
            return Vec::new();
        }
    };
    let expected_path = keepassxc_path.to_string_lossy().to_string();
    let mut origins = Vec::new();
    if let Some(settings) = value
        .get("extensions")
        .and_then(|value| value.get("settings"))
        .and_then(Value::as_object)
    {
        for (extension_id, item) in settings {
            let Some(path) = item.get("path").and_then(Value::as_str) else {
                continue;
            };
            if path.eq_ignore_ascii_case(&expected_path) {
                let origin = format!("chrome-extension://{extension_id}/");
                write_keepassxc_log(
                    state,
                    profile,
                    &format!(
                        "Discovered KeePassXC runtime origin {} from {}",
                        origin,
                        secure_preferences_path.display()
                    ),
                );
                origins.push(origin);
            }
        }
    }
    if origins.is_empty() {
        write_keepassxc_log(
            state,
            profile,
            &format!(
                "KeePassXC runtime origin was not found in {} for path {}",
                secure_preferences_path.display(),
                expected_path
            ),
        );
    }
    origins
}

#[cfg(target_os = "windows")]
fn discover_keepassxc_proxy_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("KeePassXC").join(KEEPASSXC_PROXY_FILE));
    }
    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        candidates.push(
            PathBuf::from(program_files_x86)
                .join("KeePassXC")
                .join(KEEPASSXC_PROXY_FILE),
        );
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local_app_data)
                .join("Programs")
                .join("KeePassXC")
                .join(KEEPASSXC_PROXY_FILE),
        );
    }
    candidates.into_iter().find(|path| path.exists())
}

#[cfg(target_os = "windows")]
fn register_native_host_key(
    state: &AppState,
    profile: &ProfileMetadata,
    registry_key: &str,
    manifest_path: &Path,
) {
    let manifest_value = manifest_path.to_string_lossy().to_string();
    match Command::new("reg")
        .args([
            "add",
            registry_key,
            "/ve",
            "/t",
            "REG_SZ",
            "/d",
            &manifest_value,
            "/f",
        ])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if output.status.success() {
                write_keepassxc_log(
                    state,
                    profile,
                    &format!(
                        "Registered KeePassXC native host key {} -> {}{}",
                        registry_key,
                        manifest_value,
                        if stdout.is_empty() {
                            String::new()
                        } else {
                            format!(" ({stdout})")
                        }
                    ),
                );
            } else {
                write_keepassxc_log(
                    state,
                    profile,
                    &format!(
                        "Failed to register KeePassXC native host key {}. status={:?} stdout={} stderr={}",
                        registry_key,
                        output.status.code(),
                        stdout,
                        stderr
                    ),
                );
            }
        }
        Err(error) => {
            write_keepassxc_log(
                state,
                profile,
                &format!(
                    "Failed to spawn reg.exe for KeePassXC key {}: {}",
                    registry_key, error
                ),
            );
        }
    }
}

fn clear_keepassxc_debug_flag(state: &AppState, profile_root: &Path) {
    let path = keepassxc_debug_flag_path(profile_root);
    if path.exists() {
        if let Err(error) = fs::remove_file(&path) {
            let line = format!(
                "[keepassxc-bridge] failed to clear debug flag {}: {}",
                path.display(),
                error
            );
            eprintln!("{line}");
            if let Ok(mut logs) = state.runtime_logs.lock() {
                logs.push(line);
                if logs.len() > 1000 {
                    let overflow = logs.len() - 1000;
                    logs.drain(0..overflow);
                }
            }
        }
    }
}

fn write_keepassxc_log(state: &AppState, profile: &ProfileMetadata, message: &str) {
    let line = format!("[keepassxc-bridge] profile={} {}", profile.id, message);
    eprintln!("{line}");
    if let Ok(mut logs) = state.runtime_logs.lock() {
        logs.push(line);
        if logs.len() > 1000 {
            let overflow = logs.len() - 1000;
            logs.drain(0..overflow);
        }
    }
}
