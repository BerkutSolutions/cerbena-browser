use super::*;

pub(super) fn apply_snapshot_material_impl(
    state: &AppState,
    profile_id: Uuid,
    material: &SnapshotMaterial,
    selected_paths: &[String],
    scope: RestoreScope,
) -> Result<(), String> {
    let profile_key = profile_id.to_string();
    let restore_config = selected_paths
        .iter()
        .any(|path| path == "profile/config.json");
    let restore_identity = selected_paths
        .iter()
        .any(|path| path == "identity/preset.json");
    let restore_dns = selected_paths.iter().any(|path| path == "network/dns.json");
    let restore_vpn = selected_paths
        .iter()
        .any(|path| path == "network/vpn_proxy.json");
    let restore_sync_controls = selected_paths
        .iter()
        .any(|path| path == "sync/controls.json");
    let restore_sync_conflicts = selected_paths
        .iter()
        .any(|path| path == "sync/conflicts.json");
    let restore_files = selected_paths
        .iter()
        .filter_map(|path| path.strip_prefix("files/").map(str::to_string))
        .collect::<Vec<_>>();

    if restore_config {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "manager lock poisoned".to_string())?;
        manager
            .update_profile_with_actor(
                profile_id,
                PatchProfileInput {
                    name: Some(material.profile.name.clone()),
                    description: Some(material.profile.description.clone()),
                    tags: Some(material.profile.tags.clone()),
                    default_start_page: Some(material.profile.default_start_page.clone()),
                    default_search_provider: Some(material.profile.default_search_provider.clone()),
                    ephemeral_mode: Some(material.profile.ephemeral_mode),
                    panic_frame_enabled: Some(material.profile.panic_frame_enabled),
                    panic_frame_color: None,
                    panic_protected_sites: Some(material.profile.panic_protected_sites.clone()),
                    ephemeral_retain_paths: Some(material.profile.ephemeral_retain_paths.clone()),
                    ..PatchProfileInput::default()
                },
                None,
                "sync-restore",
            )
            .map_err(|e| e.to_string())?;
    }

    if restore_identity {
        let mut store = state
            .identity_store
            .lock()
            .map_err(|_| "identity store lock poisoned".to_string())?;
        match &material.identity_preset {
            Some(preset) => {
                store.items.insert(profile_key.clone(), preset.clone());
            }
            None => {
                store.items.remove(&profile_key);
            }
        }
        let path = state.identity_store_path(&state.app_handle)?;
        persist_identity_store(&path, &store)?;
    }

    if restore_dns || restore_vpn {
        let mut store = state
            .network_store
            .lock()
            .map_err(|_| "network store lock poisoned".to_string())?;
        if restore_dns {
            match &material.dns_policy {
                Some(policy) => {
                    store.dns.insert(profile_key.clone(), policy.clone());
                }
                None => {
                    store.dns.remove(&profile_key);
                }
            }
        }
        if restore_vpn {
            match &material.vpn_proxy_policy {
                Some(policy) => {
                    store.vpn_proxy.insert(profile_key.clone(), policy.clone());
                }
                None => {
                    store.vpn_proxy.remove(&profile_key);
                }
            }
        }
        let path = state.network_store_path(&state.app_handle)?;
        persist_network_store(&path, &store)?;
    }

    if restore_sync_controls || restore_sync_conflicts {
        let mut store = state
            .sync_store
            .lock()
            .map_err(|_| "sync store lock poisoned".to_string())?;
        if restore_sync_controls {
            match &material.sync_controls {
                Some(controls) => {
                    store.controls.insert(profile_key.clone(), controls.clone());
                }
                None => {
                    store.controls.remove(&profile_key);
                }
            }
        }
        if restore_sync_conflicts {
            if material.sync_conflicts.is_empty() {
                store.conflicts.remove(&profile_key);
            } else {
                store
                    .conflicts
                    .insert(profile_key.clone(), material.sync_conflicts.clone());
            }
        }
        let path = state.sync_store_path(&state.app_handle)?;
        persist_sync_store_with_secret(&path, &state.sensitive_store_secret, &store)?;
    }

    if !restore_files.is_empty() {
        let data_root = state.profile_root.join(profile_key).join("data");
        if matches!(scope, RestoreScope::Full) {
            if data_root.exists() {
                fs::remove_dir_all(&data_root)
                    .map_err(|e| format!("clear profile data for restore: {e}"))?;
            }
            fs::create_dir_all(&data_root)
                .map_err(|e| format!("recreate profile data for restore: {e}"))?;
        }
        for entry in &material.files {
            if !restore_files
                .iter()
                .any(|path| path == &entry.relative_path)
            {
                continue;
            }
            let target = normalize_snapshot_file_target(&data_root, &entry.relative_path)?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("create restored parent dir: {e}"))?;
            }
            let bytes = B64
                .decode(entry.content_b64.trim())
                .map_err(|e| format!("decode restored file {}: {e}", entry.relative_path))?;
            fs::write(&target, bytes)
                .map_err(|e| format!("write restored file {}: {e}", target.display()))?;
        }
    }

    Ok(())
}

pub(crate) fn collect_profile_data_files(data_root: &Path) -> Result<Vec<SnapshotFileEntry>, String> {
    files::collect_profile_data_files(data_root)
}

pub(crate) fn normalize_snapshot_file_target(
    data_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, String> {
    files::normalize_snapshot_file_target(data_root, relative_path)
}

