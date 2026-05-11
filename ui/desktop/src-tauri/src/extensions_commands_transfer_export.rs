use super::*;

pub(crate) fn export_extension_links_file_impl(
    state: &AppState,
    directory: &str,
) -> Result<TransferExtensionLibraryResponse, String> {
    let library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?
        .clone();
    let mut skipped = 0usize;
    let manifest = ExtensionLibraryTransferManifest {
        version: 1,
        mode: "file".to_string(),
        items: library
            .items
            .values()
            .filter_map(|item| {
                let linkable = normalized_extension_variants(item).iter().any(|variant| {
                    variant
                        .store_url
                        .as_deref()
                        .map(str::trim)
                        .is_some_and(|value| !value.is_empty())
                        || variant.source_value.trim().starts_with("http://")
                        || variant.source_value.trim().starts_with("https://")
                });
                if !linkable {
                    skipped += 1;
                    return None;
                }
                Some(transfer::transfer_item_from_library_item(
                    item.clone(),
                    normalized_extension_variants(item)
                        .into_iter()
                        .map(|variant| ExtensionLibraryTransferVariant {
                            engine_scope: variant.engine_scope,
                            version: variant.version,
                            source_kind: variant.source_kind,
                            source_value: variant.source_value,
                            logo_url: variant.logo_url,
                            store_url: variant.store_url,
                            package_file_name: variant.package_file_name,
                            package_relative_path: None,
                        })
                        .collect(),
                ))
            })
            .collect(),
    };
    let target_dir = PathBuf::from(directory);
    fs::create_dir_all(&target_dir).map_err(|e| format!("create export dir: {e}"))?;
    let file_path = target_dir.join(EXTENSION_LINKS_FILE_NAME);
    let bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("serialize export: {e}"))?;
    fs::write(&file_path, bytes).map_err(|e| format!("write export file: {e}"))?;
    Ok(TransferExtensionLibraryResponse {
        directory: target_dir.to_string_lossy().to_string(),
        file_name: Some(EXTENSION_LINKS_FILE_NAME.to_string()),
        imported: 0,
        exported: manifest.items.len(),
        skipped,
        errors: Vec::new(),
    })
}

pub(crate) fn export_extension_archive_folder_impl(
    state: &AppState,
    directory: &str,
) -> Result<TransferExtensionLibraryResponse, String> {
    let library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?
        .clone();
    let target_root = PathBuf::from(directory).join(EXTENSION_ARCHIVE_DIR_NAME);
    let packages_dir = target_root.join(EXTENSION_ARCHIVE_PACKAGES_DIR_NAME);
    fs::create_dir_all(&packages_dir).map_err(|e| format!("create archive dir: {e}"))?;
    let mut errors = Vec::new();
    let mut manifest_items = Vec::new();

    for item in library.items.values() {
        let variant_transfers = normalized_extension_variants(item)
            .into_iter()
            .map(|variant| {
                let package_relative_path =
                    variant.package_path.as_deref().and_then(|package_path| {
                        let source_path = PathBuf::from(package_path);
                        if !source_path.is_file() {
                            errors.push(format!("{}: package file is missing", item.display_name));
                            return None;
                        }
                        let file_name = variant
                            .package_file_name
                            .clone()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or_else(|| {
                                source_path
                                    .file_name()
                                    .and_then(|value| value.to_str())
                                    .unwrap_or("extension.zip")
                                    .to_string()
                            });
                        let safe_name = sanitize_file_name(&file_name);
                        let unique_name = unique_archive_file_name(
                            &packages_dir,
                            &format!(
                                "{}-{}",
                                item.id,
                                normalize_engine_scope(&variant.engine_scope)
                            ),
                            &safe_name,
                        );
                        let dest_path = packages_dir.join(&unique_name);
                        match fs::copy(&source_path, &dest_path) {
                            Ok(_) => Some(format!(
                                "{EXTENSION_ARCHIVE_PACKAGES_DIR_NAME}/{unique_name}"
                            )),
                            Err(error) => {
                                errors.push(format!(
                                    "{}: copy package failed: {error}",
                                    item.display_name
                                ));
                                None
                            }
                        }
                    });
                ExtensionLibraryTransferVariant {
                    engine_scope: normalize_engine_scope(&variant.engine_scope),
                    version: variant.version,
                    source_kind: variant.source_kind,
                    source_value: variant.source_value,
                    logo_url: variant.logo_url,
                    store_url: variant.store_url,
                    package_file_name: variant.package_file_name,
                    package_relative_path,
                }
            })
            .collect::<Vec<_>>();
        manifest_items.push(transfer::transfer_item_from_library_item(
            item.clone(),
            variant_transfers,
        ));
    }

    let manifest = ExtensionLibraryTransferManifest {
        version: 1,
        mode: "archive".to_string(),
        items: manifest_items,
    };
    let manifest_path = target_root.join(EXTENSION_ARCHIVE_MANIFEST_FILE_NAME);
    let bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("serialize archive: {e}"))?;
    fs::write(&manifest_path, bytes).map_err(|e| format!("write archive manifest: {e}"))?;

    Ok(TransferExtensionLibraryResponse {
        directory: target_root.to_string_lossy().to_string(),
        file_name: Some(EXTENSION_ARCHIVE_MANIFEST_FILE_NAME.to_string()),
        imported: 0,
        exported: manifest.items.len(),
        skipped: 0,
        errors,
    })
}
