use super::*;

pub(crate) fn persist_identity_store_impl(
    path: &PathBuf,
    store: &IdentityStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create identity dir: {e}"))?;
    }
    let bytes =
        serde_json::to_vec_pretty(store).map_err(|e| format!("serialize identity store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write identity store: {e}"))
}

pub(crate) fn persist_network_store_impl(
    path: &PathBuf,
    store: &NetworkStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create network dir: {e}"))?;
    }
    let bytes =
        serde_json::to_vec_pretty(store).map_err(|e| format!("serialize network store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write network store: {e}"))
}

pub(crate) fn persist_network_sandbox_store_impl(
    path: &PathBuf,
    store: &NetworkSandboxStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create network sandbox dir: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|e| format!("serialize network sandbox store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write network sandbox store: {e}"))
}

pub(crate) fn persist_extension_library_store_impl(
    path: &PathBuf,
    store: &ExtensionLibraryStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create extension library dir: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|e| format!("serialize extension library: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write extension library: {e}"))
}

pub(crate) fn persist_app_update_store_impl(
    path: &PathBuf,
    store: &AppUpdateStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create app update dir: {e}"))?;
    }
    let bytes =
        serde_json::to_vec_pretty(store).map_err(|e| format!("serialize app update store: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write app update store: {e}"))
}

pub(crate) fn persist_sync_store_with_secret_impl(
    path: &PathBuf,
    secret_material: &str,
    store: &SyncStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create sync dir: {e}"))?;
    }
    crate::sensitive_store::persist_sensitive_json(path, "sync-store", secret_material, store)
}

pub(crate) fn persist_link_routing_store_with_secret_impl(
    path: &PathBuf,
    secret_material: &str,
    store: &LinkRoutingStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create link routing dir: {e}"))?;
    }
    crate::sensitive_store::persist_sensitive_json(
        path,
        "link-routing-store",
        secret_material,
        store,
    )
}
