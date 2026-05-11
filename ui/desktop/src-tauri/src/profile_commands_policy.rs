use super::*;

pub(crate) fn write_locked_app_policy_impl(
    profile: &ProfileMetadata,
    profile_root: &Path,
) -> Result<(), std::io::Error> {
    let path = profile_root.join("policy").join("locked-app.json");
    if let Some(policy) = locked_app_policy_for_profile_impl(profile) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&policy)
            .map_err(std::io::Error::other)?;
        fs::write(&path, bytes)?;
    } else if path.exists() {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

pub(crate) fn write_profile_identity_policy_impl(
    state: &AppState,
    profile_id: Uuid,
    profile_root: &Path,
) -> Result<Option<String>, std::io::Error> {
    let path = profile_root.join("policy").join("identity-preset.json");
    let profile_key = profile_id.to_string();
    let preset = state
        .identity_store
        .lock()
        .ok()
        .and_then(|store| store.items.get(&profile_key).cloned());
    if let Some(preset) = preset {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&preset)
            .map_err(std::io::Error::other)?;
        fs::write(&path, bytes)?;
        return Ok(Some(identity_policy_hash_bytes(&fs::read(&path)?)));
    } else if path.exists() {
        let _ = fs::remove_file(path);
    }
    Ok(None)
}

pub(crate) fn should_restart_for_identity_policy_impl(
    profile_root: &Path,
    engine: &str,
    expected_hash: Option<&str>,
) -> bool {
    let Some(expected_hash) = expected_hash else {
        return false;
    };
    let Ok(raw) = fs::read(identity_applied_marker_path(profile_root)) else {
        return true;
    };
    let Ok(marker) = serde_json::from_slice::<IdentityAppliedMarker>(&raw) else {
        return true;
    };
    marker.engine != engine || marker.identity_hash != expected_hash
}

pub(crate) fn persist_identity_applied_marker_impl(
    profile_root: &Path,
    engine: &str,
    identity_hash: Option<&str>,
) -> Result<(), std::io::Error> {
    let path = identity_applied_marker_path(profile_root);
    let Some(identity_hash) = identity_hash else {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let marker = IdentityAppliedMarker {
        engine: engine.to_string(),
        identity_hash: identity_hash.to_string(),
    };
    let bytes = serde_json::to_vec_pretty(&marker)
        .map_err(std::io::Error::other)?;
    fs::write(path, bytes)
}

pub(crate) fn locked_app_policy_for_profile_impl(
    profile: &ProfileMetadata,
) -> Option<LockedAppPolicyRecord> {
    if profile
        .tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case("locked-app:discord"))
    {
        return Some(LockedAppPolicyRecord {
            start_url: "https://discord.com/app".to_string(),
            allowed_hosts: vec![
                "discord.com".to_string(),
                "discord.gg".to_string(),
                "discordapp.com".to_string(),
                "discordapp.net".to_string(),
                "discord.media".to_string(),
            ],
        });
    }
    if profile
        .tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case("locked-app:telegram"))
    {
        return Some(LockedAppPolicyRecord {
            start_url: "https://web.telegram.org/".to_string(),
            allowed_hosts: vec![
                "web.telegram.org".to_string(),
                "telegram.org".to_string(),
                "t.me".to_string(),
                "telegram.me".to_string(),
            ],
        });
    }
    if profile
        .tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case("locked-app:custom"))
    {
        let start_url = normalize_start_page_url(profile.default_start_page.as_deref());
        let parsed = reqwest::Url::parse(&start_url).ok()?;
        let host = parsed.host_str()?.trim().to_ascii_lowercase();
        if host.is_empty() {
            return None;
        }
        return Some(LockedAppPolicyRecord {
            start_url,
            allowed_hosts: vec![host],
        });
    }
    None
}

fn identity_policy_hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn identity_applied_marker_path(profile_root: &Path) -> PathBuf {
    profile_root.join("policy").join("identity-applied.json")
}
