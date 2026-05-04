use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use browser_profile::ProfileMetadata;
use serde_json::Value;

use crate::{
    launcher_commands::push_runtime_log,
    profile_security::tags_allow_keepassxc,
    state::{app_local_data_root, AppState},
};

const KEEPASSXC_PROXY_FILE: &str = "keepassxc-proxy.exe";
const KEEPASSXC_HOST_NAME: &str = "org.keepassxc.keepassxc_browser";
const KEEPASSXC_STORE_EXTENSION_ID: &str = "oboonakemofpalcgghocfoadofidjkkk";
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
    let keepassxc_paths = resolve_keepassxc_extension_paths(profile_root);
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
    if keepassxc_paths.is_empty() {
        write_keepassxc_log(
            state,
            profile,
            &format!(
                "KeePassXC extension directory was not found under {}",
                profile_root.join("policy").join("wayfern-extensions").display()
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
    let expected_paths = keepassxc_paths
        .iter()
        .map(|path| normalize_windowsish_path(path))
        .collect::<BTreeSet<_>>();
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
            if expected_paths.contains(&normalize_windowsish_path(Path::new(path))) {
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
                "KeePassXC runtime origin was not found in {} for paths {:?}",
                secure_preferences_path.display(),
                keepassxc_paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
            ),
        );
    }
    origins
}

fn resolve_keepassxc_extension_paths(profile_root: &Path) -> Vec<PathBuf> {
    let extensions_root = profile_root.join("policy").join("wayfern-extensions");
    let mut paths = Vec::new();
    let store_id_dir = extensions_root.join(KEEPASSXC_STORE_EXTENSION_ID);
    if store_id_dir.is_dir() {
        paths.push(store_id_dir);
    }
    let read_dir = match fs::read_dir(&extensions_root) {
        Ok(entries) => entries,
        Err(_) => return paths,
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() || paths.iter().any(|candidate| candidate == &path) {
            continue;
        }
        if directory_looks_like_keepassxc(&path) {
            paths.push(path);
        }
    }
    paths
}

fn directory_looks_like_keepassxc(path: &Path) -> bool {
    let manifest_path = path.join("manifest.json");
    let Ok(text) = fs::read_to_string(manifest_path) else {
        return path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase().contains("keepassxc"))
            .unwrap_or(false);
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    manifest
        .get("name")
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase().contains("keepassxc"))
        .unwrap_or(false)
}

fn normalize_windowsish_path(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_ascii_lowercase()
}

#[cfg(target_os = "windows")]
fn discover_keepassxc_proxy_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        candidates.push(
            PathBuf::from(program_files)
                .join("KeePassXC")
                .join(KEEPASSXC_PROXY_FILE),
        );
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
            push_runtime_log(state, line);
        }
    }
}

fn write_keepassxc_log(state: &AppState, profile: &ProfileMetadata, message: &str) {
    let line = format!("[keepassxc-bridge] profile={} {}", profile.id, message);
    eprintln!("{line}");
    push_runtime_log(state, line);
}
