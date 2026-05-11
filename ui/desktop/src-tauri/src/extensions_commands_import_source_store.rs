use super::*;

pub(crate) fn derive_store_url_metadata_batch(
    request: &ImportExtensionLibraryRequest,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    download_store_extension_metadata(store_url.unwrap_or(request.source_value.as_str()))
        .map(|metadata| vec![metadata])
}
