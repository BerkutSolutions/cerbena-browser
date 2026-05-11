#[cfg(target_os = "windows")]
use std::path::Path;

use tauri::AppHandle;

#[cfg(target_os = "windows")]
const PRODUCT_NAME: &str = "Cerbena Browser";
#[cfg(target_os = "windows")]
const BROWSER_DESCRIPTION: &str =
    "Isolated browser profiles with controlled link routing and network policies.";
#[cfg(target_os = "windows")]
const UNINSTALLER_FILE_NAME: &str = "Cerbena Browser Uninstall.exe";
#[cfg(target_os = "windows")]
const PUBLISHER: &str = "Berkut Solutions";
#[cfg(target_os = "windows")]
const UNINSTALL_SUBKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Cerbena Browser";
#[cfg(target_os = "windows")]
const START_MENU_INTERNET_SUBKEY: &str = r"Software\Clients\StartMenuInternet\Cerbena Browser";
#[cfg(target_os = "windows")]
const REGISTERED_APPLICATIONS_SUBKEY: &str = r"Software\RegisteredApplications";
#[cfg(target_os = "windows")]
const CERBENA_URL_PROG_ID: &str = "CerbenaBrowser.URL";
#[cfg(target_os = "windows")]
const CERBENA_HTML_PROG_ID: &str = "CerbenaBrowser.HTML";
#[cfg(target_os = "windows")]
const CERBENA_MHTML_PROG_ID: &str = "CerbenaBrowser.MHTML";
#[cfg(target_os = "windows")]
const CERBENA_PDF_PROG_ID: &str = "CerbenaBrowser.PDF";
#[cfg(target_os = "windows")]
const CERBENA_XHTML_PROG_ID: &str = "CerbenaBrowser.XHTML";
#[cfg(target_os = "windows")]
const CERBENA_SVG_PROG_ID: &str = "CerbenaBrowser.SVG";
#[cfg(target_os = "windows")]
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
#[cfg(target_os = "windows")]
const URL_ASSOCIATIONS: &[&str] = &[
    "http", "https", "irc", "mailto", "mms", "news", "nntp", "sms", "smsto", "snews", "tel", "urn",
    "webcal",
];
#[cfg(target_os = "windows")]
const FILE_ASSOCIATIONS: &[(&str, &str, &str)] = &[
    (".htm", CERBENA_HTML_PROG_ID, "Cerbena HTML Document"),
    (".html", CERBENA_HTML_PROG_ID, "Cerbena HTML Document"),
    (".shtml", CERBENA_HTML_PROG_ID, "Cerbena HTML Document"),
    (".mht", CERBENA_MHTML_PROG_ID, "Cerbena MHTML Document"),
    (".mhtml", CERBENA_MHTML_PROG_ID, "Cerbena MHTML Document"),
    (".pdf", CERBENA_PDF_PROG_ID, "Cerbena PDF Document"),
    (".svg", CERBENA_SVG_PROG_ID, "Cerbena SVG Document"),
    (".xhy", CERBENA_XHTML_PROG_ID, "Cerbena XHTML Document"),
    (".xht", CERBENA_XHTML_PROG_ID, "Cerbena XHTML Document"),
    (".xhtml", CERBENA_XHTML_PROG_ID, "Cerbena XHTML Document"),
];

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
        let _ = register_browser_capabilities(&current_exe, &icon_path);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
    }
}

#[cfg(target_os = "windows")]
pub fn register_browser_capabilities_for_current_install() -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let install_root = current_exe
        .parent()
        .ok_or_else(|| "resolve install root for browser registration".to_string())?;
    let icon_path = {
        let candidate = install_root.join("cerbena.ico");
        if candidate.is_file() {
            candidate
        } else {
            current_exe.clone()
        }
    };
    register_browser_capabilities(&current_exe, &icon_path)
}

