use super::{
    store_load, AppUpdateStore, ExtensionLibraryStore, HiddenDefaultProfilesStore,
    IdentityStore, LinkRoutingStore, NetworkStore, PathBuf, SyncStore,
};

pub(crate) fn load_identity_store(path: &PathBuf) -> Result<IdentityStore, String> {
    store_load::load_identity_store(path)
}

pub(crate) fn load_network_store(path: &PathBuf) -> Result<NetworkStore, String> {
    store_load::load_network_store(path)
}

pub(crate) fn load_extension_library_store(path: &PathBuf) -> Result<ExtensionLibraryStore, String> {
    store_load::load_extension_library_store(path)
}

pub(crate) fn load_app_update_store(path: &PathBuf) -> Result<AppUpdateStore, String> {
    store_load::load_app_update_store(path)
}

pub(crate) fn load_hidden_default_profiles_store(
    path: &PathBuf,
) -> Result<HiddenDefaultProfilesStore, String> {
    store_load::load_hidden_default_profiles_store(path)
}

pub(crate) fn load_sync_store(path: &PathBuf, secret_material: &str) -> Result<SyncStore, String> {
    store_load::load_sync_store(path, secret_material)
}

pub(crate) fn load_link_routing_store(
    path: &PathBuf,
    secret_material: &str,
) -> Result<LinkRoutingStore, String> {
    store_load::load_link_routing_store(path, secret_material)
}

