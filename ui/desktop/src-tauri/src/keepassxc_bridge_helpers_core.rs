use super::*;

pub(crate) fn keepassxc_manifest(
    state: &AppState,
    profile: &ProfileMetadata,
    profile_root: &Path,
    proxy_path: &Path,
) -> Value {
    let mut manifest = serde_json::json!({
        "name": KEEPASSXC_HOST_NAME,
        "description": format!(
            "KeePassXC bridge for Cerbena Browser / {}",
            support::keepassxc_engine_label(&profile.engine)
        ),
        "path": proxy_path,
        "type": "stdio"
    });
    match profile.engine {
        Engine::Chromium | Engine::UngoogledChromium => {
            let allowed_origins =
                support::collect_keepassxc_allowed_origins(state, profile, profile_root);
            manifest["allowed_origins"] = serde_json::json!(allowed_origins);
        }
        Engine::FirefoxEsr | Engine::Librewolf => {
            let allowed_extensions = support::collect_keepassxc_firefox_extension_ids(
                state,
                profile,
                profile_root,
            );
            manifest["allowed_extensions"] = serde_json::json!(allowed_extensions);
        }
    }
    manifest
}

#[cfg(target_os = "windows")]

#[path = "keepassxc_bridge_helpers_core_support.rs"]
mod support;
pub(crate) use support::*;


