use super::*;

pub(crate) fn reset_profile_runtime_workspace_impl(
    state: &State<'_, AppState>,
    profile_id: Uuid,
) -> Result<(), String> {
    stop_profile_network_stack(&state.app_handle, profile_id);
    let _ = revoke_launch_session(state.inner(), profile_id, None);
    if let Ok(mut launched) = state.launched_processes.lock() {
        launched.remove(&profile_id);
    }
    clear_librewolf_profile_certificates(&state.app_handle, profile_id);
    let profile_root = state.profile_root.join(profile_id.to_string());
    if !profile_root.exists() {
        return Ok(());
    }
    let keep = ["metadata.json", "lock_state.json"];
    let entries =
        fs::read_dir(&profile_root).map_err(|e| format!("read profile workspace: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read profile workspace entry: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if keep.iter().any(|value| value == &name) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .map_err(|e| format!("remove profile workspace dir {}: {e}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .map_err(|e| format!("remove profile workspace file {}: {e}", path.display()))?;
        }
    }
    for dir_name in ["data", "cache", "extensions", "tmp"] {
        fs::create_dir_all(profile_root.join(dir_name))
            .map_err(|e| format!("recreate profile workspace dir {dir_name}: {e}"))?;
    }
    Ok(())
}

pub(crate) fn purge_profile_related_state_impl(
    state: &State<'_, AppState>,
    profile_id: Uuid,
) -> Result<(), String> {
    let profile_key = profile_id.to_string();
    let profile_root = state.profile_root.join(&profile_key);
    let user_data_dir = profile_root.join("engine-profile");
    if let Some(pid) = trusted_session_pid(state.inner(), profile_id)?
        .or_else(|| {
            state
                .launched_processes
                .lock()
                .ok()
                .and_then(|items| items.get(&profile_id).copied())
        })
        .or_else(|| find_profile_process_pid_for_dir(&user_data_dir))
    {
        terminate_process_tree(pid);
    }
    terminate_profile_processes(&user_data_dir);
    let _ = revoke_launch_session(state.inner(), profile_id, None);
    stop_profile_network_stack(&state.app_handle, profile_id);
    clear_librewolf_profile_certificates(&state.app_handle, profile_id);
    if let Ok(mut launched) = state.launched_processes.lock() {
        launched.remove(&profile_id);
    }
    if let Ok(mut store) = state.identity_store.lock() {
        store.items.remove(&profile_key);
        let path = state.identity_store_path(&state.app_handle)?;
        crate::state::persist_identity_store(&path, &store)?;
    }
    if let Ok(mut store) = state.network_store.lock() {
        store.vpn_proxy.remove(&profile_key);
        store.dns.remove(&profile_key);
        store.profile_template_selection.remove(&profile_key);
        let path = state.network_store_path(&state.app_handle)?;
        crate::state::persist_network_store(&path, &store)?;
    }
    if let Ok(mut store) = state.sync_store.lock() {
        store.controls.remove(&profile_key);
        store.conflicts.remove(&profile_key);
        store.snapshots.remove(&profile_key);
        let path = state.sync_store_path(&state.app_handle)?;
        crate::state::persist_sync_store_with_secret(
            &path,
            &state.sensitive_store_secret,
            &store,
        )?;
    }
    if let Ok(mut store) = state.link_routing_store.lock() {
        if store.global_profile_id.as_deref() == Some(profile_key.as_str()) {
            store.global_profile_id = None;
        }
        store.type_bindings.retain(|_, value| value != &profile_key);
        let path = state.link_routing_store_path(&state.app_handle)?;
        crate::state::persist_link_routing_store_with_secret(
            &path,
            &state.sensitive_store_secret,
            &store,
        )?;
    }
    if let Ok(mut store) = state.network_sandbox_store.lock() {
        store.profiles.remove(&profile_key);
        let path = state.network_sandbox_store_path(&state.app_handle)?;
        crate::state::persist_network_sandbox_store(&path, &store)?;
    }
    if let Ok(mut library) = state.extension_library.lock() {
        for item in library.items.values_mut() {
            item.assigned_profile_ids
                .retain(|value| value != &profile_key);
        }
        let path = state.extension_library_path(&state.app_handle)?;
        crate::state::persist_extension_library_store(&path, &library)?;
    }
    if let Ok(mut store) = state.profile_extension_store.lock() {
        store.profiles.remove(&profile_key);
        crate::profile_extensions::persist_profile_extension_store(&state.profile_root, &store)?;
    }
    let mut security = load_global_security_record(state.inner())?;
    for cert in &mut security.certificates {
        cert.profile_ids.retain(|value| value != &profile_key);
    }
    persist_global_security_record(state.inner(), &security)?;
    Ok(())
}

pub(crate) fn global_startup_page_impl(state: &State<'_, AppState>) -> Option<String> {
    load_global_security_record(state)
        .ok()
        .and_then(|record| record.startup_page)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
