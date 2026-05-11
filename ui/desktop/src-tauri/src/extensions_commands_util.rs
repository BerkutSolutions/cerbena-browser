use super::*;
#[path = "extensions_commands_util_archive_io.rs"]
mod archive_io;
#[path = "extensions_commands_util_archive_manifest.rs"]
mod archive_manifest;
#[path = "extensions_commands_util_library.rs"]
mod library;
#[path = "extensions_commands_util_policy.rs"]
mod policy;
#[path = "extensions_commands_util_store.rs"]
mod store;
#[path = "extensions_commands_util_transfer.rs"]
mod transfer;

pub(super) fn normalize_transfer_mode(value: &str) -> Result<TransferMode, String> {
    transfer::normalize_transfer_mode(value)
}

pub(super) fn pick_folder() -> Result<String, String> {
    transfer::pick_folder()
}

pub(super) fn pick_import_source(mode: TransferMode) -> Result<String, String> {
    transfer::pick_import_source(mode)
}

pub(crate) use archive_io::{
    read_extension_archive_metadata_batch, read_extension_archive_metadata_batch_from_bytes, read_extension_directory_metadata_batch,
};
pub(crate) use policy::{find_merge_target_extension_id, normalize_engine_scope, validate_assigned_profiles};
pub(crate) use store::download_store_extension_metadata;
pub(crate) use self::transfer::{delete_extension_package, persist_extension_package, sanitize_file_name, unique_archive_file_name};

pub(crate) fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    library::normalize_tags_impl(tags)
}

pub(crate) fn persist_library(state: &AppState, store: &ExtensionLibraryStore) -> Result<(), String> {
    library::persist_library_impl(state, store)
}

pub(crate) fn build_extension_id(seed: &str, library: &ExtensionLibraryStore) -> String {
    library::build_extension_id_impl(seed, library)
}

pub(crate) fn infer_extension_name(store_url: Option<&str>, source_value: &str) -> String {
    library::infer_extension_name_impl(store_url, source_value)
}

pub(crate) fn infer_engine_scope(store_url: Option<&str>, source_value: &str) -> String {
    library::infer_engine_scope_impl(store_url, source_value)
}

#[derive(Default)]
pub(crate) struct DerivedExtensionMetadata {
    pub(crate) stable_id: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) engine_scope: Option<String>,
    pub(crate) logo_url: Option<String>,
    pub(crate) package_bytes: Option<Vec<u8>>,
    pub(crate) package_extension: Option<String>,
    pub(crate) package_file_name: Option<String>,
}

pub(crate) fn normalized_extension_variants(item: &ExtensionLibraryItem) -> Vec<ExtensionPackageVariant> {
    library::normalized_extension_variants_impl(item)
}

pub(crate) fn sync_extension_item_legacy_fields(item: &mut ExtensionLibraryItem) {
    library::sync_extension_item_legacy_fields_impl(item)
}

pub(crate) fn build_package_variant(
    request: &ImportExtensionLibraryRequest,
    metadata: &DerivedExtensionMetadata,
    engine_scope: &str,
    version: &str,
    package_path: Option<String>,
    package_file_name: Option<String>,
    normalized_store_url: Option<&str>,
    logo_url_override: Option<&str>,
) -> ExtensionPackageVariant {
    library::build_package_variant_impl(
        request,
        metadata,
        engine_scope,
        version,
        package_path,
        package_file_name,
        normalized_store_url,
        logo_url_override,
    )
}

pub(crate) fn derive_extension_metadata_batch(
    request: &ImportExtensionLibraryRequest,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    let lower_kind = request.source_kind.to_lowercase();
    if lower_kind == "store_url" {
        match import_source_store::derive_store_url_metadata_batch(request, store_url) {
            Ok(batch) => return Ok(batch),
            Err(_) => {
                // Keep transfer-manifest imports resilient when remote store metadata
                // is temporarily unavailable. We can still import using manifest fields.
                let display_name = request
                    .display_name
                    .clone()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                let version = request
                    .version
                    .clone()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                let engine_scope = request
                    .engine_scope
                    .clone()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .or_else(|| Some(infer_engine_scope(store_url, &request.source_value)));
                let logo_url = request
                    .logo_url
                    .clone()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                return Ok(vec![DerivedExtensionMetadata {
                    stable_id: archive_manifest::store_url_fallback_id(store_url),
                    display_name,
                    version,
                    engine_scope,
                    logo_url,
                    ..DerivedExtensionMetadata::default()
                }]);
            }
        }
    }

    if matches!(
        lower_kind.as_str(),
        "local_folder" | "local_folder_picker" | "dropped_folder"
    ) {
        return import_source_local_folder::derive_local_folder_metadata_batch(request, store_url);
    }

    if matches!(lower_kind.as_str(), "local_file" | "dropped_file") {
        let batch = import_source_local_file::derive_local_file_metadata_batch(request, store_url)?;
        if !batch.is_empty() {
            return Ok(batch);
        }
    }

    Ok(vec![DerivedExtensionMetadata {
        stable_id: archive_manifest::store_url_fallback_id(store_url),
        engine_scope: Some(infer_engine_scope(store_url, &request.source_value)),
        ..DerivedExtensionMetadata::default()
    }])
}
