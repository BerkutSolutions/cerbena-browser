use std::path::Path;
#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(target_os = "windows")]
use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read},
    path::PathBuf,
};

#[cfg(target_os = "windows")]
use browser_profile::Engine;
use browser_profile::ProfileMetadata;
#[cfg(any(target_os = "windows", test))]
use serde_json::Value;
#[cfg(target_os = "windows")]
use zip::ZipArchive;

#[cfg(target_os = "windows")]
use crate::launcher_commands::push_runtime_log;
use crate::state::AppState;
#[cfg(target_os = "windows")]
use crate::{profile_security::tags_allow_keepassxc, state::app_local_data_root};
#[cfg(target_os = "windows")]
#[path = "keepassxc_bridge_helpers.rs"]
mod helpers;

#[cfg(target_os = "windows")]
const KEEPASSXC_PROXY_FILE: &str = "keepassxc-proxy.exe";
#[cfg(target_os = "windows")]
const KEEPASSXC_HOST_NAME: &str = "org.keepassxc.keepassxc_browser";
#[cfg(any(target_os = "windows", test))]
const KEEPASSXC_STORE_EXTENSION_ID: &str = "oboonakemofpalcgghocfoadofidjkkk";
#[cfg(target_os = "windows")]
const KEEPASSXC_STORE_EXTENSION_ORIGIN: &str =
    "chrome-extension://oboonakemofpalcgghocfoadofidjkkk/";
#[cfg(any(target_os = "windows", test))]
const KEEPASSXC_FIREFOX_EXTENSION_ID: &str = "keepassxc-browser@keepassxc.org";
#[cfg(target_os = "windows")]
const KEEPASSXC_DEBUG_FLAG_FILE: &str = "keepassxc-native-messaging-debug.flag";

#[cfg(target_os = "windows")]
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
            helpers::clear_keepassxc_debug_flag(state, profile_root);
            return Ok(());
        }

        helpers::write_keepassxc_log(
            state,
            profile,
            &format!(
                "Preparing KeePassXC native messaging bridge for {}.",
                helpers::keepassxc_engine_label(&profile.engine)
            ),
        );

        let Some(proxy_path) = helpers::discover_keepassxc_proxy_path() else {
            helpers::write_keepassxc_log(
                state,
                profile,
                "KeePassXC proxy executable was not found on disk.",
            );
            helpers::clear_keepassxc_debug_flag(state, profile_root);
            return Ok(());
        };
        helpers::write_keepassxc_log(
            state,
            profile,
            &format!("KeePassXC proxy resolved to {}", proxy_path.display()),
        );

        let app_data = app_local_data_root(&state.app_handle)?;
        let bridge_root = app_data.join("native-messaging").join("keepassxc");
        fs::create_dir_all(&bridge_root)
            .map_err(|e| format!("create KeePassXC bridge directory: {e}"))?;
        let manifest_path = bridge_root.join(format!("{KEEPASSXC_HOST_NAME}.json"));
        let manifest = helpers::keepassxc_manifest(state, profile, profile_root, &proxy_path);
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest)
                .map_err(|e| format!("serialize KeePassXC manifest: {e}"))?,
        )
        .map_err(|e| format!("write KeePassXC manifest: {e}"))?;
        helpers::write_keepassxc_log(
            state,
            profile,
            &format!(
                "KeePassXC manifest written to {} with {}",
                manifest_path.display(),
                helpers::keepassxc_manifest_debug_summary(&manifest)
            ),
        );

        for key in helpers::keepassxc_registry_keys(&profile.engine) {
            helpers::register_native_host_key(state, profile, &key, &manifest_path);
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
        helpers::write_keepassxc_log(
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
