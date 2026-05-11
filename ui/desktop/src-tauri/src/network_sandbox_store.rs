use super::*;
use std::fs;

pub(crate) fn load_network_sandbox_store_impl(path: &PathBuf) -> Result<NetworkSandboxStore, String> {
    if !path.exists() {
        return Ok(NetworkSandboxStore::default());
    }
    let raw = fs::read(path).map_err(|e| format!("read network sandbox store: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("parse network sandbox store: {e}"))
}

pub(crate) fn migrate_network_sandbox_store_impl(
    store: &mut NetworkSandboxStore,
    network_store: &NetworkStore,
) -> Result<bool, String> {
    normalize_global_settings(&mut store.global);
    let mut changed = false;
    let mut profile_keys = network_store.vpn_proxy.keys().cloned().collect::<Vec<_>>();
    for key in network_store.profile_template_selection.keys() {
        if !profile_keys.iter().any(|item| item == key) {
            profile_keys.push(key.clone());
        }
    }
    for profile_key in profile_keys {
        let entry = store.profiles.entry(profile_key.clone()).or_default();
        let legacy_native =
            profile_requires_legacy_native_compatibility(network_store, &profile_key)?;
        if legacy_native && entry.preferred_mode.is_none() {
            entry.preferred_mode = Some(MODE_COMPAT_NATIVE.to_string());
            entry.migrated_legacy_native = true;
            entry.last_resolved_mode = Some(MODE_COMPAT_NATIVE.to_string());
            entry.last_resolution_reason =
                Some("Adapted from a pre-sandbox AmneziaWG profile".to_string());
            changed = true;
        } else if entry.preferred_mode.is_some() {
            let normalized = normalize_mode(entry.preferred_mode.clone());
            if entry.preferred_mode != normalized {
                entry.preferred_mode = normalized;
                changed = true;
            }
        }
    }
    Ok(changed)
}
