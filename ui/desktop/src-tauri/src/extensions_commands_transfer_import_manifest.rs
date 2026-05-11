use super::*;

pub(crate) fn import_transfer_manifest_impl(
    state: &AppState,
    manifest: &ExtensionLibraryTransferManifest,
    base_dir: &Path,
    mode: TransferMode,
) -> Result<TransferExtensionLibraryResponse, String> {
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();

    for item in &manifest.items {
        let import_requests =
            match transfer::build_import_requests_from_transfer_item(item, base_dir, mode) {
                Ok(requests) if requests.is_empty() => {
                    skipped += 1;
                    continue;
                }
                Ok(requests) => requests,
                Err(error) => {
                    errors.push(format!("{}: {error}", item.display_name));
                    skipped += 1;
                    continue;
                }
            };
        let mut item_imported = false;
        for import_request in import_requests {
            match transfer::import_extension_library_item_impl(state, import_request) {
                Ok(ids) if !ids.is_empty() => item_imported = true,
                Ok(_) => {}
                Err(error) => errors.push(format!("{}: {error}", item.display_name)),
            }
        }
        if item_imported {
            imported += 1;
        }
    }

    Ok(TransferExtensionLibraryResponse {
        directory: base_dir.to_string_lossy().to_string(),
        file_name: Some(match mode {
            TransferMode::File => EXTENSION_LINKS_FILE_NAME.to_string(),
            TransferMode::Archive => EXTENSION_ARCHIVE_MANIFEST_FILE_NAME.to_string(),
        }),
        imported,
        exported: 0,
        skipped,
        errors,
    })
}
