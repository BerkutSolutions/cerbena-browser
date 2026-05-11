use super::*;

pub(crate) fn keepassxc_registry_keys(engine: &Engine) -> Vec<String> {
    match engine {
        Engine::Chromium | Engine::UngoogledChromium => vec![
            format!(r"HKCU\Software\Chromium\NativeMessagingHosts\{KEEPASSXC_HOST_NAME}"),
            format!(r"HKCU\Software\Chromium\NativeMessagingHosts\{KEEPASSXC_HOST_NAME}"),
            format!(r"HKCU\Software\Google\Chrome\NativeMessagingHosts\{KEEPASSXC_HOST_NAME}"),
            format!(r"HKCU\Software\Vivaldi\NativeMessagingHosts\{KEEPASSXC_HOST_NAME}"),
        ],
        Engine::FirefoxEsr | Engine::Librewolf => vec![
            format!(r"HKCU\Software\Mozilla\NativeMessagingHosts\{KEEPASSXC_HOST_NAME}"),
            format!(r"HKCU\Software\LibreWolf\NativeMessagingHosts\{KEEPASSXC_HOST_NAME}"),
        ],
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn discover_keepassxc_proxy_path() -> Option<PathBuf> {
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
pub(crate) fn register_native_host_key(
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

#[cfg(target_os = "windows")]
pub(crate) fn clear_keepassxc_debug_flag(state: &AppState, profile_root: &Path) {
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

#[cfg(target_os = "windows")]
pub(crate) fn write_keepassxc_log(state: &AppState, profile: &ProfileMetadata, message: &str) {
    let line = format!("[keepassxc-bridge] profile={} {}", profile.id, message);
    eprintln!("{line}");
    push_runtime_log(state, line);
}

#[cfg(test)]
mod tests {
    use super::{
        keepassxc_manifest_debug_summary, manifest_stable_id, KEEPASSXC_FIREFOX_EXTENSION_ID,
    };

    #[test]
    fn extracts_firefox_manifest_id() {
        let manifest = serde_json::json!({
            "browser_specific_settings": {
                "gecko": {
                    "id": KEEPASSXC_FIREFOX_EXTENSION_ID
                }
            }
        });
        assert_eq!(
            manifest_stable_id(&manifest).as_deref(),
            Some(KEEPASSXC_FIREFOX_EXTENSION_ID)
        );
    }

    #[test]
    fn debug_summary_prefers_allowed_extensions() {
        let manifest = serde_json::json!({
            "allowed_extensions": [KEEPASSXC_FIREFOX_EXTENSION_ID]
        });
        assert!(keepassxc_manifest_debug_summary(&manifest).contains("allowed_extensions"));
    }
}
