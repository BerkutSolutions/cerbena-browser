use super::*;

pub(crate) fn refresh_extension_library_updates_impl(
    state: &AppState,
    profile_filter: Option<&str>,
) -> Result<RefreshExtensionLibraryUpdatesResponse, String> {
    let snapshot = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?
        .clone();
    if !snapshot.auto_update_enabled {
        return Ok(RefreshExtensionLibraryUpdatesResponse {
            checked: 0,
            updated: 0,
            skipped: snapshot.items.len(),
            errors: Vec::new(),
        });
    }

    let items = snapshot
        .items
        .values()
        .filter(|item| item.auto_update_enabled)
        .filter(|item| {
            item.store_url
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
        })
        .filter(|item| {
            profile_filter
                .map(|profile_id| item.assigned_profile_ids.iter().any(|id| id == profile_id))
                .unwrap_or(true)
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut summary = RefreshExtensionLibraryUpdatesResponse {
        checked: items.len(),
        updated: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    for item in items {
        match refresh_extension_library_item(state, &item.id) {
            Ok(updated) => {
                if updated {
                    summary.updated += 1;
                } else {
                    summary.skipped += 1;
                }
            }
            Err(error) => summary.errors.push(format!("{}: {error}", item.display_name)),
        }
    }

    Ok(summary)
}

fn refresh_extension_library_item(state: &AppState, extension_id: &str) -> Result<bool, String> {
    let item_snapshot = {
        let library = state
            .extension_library
            .lock()
            .map_err(|_| "extension library lock poisoned".to_string())?;
        library
            .items
            .get(extension_id)
            .cloned()
            .ok_or_else(|| "extension not found".to_string())?
    };
    let mut refreshed_variants = Vec::new();
    let mut refreshed_any = false;
    for variant in normalized_extension_variants(&item_snapshot) {
        let store_url = variant
            .store_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(store_url) = store_url {
            let metadata = download_store_extension_metadata(store_url)?;
            let (package_path, package_file_name) = persist_extension_package(
                state,
                &item_snapshot.id,
                metadata.package_bytes.as_deref(),
                metadata.package_extension.as_deref(),
                metadata.package_file_name.as_deref(),
            )?;
            let engine_scope = metadata
                .engine_scope
                .clone()
                .unwrap_or_else(|| variant.engine_scope.clone());
            refreshed_variants.push(ExtensionPackageVariant {
                engine_scope,
                version: metadata
                    .version
                    .clone()
                    .unwrap_or_else(|| variant.version.clone()),
                source_kind: variant.source_kind.clone(),
                source_value: variant.source_value.clone(),
                logo_url: metadata.logo_url.clone().or_else(|| variant.logo_url.clone()),
                store_url: Some(store_url.to_string()),
                package_path: package_path.or_else(|| variant.package_path.clone()),
                package_file_name: package_file_name.or_else(|| variant.package_file_name.clone()),
            });
            refreshed_any = true;
        } else {
            refreshed_variants.push(variant);
        }
    }
    if !refreshed_any {
        return Err("store URL is not configured".to_string());
    }

    let mut library = state
        .extension_library
        .lock()
        .map_err(|_| "extension library lock poisoned".to_string())?;
    let item = library
        .items
        .get_mut(extension_id)
        .ok_or_else(|| "extension not found".to_string())?;

    let mut changed = false;
    if normalized_extension_variants(item) != refreshed_variants {
        item.package_variants = refreshed_variants;
        changed = true;
    }
    let before_display_name = item.display_name.clone();
    sync_extension_item_legacy_fields(item);
    if item.display_name != before_display_name {
        item.display_name = before_display_name;
    }

    if changed {
        persist_library(state, &library)?;
    }
    Ok(changed)
}
