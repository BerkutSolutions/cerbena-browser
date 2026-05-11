use super::*;
use super::transfer_export as export_owner;
use super::transfer_import_manifest as import_manifest_owner;

pub(crate) fn import_extension_library_item_cmd(
    state: State<AppState>,
    request: ImportExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<String>, String> {
    let assigned_profile_ids = request.assigned_profile_ids.clone();
    let imported_ids = import_extension_library_item_impl(&state, request)?;
    for extension_id in &imported_ids {
        if !assigned_profile_ids.is_empty() {
            profile_extensions::set_library_item_profile_assignments(
                state.inner(),
                extension_id,
                &assigned_profile_ids,
            )?;
        }
    }
    Ok(ok(
        correlation_id,
        if imported_ids.len() == 1 {
            imported_ids.into_iter().next().unwrap_or_default()
        } else {
            format!("imported:{}", imported_ids.len())
        },
    ))
}

pub(crate) fn export_extension_library_cmd(
    state: State<AppState>,
    request: TransferExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<TransferExtensionLibraryResponse>, String> {
    let directory = pick_folder()?;
    let response = match normalize_transfer_mode(&request.mode)? {
        TransferMode::File => export_extension_links_file(&state, &directory)?,
        TransferMode::Archive => export_extension_archive_folder(&state, &directory)?,
    };
    Ok(ok(correlation_id, response))
}

pub(crate) fn import_extension_library_cmd(
    state: State<AppState>,
    request: TransferExtensionLibraryRequest,
    correlation_id: String,
) -> Result<UiEnvelope<TransferExtensionLibraryResponse>, String> {
    let mode = normalize_transfer_mode(&request.mode)?;
    let selection = pick_import_source(mode)?;
    let response = match mode {
        TransferMode::File => import_extension_links_file(&state, &selection)?,
        TransferMode::Archive => import_extension_archive_folder(&state, &selection)?,
    };
    Ok(ok(correlation_id, response))
}

pub(crate) fn import_extension_library_item_impl(
    state: &AppState,
    request: ImportExtensionLibraryRequest,
) -> Result<Vec<String>, String> {
    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let normalized_store_url = request
        .store_url
        .clone()
        .filter(|value| !value.trim().is_empty());
    let package_metadata_batch =
        derive_extension_metadata_batch(&request, normalized_store_url.as_deref())?;
    let display_name_override = request
        .display_name
        .clone()
        .filter(|value| !value.trim().is_empty());
    let version_override = request.version.clone().filter(|value| !value.trim().is_empty());
    let engine_scope_override = request
        .engine_scope
        .clone()
        .filter(|value| !value.trim().is_empty());
    let logo_url_override = request
        .logo_url
        .clone()
        .filter(|value| !value.trim().is_empty());
    let normalized_tags = normalize_tags(request.tags.clone().unwrap_or_default());
    let is_single_import = package_metadata_batch.len() == 1;
    let mut imported_ids = Vec::new();
    let normalized_store_url = normalized_store_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let logo_url_override = logo_url_override
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    for package_metadata in package_metadata_batch {
        let seed = display_name_override
            .as_deref()
            .filter(|_| imported_ids.is_empty())
            .or(package_metadata.stable_id.as_deref())
            .or(package_metadata.display_name.as_deref())
            .or(normalized_store_url.as_deref())
            .unwrap_or(&request.source_value);
        let inferred_name = display_name_override
            .clone()
            .filter(|_| is_single_import)
            .or(package_metadata.display_name.clone())
            .unwrap_or_else(|| infer_extension_name(normalized_store_url, &request.source_value));
        let inferred_engine = engine_scope_override
            .clone()
            .or(package_metadata.engine_scope.clone())
            .unwrap_or_else(|| infer_engine_scope(normalized_store_url, &request.source_value));
        validate_assigned_profiles(state, &inferred_engine, &request.assigned_profile_ids)?;
        let merge_target_id =
            find_merge_target_extension_id(&library, &inferred_name, &inferred_engine);
        let persist_id = merge_target_id
            .clone()
            .unwrap_or_else(|| build_extension_id(seed, &library));
        let (package_path, package_file_name) = persist_extension_package(
            state,
            &persist_id,
            package_metadata.package_bytes.as_deref(),
            package_metadata.package_extension.as_deref(),
            package_metadata.package_file_name.as_deref(),
        )?;
        let version = version_override
            .clone()
            .filter(|_| is_single_import)
            .or(package_metadata.version.clone())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
        let variant = build_package_variant(
            &request,
            &package_metadata,
            &inferred_engine,
            &version,
            package_path,
            package_file_name,
            normalized_store_url,
            logo_url_override,
        );
        if let Some(existing_id) = merge_target_id {
            let item = library
                .items
                .get_mut(&existing_id)
                .ok_or_else(|| "extension not found during merge".to_string())?;
            if !display_name_override
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
                || item.display_name.trim().is_empty()
            {
                item.display_name = inferred_name.clone();
            }
            item.tags = normalize_tags([item.tags.clone(), normalized_tags.clone()].concat());
            item.assigned_profile_ids = {
                let mut merged = item.assigned_profile_ids.clone();
                merged.extend(request.assigned_profile_ids.clone());
                merged.sort();
                merged.dedup();
                merged
            };
            item.auto_update_enabled = request
                .auto_update_enabled
                .unwrap_or(item.auto_update_enabled);
            item.preserve_on_panic_wipe = request
                .preserve_on_panic_wipe
                .unwrap_or(item.preserve_on_panic_wipe);
            item.protect_data_from_panic_wipe = request
                .protect_data_from_panic_wipe
                .unwrap_or(item.protect_data_from_panic_wipe);
            let mut variants = normalized_extension_variants(item);
            variants.retain(|existing| {
                normalize_engine_scope(&existing.engine_scope)
                    != normalize_engine_scope(&variant.engine_scope)
            });
            variants.push(variant);
            item.package_variants = variants;
            sync_extension_item_legacy_fields(item);
            imported_ids.push(existing_id);
            continue;
        }

        let id = persist_id;
        let mut item = ExtensionLibraryItem {
            id: id.clone(),
            display_name: inferred_name,
            version,
            engine_scope: inferred_engine,
            source_kind: request.source_kind.clone(),
            source_value: request.source_value.clone(),
            logo_url: logo_url_override
                .map(str::to_string)
                .or(package_metadata.logo_url),
            store_url: normalized_store_url.map(str::to_string),
            tags: normalized_tags.clone(),
            assigned_profile_ids: request.assigned_profile_ids.clone(),
            auto_update_enabled: request.auto_update_enabled.unwrap_or(false),
            preserve_on_panic_wipe: request.preserve_on_panic_wipe.unwrap_or(false),
            protect_data_from_panic_wipe: request.protect_data_from_panic_wipe.unwrap_or(false),
            package_path: None,
            package_file_name: None,
            package_variants: vec![variant],
        };
        sync_extension_item_legacy_fields(&mut item);
        library.items.insert(id.clone(), item);
        imported_ids.push(id);
    }
    persist_library(state, &library)?;
    Ok(imported_ids)
}

fn export_extension_links_file(
    state: &AppState,
    directory: &str,
) -> Result<TransferExtensionLibraryResponse, String> {
    export_owner::export_extension_links_file_impl(state, directory)
}

fn export_extension_archive_folder(
    state: &AppState,
    directory: &str,
) -> Result<TransferExtensionLibraryResponse, String> {
    export_owner::export_extension_archive_folder_impl(state, directory)
}

fn import_extension_links_file(
    state: &AppState,
    manifest_file: &str,
) -> Result<TransferExtensionLibraryResponse, String> {
    let manifest_path = PathBuf::from(manifest_file);
    let manifest = read_transfer_manifest(&manifest_path)?;
    let base_dir = manifest_path
        .parent()
        .ok_or_else(|| "import file parent directory could not be resolved".to_string())?;
    import_transfer_manifest(state, &manifest, base_dir, TransferMode::File)
}

fn import_extension_archive_folder(
    state: &AppState,
    manifest_file: &str,
) -> Result<TransferExtensionLibraryResponse, String> {
    let chosen = PathBuf::from(manifest_file);
    let archive_root = chosen
        .parent()
        .ok_or_else(|| "archive manifest parent directory could not be resolved".to_string())?
        .to_path_buf();
    let manifest_path = archive_root.join(EXTENSION_ARCHIVE_MANIFEST_FILE_NAME);
    let manifest = read_transfer_manifest(&manifest_path)?;
    import_transfer_manifest(state, &manifest, &archive_root, TransferMode::Archive)
}

fn read_transfer_manifest(path: &Path) -> Result<ExtensionLibraryTransferManifest, String> {
    let bytes = fs::read(path).map_err(|e| format!("read manifest {}: {e}", path.display()))?;
    serde_json::from_slice::<ExtensionLibraryTransferManifest>(&bytes)
        .map_err(|e| format!("parse manifest {}: {e}", path.display()))
}

fn import_transfer_manifest(
    state: &AppState,
    manifest: &ExtensionLibraryTransferManifest,
    base_dir: &Path,
    mode: TransferMode,
) -> Result<TransferExtensionLibraryResponse, String> {
    import_manifest_owner::import_transfer_manifest_impl(state, manifest, base_dir, mode)
}

pub(crate) fn transfer_item_from_library_item(
    item: ExtensionLibraryItem,
    variants: Vec<ExtensionLibraryTransferVariant>,
) -> ExtensionLibraryTransferItem {
    ExtensionLibraryTransferItem {
        display_name: item.display_name,
        version: item.version,
        engine_scope: item.engine_scope,
        source_kind: item.source_kind,
        source_value: item.source_value,
        logo_url: item.logo_url,
        store_url: item.store_url,
        tags: normalize_tags(item.tags),
        auto_update_enabled: item.auto_update_enabled,
        preserve_on_panic_wipe: item.preserve_on_panic_wipe,
        protect_data_from_panic_wipe: item.protect_data_from_panic_wipe,
        package_file_name: item.package_file_name,
        package_relative_path: variants
            .first()
            .and_then(|variant| variant.package_relative_path.clone()),
        variants,
    }
}

pub(crate) fn build_import_requests_from_transfer_item(
    item: &ExtensionLibraryTransferItem,
    base_dir: &Path,
    mode: TransferMode,
) -> Result<Vec<ImportExtensionLibraryRequest>, String> {
    let variants = if item.variants.is_empty() {
        vec![ExtensionLibraryTransferVariant {
            engine_scope: item.engine_scope.clone(),
            version: item.version.clone(),
            source_kind: item.source_kind.clone(),
            source_value: item.source_value.clone(),
            logo_url: item.logo_url.clone(),
            store_url: item.store_url.clone(),
            package_file_name: item.package_file_name.clone(),
            package_relative_path: item.package_relative_path.clone(),
        }]
    } else {
        item.variants.clone()
    };

    let mut requests = Vec::new();
    for variant in variants {
        let package_bytes_base64 = match mode {
            TransferMode::Archive => match variant.package_relative_path.as_deref() {
                Some(relative_path) => {
                    let package_path = base_dir.join(relative_path.replace('/', "\\"));
                    if !package_path.is_file() {
                        None
                    } else {
                        let bytes = fs::read(&package_path)
                            .map_err(|e| format!("read package {}: {e}", package_path.display()))?;
                        Some(BASE64_STANDARD.encode(bytes))
                    }
                }
                None => None,
            },
            TransferMode::File => None,
        };

        let source_kind = if package_bytes_base64.is_some() {
            "local_file".to_string()
        } else {
            variant.source_kind.clone()
        };
        let source_value = if package_bytes_base64.is_some() {
            variant
                .package_file_name
                .clone()
                .unwrap_or_else(|| item.display_name.clone())
        } else if let Some(store_url) = variant
            .store_url
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            store_url
        } else if variant.source_value.trim().starts_with("http://")
            || variant.source_value.trim().starts_with("https://")
        {
            variant.source_value.clone()
        } else if !variant.source_value.trim().is_empty() {
            // Preserve non-URL store/source identifiers from link manifests
            // instead of silently skipping them during import.
            variant.source_value.clone()
        } else {
            continue;
        };

        requests.push(ImportExtensionLibraryRequest {
            source_kind,
            source_value: source_value.clone(),
            store_url: variant.store_url.clone(),
            display_name: Some(item.display_name.clone()),
            version: Some(variant.version.clone()),
            logo_url: variant.logo_url.clone().or_else(|| item.logo_url.clone()),
            engine_scope: Some(variant.engine_scope.clone()),
            tags: Some(item.tags.clone()),
            assigned_profile_ids: Vec::new(),
            auto_update_enabled: Some(item.auto_update_enabled),
            preserve_on_panic_wipe: Some(item.preserve_on_panic_wipe),
            protect_data_from_panic_wipe: Some(item.protect_data_from_panic_wipe),
            package_file_name: variant.package_file_name.clone(),
            package_bytes_base64,
        });
    }

    Ok(requests)
}
