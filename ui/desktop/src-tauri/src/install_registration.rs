use std::path::Path;

use tauri::AppHandle;

const PRODUCT_NAME: &str = "Cerbena Browser";
const UNINSTALLER_FILE_NAME: &str = "Cerbena Browser Uninstall.exe";
const PUBLISHER: &str = "Berkut Solutions";
const UNINSTALL_SUBKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Cerbena Browser";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn reconcile_install_registration(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        let Ok(current_exe) = std::env::current_exe() else {
            return;
        };
        let Some(install_root) = current_exe.parent().map(|path| path.to_path_buf()) else {
            return;
        };
        let uninstaller_path = install_root.join(UNINSTALLER_FILE_NAME);
        if !uninstaller_path.is_file() {
            return;
        }
        let icon_path = {
            let candidate = install_root.join("cerbena.ico");
            if candidate.is_file() {
                candidate
            } else {
                current_exe.clone()
            }
        };
        let _ = register_uninstall_metadata(&install_root, &icon_path, &uninstaller_path);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
    }
}

#[cfg(target_os = "windows")]
fn register_uninstall_metadata(
    install_root: &Path,
    icon_path: &Path,
    uninstaller_path: &Path,
) -> Result<(), String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(UNINSTALL_SUBKEY)
        .map_err(|e| format!("create uninstall key: {e}"))?;
    key.set_value("DisplayName", &PRODUCT_NAME)
        .map_err(|e| format!("set DisplayName: {e}"))?;
    key.set_value("DisplayVersion", &CURRENT_VERSION)
        .map_err(|e| format!("set DisplayVersion: {e}"))?;
    key.set_value("Publisher", &PUBLISHER)
        .map_err(|e| format!("set Publisher: {e}"))?;
    key.set_value("InstallLocation", &install_root.to_string_lossy().to_string())
        .map_err(|e| format!("set InstallLocation: {e}"))?;
    key.set_value("DisplayIcon", &icon_path.to_string_lossy().to_string())
        .map_err(|e| format!("set DisplayIcon: {e}"))?;
    key.set_value(
        "UninstallString",
        &format!("\"{}\" --uninstall", uninstaller_path.to_string_lossy()),
    )
    .map_err(|e| format!("set UninstallString: {e}"))?;
    key.set_value(
        "QuietUninstallString",
        &format!(
            "\"{}\" --uninstall --silent",
            uninstaller_path.to_string_lossy()
        ),
    )
    .map_err(|e| format!("set QuietUninstallString: {e}"))?;
    key.set_value("NoModify", &1u32)
        .map_err(|e| format!("set NoModify: {e}"))?;
    key.set_value("NoRepair", &1u32)
        .map_err(|e| format!("set NoRepair: {e}"))?;
    Ok(())
}
