use super::*;

pub(crate) fn derive_local_file_metadata_batch(
    request: &ImportExtensionLibraryRequest,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    if let Some(raw_bytes) = request
        .package_bytes_base64
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let bytes = BASE64_STANDARD
            .decode(raw_bytes)
            .map_err(|e| format!("decode extension package: {e}"))?;
        let file_name = request
            .package_file_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&request.source_value);
        return read_extension_archive_metadata_batch_from_bytes(&bytes, file_name, store_url);
    }

    let path = Path::new(&request.source_value);
    if path.exists() {
        return read_extension_archive_metadata_batch(path, store_url);
    }

    Ok(vec![])
}
