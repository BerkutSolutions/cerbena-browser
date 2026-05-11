use super::*;

pub(crate) fn load_json_with_default<T, F>(path: &PathBuf, default_fn: F) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    F: FnOnce() -> T,
{
    if !path.exists() {
        return Ok(default_fn());
    }
    let raw = fs::read(path).map_err(|e| format!("read store {}: {e}", path.display()))?;
    serde_json::from_slice(&raw).map_err(|e| format!("parse store {}: {e}", path.display()))
}

pub(crate) fn load_identity_store(path: &PathBuf) -> Result<IdentityStore, String> {
    load_json_with_default(path, IdentityStore::default)
}

pub(crate) fn load_network_store(path: &PathBuf) -> Result<NetworkStore, String> {
    load_json_with_default(path, NetworkStore::default)
}

pub(crate) fn load_extension_library_store(path: &PathBuf) -> Result<ExtensionLibraryStore, String> {
    load_json_with_default(path, ExtensionLibraryStore::default)
}

pub(crate) fn load_app_update_store(path: &PathBuf) -> Result<AppUpdateStore, String> {
    load_json_with_default(path, AppUpdateStore::default)
}

pub(crate) fn load_hidden_default_profiles_store(
    path: &PathBuf,
) -> Result<HiddenDefaultProfilesStore, String> {
    load_json_with_default(path, HiddenDefaultProfilesStore::default)
}

pub(crate) fn load_sync_store(path: &PathBuf, secret_material: &str) -> Result<SyncStore, String> {
    crate::sensitive_store::load_sensitive_json_or_default(path, "sync-store", secret_material)
}

pub(crate) fn load_link_routing_store(
    path: &PathBuf,
    secret_material: &str,
) -> Result<LinkRoutingStore, String> {
    crate::sensitive_store::load_sensitive_json_or_default(path, "link-routing-store", secret_material)
}
