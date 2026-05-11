use super::*;

pub(crate) fn derive_local_folder_metadata_batch(
    request: &ImportExtensionLibraryRequest,
    store_url: Option<&str>,
) -> Result<Vec<DerivedExtensionMetadata>, String> {
    let lower_kind = request.source_kind.to_lowercase();
    let folder = if lower_kind == "local_folder_picker" {
        PathBuf::from(pick_folder()?)
    } else {
        PathBuf::from(request.source_value.trim())
    };
    read_extension_directory_metadata_batch(&folder, store_url)
}