#[cfg(not(target_os = "windows"))]
pub fn register_browser_capabilities_for_current_install() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn remove_browser_capabilities() -> Result<(), String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(START_MENU_INTERNET_SUBKEY);
    let _ = hkcu.delete_subkey_all(format!(r"Software\Classes\{CERBENA_URL_PROG_ID}"));
    let _ = hkcu.delete_subkey_all(format!(r"Software\Classes\{CERBENA_HTML_PROG_ID}"));
    let _ = hkcu.delete_subkey_all(format!(r"Software\Classes\{CERBENA_MHTML_PROG_ID}"));
    let _ = hkcu.delete_subkey_all(format!(r"Software\Classes\{CERBENA_PDF_PROG_ID}"));
    let _ = hkcu.delete_subkey_all(format!(r"Software\Classes\{CERBENA_XHTML_PROG_ID}"));
    let _ = hkcu.delete_subkey_all(format!(r"Software\Classes\{CERBENA_SVG_PROG_ID}"));
    if let Ok(registered) =
        hkcu.open_subkey_with_flags(REGISTERED_APPLICATIONS_SUBKEY, winreg::enums::KEY_SET_VALUE)
    {
        let _ = registered.delete_value(PRODUCT_NAME);
    }
    for (extension, prog_id, _) in FILE_ASSOCIATIONS {
        let path = format!(r"Software\Classes\{}\OpenWithProgids", extension);
        if let Ok(key) = hkcu.open_subkey_with_flags(&path, winreg::enums::KEY_SET_VALUE) {
            let _ = key.delete_value(*prog_id);
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn remove_browser_capabilities() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn is_default_browser() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    ["http", "https"].iter().all(|scheme| {
        let user_choice = format!(
            "Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\{}\\UserChoice",
            scheme
        );
        hkcu.open_subkey(&user_choice)
            .ok()
            .and_then(|key| key.get_value::<String, _>("ProgId").ok())
            .map(|prog_id| prog_id.eq_ignore_ascii_case(CERBENA_URL_PROG_ID))
            .unwrap_or(false)
    })
}

#[cfg(not(target_os = "windows"))]
pub fn is_default_browser() -> bool {
    false
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
    key.set_value(
        "InstallLocation",
        &install_root.to_string_lossy().to_string(),
    )
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

#[cfg(target_os = "windows")]
fn register_browser_capabilities(current_exe: &Path, icon_path: &Path) -> Result<(), String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let command = format!("\"{}\" \"%1\"", current_exe.to_string_lossy());
    let icon = icon_path.to_string_lossy().to_string();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    register_prog_id(
        &hkcu,
        CERBENA_URL_PROG_ID,
        PRODUCT_NAME,
        &icon,
        &command,
        true,
    )?;
    register_prog_id(
        &hkcu,
        CERBENA_HTML_PROG_ID,
        "Cerbena HTML Document",
        &icon,
        &command,
        false,
    )?;
    register_prog_id(
        &hkcu,
        CERBENA_MHTML_PROG_ID,
        "Cerbena MHTML Document",
        &icon,
        &command,
        false,
    )?;
    register_prog_id(
        &hkcu,
        CERBENA_PDF_PROG_ID,
        "Cerbena PDF Document",
        &icon,
        &command,
        false,
    )?;
    register_prog_id(
        &hkcu,
        CERBENA_XHTML_PROG_ID,
        "Cerbena XHTML Document",
        &icon,
        &command,
        false,
    )?;
    register_prog_id(
        &hkcu,
        CERBENA_SVG_PROG_ID,
        "Cerbena SVG Document",
        &icon,
        &command,
        false,
    )?;

    let (client_key, _) = hkcu
        .create_subkey(START_MENU_INTERNET_SUBKEY)
        .map_err(|e| format!("create start menu client key: {e}"))?;
    client_key
        .set_value("", &PRODUCT_NAME)
        .map_err(|e| format!("set start menu client name: {e}"))?;
    client_key
        .set_value("LocalizedString", &PRODUCT_NAME)
        .map_err(|e| format!("set localized string: {e}"))?;
    let (client_icon, _) = client_key
        .create_subkey("DefaultIcon")
        .map_err(|e| format!("create start menu icon key: {e}"))?;
    client_icon
        .set_value("", &icon)
        .map_err(|e| format!("set start menu icon: {e}"))?;
    let (client_command, _) = client_key
        .create_subkey(r"shell\open\command")
        .map_err(|e| format!("create start menu command key: {e}"))?;
    client_command
        .set_value("", &command)
        .map_err(|e| format!("set start menu command: {e}"))?;

    let (capabilities, _) = client_key
        .create_subkey("Capabilities")
        .map_err(|e| format!("create browser capabilities key: {e}"))?;
    capabilities
        .set_value("ApplicationName", &PRODUCT_NAME)
        .map_err(|e| format!("set application name: {e}"))?;
    capabilities
        .set_value("ApplicationDescription", &BROWSER_DESCRIPTION)
        .map_err(|e| format!("set application description: {e}"))?;
    let (url_associations, _) = capabilities
        .create_subkey("UrlAssociations")
        .map_err(|e| format!("create url associations key: {e}"))?;
    for scheme in URL_ASSOCIATIONS {
        url_associations
            .set_value(*scheme, &CERBENA_URL_PROG_ID)
            .map_err(|e| format!("set {scheme} association: {e}"))?;
    }
    let (file_associations, _) = capabilities
        .create_subkey("FileAssociations")
        .map_err(|e| format!("create file associations key: {e}"))?;
    for (extension, prog_id, _) in FILE_ASSOCIATIONS {
        file_associations
            .set_value(*extension, prog_id)
            .map_err(|e| format!("set {extension} association: {e}"))?;
    }

    let (registered, _) = hkcu
        .create_subkey(REGISTERED_APPLICATIONS_SUBKEY)
        .map_err(|e| format!("create registered applications key: {e}"))?;
    registered
        .set_value(
            PRODUCT_NAME,
            &format!(r"{}\Capabilities", START_MENU_INTERNET_SUBKEY),
        )
        .map_err(|e| format!("set registered application path: {e}"))?;
    for (extension, prog_id, _) in FILE_ASSOCIATIONS {
        register_extension_open_with(&hkcu, extension, prog_id)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn register_prog_id(
    hkcu: &winreg::RegKey,
    prog_id: &str,
    display_name: &str,
    icon: &str,
    command: &str,
    is_url_protocol: bool,
) -> Result<(), String> {
    let (prog_key, _) = hkcu
        .create_subkey(format!(r"Software\Classes\{prog_id}"))
        .map_err(|e| format!("create {prog_id} key: {e}"))?;
    prog_key
        .set_value("", &display_name)
        .map_err(|e| format!("set {prog_id} display name: {e}"))?;
    if is_url_protocol {
        prog_key
            .set_value("URL Protocol", &"")
            .map_err(|e| format!("set {prog_id} url protocol marker: {e}"))?;
    }
    let (default_icon, _) = prog_key
        .create_subkey("DefaultIcon")
        .map_err(|e| format!("create {prog_id} icon key: {e}"))?;
    default_icon
        .set_value("", &icon)
        .map_err(|e| format!("set {prog_id} icon: {e}"))?;
    let (open_command, _) = prog_key
        .create_subkey(r"shell\open\command")
        .map_err(|e| format!("create {prog_id} command key: {e}"))?;
    open_command
        .set_value("", &command)
        .map_err(|e| format!("set {prog_id} command: {e}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn register_extension_open_with(
    hkcu: &winreg::RegKey,
    extension: &str,
    prog_id: &str,
) -> Result<(), String> {
    use winreg::{enums::RegType, RegValue};

    let (key, _) = hkcu
        .create_subkey(format!(r"Software\Classes\{}\OpenWithProgids", extension))
        .map_err(|e| format!("create {extension} OpenWithProgids key: {e}"))?;
    key.set_raw_value(
        prog_id,
        &RegValue {
            bytes: Vec::new(),
            vtype: RegType::REG_NONE,
        },
    )
    .map_err(|e| format!("set {extension} OpenWithProgids value: {e}"))?;
    Ok(())
}
