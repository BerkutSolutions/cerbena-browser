use super::*;
use browser_profile::{CreateProfileInput, Engine, PatchProfileInput};

pub(crate) const BUILTIN_DEFAULT_PROFILE_NAMES: &[&str] = &[
    "Chromium Default",
    "Firefox Default",
    "Chromium Private Memory",
    "LibreWolf Private Memory",
    "Discord",
    "Telegram",
];

pub(crate) fn is_builtin_default_profile_name_impl(name: &str) -> bool {
    BUILTIN_DEFAULT_PROFILE_NAMES
        .iter()
        .any(|value| *value == name)
}

pub(crate) fn persist_hidden_default_profiles_store_impl(
    path: &PathBuf,
    store: &HiddenDefaultProfilesStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create hidden default profiles dir: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|e| format!("serialize hidden default profiles store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write hidden default profiles store: {e}"))
}

pub(crate) fn ensure_default_profiles_impl(
    manager: &ProfileManager,
    hidden_names: &BTreeSet<String>,
) -> Result<(), String> {
    let existing = manager.list_profiles().map_err(|e| e.to_string())?;
    if let Some(old_private) = existing
        .iter()
        .find(|profile| profile.name == "Firefox Private Memory")
    {
        let has_new_private = existing
            .iter()
            .any(|profile| profile.name == "LibreWolf Private Memory");
        if !has_new_private {
            manager
                .update_profile(
                    old_private.id,
                    PatchProfileInput {
                        name: Some("LibreWolf Private Memory".to_string()),
                        description: Some(Some(
                            "Ephemeral memory-only LibreWolf profile for private sessions."
                                .to_string(),
                        )),
                        tags: Some(vec![
                            "default".to_string(),
                            "private".to_string(),
                            "engine:librewolf".to_string(),
                        ]),
                        engine: Some(Engine::Librewolf),
                        ephemeral_mode: Some(true),
                        default_start_page: Some(Some("https://duckduckgo.com".to_string())),
                        default_search_provider: Some(Some("duckduckgo".to_string())),
                        ..PatchProfileInput::default()
                    },
                )
                .map_err(|e| e.to_string())?;
        }
    }
    let existing = manager.list_profiles().map_err(|e| e.to_string())?;
    let has = |name: &str| existing.iter().any(|p| p.name == name);
    if !hidden_names.contains("Chromium Default") && !has("Chromium Default") {
        manager
            .create_profile(CreateProfileInput {
                name: "Chromium Default".to_string(),
                description: Some("Default isolated Chromium profile (Chromium).".to_string()),
                tags: vec!["default".to_string(), "engine:chromium".to_string()],
                engine: Engine::Chromium,
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
    if !hidden_names.contains("Firefox Default") && !has("Firefox Default") {
        manager
            .create_profile(CreateProfileInput {
                name: "Firefox Default".to_string(),
                description: Some("Default isolated Firefox profile (Firefox ESR).".to_string()),
                tags: vec!["default".to_string(), "engine:firefox-esr".to_string()],
                engine: Engine::FirefoxEsr,
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
    if !hidden_names.contains("Chromium Private Memory") && !has("Chromium Private Memory") {
        manager
            .create_profile(CreateProfileInput {
                name: "Chromium Private Memory".to_string(),
                description: Some(
                    "Ephemeral memory-only Chromium profile for private sessions.".to_string(),
                ),
                tags: vec![
                    "default".to_string(),
                    "private".to_string(),
                    "engine:chromium".to_string(),
                ],
                engine: Engine::Chromium,
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
    if !hidden_names.contains("LibreWolf Private Memory") && !has("LibreWolf Private Memory") {
        manager
            .create_profile(CreateProfileInput {
                name: "LibreWolf Private Memory".to_string(),
                description: Some(
                    "Ephemeral memory-only LibreWolf profile for private sessions.".to_string(),
                ),
                tags: vec![
                    "default".to_string(),
                    "private".to_string(),
                    "engine:librewolf".to_string(),
                ],
                engine: Engine::Librewolf,
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
    if !hidden_names.contains("Discord") && !has("Discord") {
        manager
            .create_profile(CreateProfileInput {
                name: "Discord".to_string(),
                description: Some(
                    "Strict Discord app window without a free address bar.".to_string(),
                ),
                tags: vec![
                    "default".to_string(),
                    "engine:chromium".to_string(),
                    "locked-app:discord".to_string(),
                ],
                engine: Engine::Chromium,
                default_start_page: Some("https://discord.com/app".to_string()),
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
    if !hidden_names.contains("Telegram") && !has("Telegram") {
        manager
            .create_profile(CreateProfileInput {
                name: "Telegram".to_string(),
                description: Some(
                    "Strict Telegram app window without a free address bar.".to_string(),
                ),
                tags: vec![
                    "default".to_string(),
                    "engine:chromium".to_string(),
                    "locked-app:telegram".to_string(),
                ],
                engine: Engine::Chromium,
                default_start_page: Some("https://web.telegram.org/".to_string()),
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
    Ok(())
}
