use super::*;

pub(crate) fn stage_release_if_needed_impl(
    state: &AppState,
    store: &mut AppUpdateStore,
    candidate: &ReleaseCandidate,
) -> Result<(), String> {
    let Some(asset_url) = candidate.asset_url.as_ref() else {
        store.status = "available".to_string();
        return Ok(());
    };
    let Some(asset_name) = candidate.asset_name.as_ref() else {
        store.status = "available".to_string();
        return Ok(());
    };

    if store.staged_version.as_deref() == Some(candidate.version.as_str())
        && store
            .staged_asset_path
            .as_deref()
            .map(Path::new)
            .is_some_and(Path::is_file)
    {
        if store.status == "applied_pending_relaunch" && !store.pending_apply_on_exit {
            let message = format!(
                "staged update {} was already applied but current process still reports {}; blocking repeated apply loop",
                candidate.version, CURRENT_VERSION
            );
            push_runtime_log(state, format!("[updater] {}", message));
            store.status = "error".to_string();
            store.last_error = Some(message);
            return Ok(());
        }
        push_runtime_log(
            state,
            format!(
                "[updater] reuse staged asset version={} path={}",
                candidate.version,
                store.staged_asset_path.as_deref().unwrap_or_default()
            ),
        );
        store.status = if can_auto_apply_asset(asset_name) {
            "downloaded".to_string()
        } else {
            "available".to_string()
        };
        store.selected_asset_type = candidate.asset_type.clone();
        store.selected_asset_reason = candidate.asset_selection_reason.clone();
        store.install_handoff_mode = candidate.install_handoff_mode.clone();
        store.pending_apply_on_exit = can_auto_apply_asset(asset_name);
        return Ok(());
    }

    let root = resolve_update_asset_root(state, asset_name)?;
    let target_dir = root.join(&candidate.version);
    fs::create_dir_all(&target_dir).map_err(|e| format!("create update dir: {e}"))?;
    let asset_path = target_dir.join(asset_name);

    let client = build_release_http_client(Duration::from_secs(60), true)
        .map_err(|e| format!("build update download client: {e}"))?;
    push_runtime_log(
        state,
        format!(
            "[updater] staging download start version={} asset={} target_dir={}",
            candidate.version,
            asset_name,
            target_dir.display()
        ),
    );
    let bytes = download_release_bytes_impl(&client, asset_url, "update asset")?;
    verify_release_candidate(candidate, &bytes)?;
    fs::write(&asset_path, &bytes).map_err(|e| format!("write update asset: {e}"))?;
    push_runtime_log(
        state,
        format!(
            "[updater] staging complete asset={} bytes={} path={}",
            asset_name,
            bytes.len(),
            asset_path.display()
        ),
    );

    store.staged_version = Some(candidate.version.clone());
    store.staged_asset_name = Some(asset_name.clone());
    store.staged_asset_path = Some(asset_path.to_string_lossy().to_string());
    store.selected_asset_type = candidate.asset_type.clone();
    store.selected_asset_reason = candidate.asset_selection_reason.clone();
    store.install_handoff_mode = candidate.install_handoff_mode.clone();
    store.pending_apply_on_exit = can_auto_apply_asset(asset_name);
    store.status = if store.pending_apply_on_exit {
        "downloaded".to_string()
    } else {
        "available".to_string()
    };
    Ok(())
}

pub(crate) fn stage_verified_release_asset_impl(
    state: &AppState,
    candidate: &ReleaseCandidate,
    asset_bytes: &[u8],
) -> Result<PathBuf, String> {
    let asset_name = candidate
        .asset_name
        .as_deref()
        .ok_or_else(|| "release asset name is missing".to_string())?;
    let root = resolve_update_asset_root(state, asset_name)?;
    let target_dir = root.join(&candidate.version);
    fs::create_dir_all(&target_dir).map_err(|e| format!("create update dir: {e}"))?;
    let asset_path = target_dir.join(asset_name);
    fs::write(&asset_path, asset_bytes).map_err(|e| format!("write update asset: {e}"))?;

    let mut store = state
        .app_update_store
        .lock()
        .map_err(|e| format!("lock app update store: {e}"))?;
    store.latest_version = Some(candidate.version.clone());
    store.release_url = Some(candidate.release_url.clone());
    store.has_update = true;
    store.staged_version = Some(candidate.version.clone());
    store.staged_asset_name = Some(asset_name.to_string());
    store.staged_asset_path = Some(asset_path.to_string_lossy().to_string());
    store.selected_asset_type = candidate.asset_type.clone();
    store.selected_asset_reason = candidate.asset_selection_reason.clone();
    store.install_handoff_mode = candidate.install_handoff_mode.clone();
    store.pending_apply_on_exit = can_auto_apply_asset(asset_name);
    store.status = if store.pending_apply_on_exit {
        "downloaded".to_string()
    } else {
        "available".to_string()
    };
    store.last_error = None;
    persist_update_store_from_state(state, &store)?;
    Ok(asset_path)
}

pub(crate) fn download_release_bytes_impl(
    client: &Client,
    url: &str,
    label: &str,
) -> Result<Vec<u8>, String> {
    let mut last_error = String::new();
    for attempt in 1..=3 {
        let response = match client.get(url).send() {
            Ok(value) => value,
            Err(error) => {
                last_error = format!("download {label}: {error}");
                if attempt < 3 {
                    thread::sleep(Duration::from_millis(250 * attempt as u64));
                    continue;
                }
                break;
            }
        };
        if !response.status().is_success() {
            let status = response.status();
            last_error = describe_http_failure(response, &format!("download {label}"), Some(url));
            if status.is_server_error() && attempt < 3 {
                thread::sleep(Duration::from_millis(250 * attempt as u64));
                continue;
            }
            break;
        }
        match response.bytes() {
            Ok(value) => return Ok(value.to_vec()),
            Err(error) => {
                last_error = format!("read {label} body: {error}");
                if attempt < 3 {
                    thread::sleep(Duration::from_millis(250 * attempt as u64));
                    continue;
                }
                break;
            }
        }
    }
    Err(last_error)
}
