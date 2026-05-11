use super::*;

pub(crate) fn should_run_auto_update_check_impl(store: &AppUpdateStore) -> bool {
    store.auto_update_enabled
        && store
            .last_checked_epoch_ms
            .map(|value| now_epoch_ms_impl().saturating_sub(value) >= UPDATE_CHECK_INTERVAL_MS)
            .unwrap_or(true)
}

pub(crate) fn persist_update_store_from_state_impl(
    state: &AppState,
    store: &AppUpdateStore,
) -> Result<(), String> {
    let path = state
        .app_update_store_path(&state.app_handle)
        .map_err(|e| format!("resolve app update store path: {e}"))?;
    persist_app_update_store(&path, store)
}

pub(crate) fn refresh_update_store_snapshot_impl(state: &AppState) -> Result<AppUpdateStore, String> {
    let path = state
        .app_update_store_path(&state.app_handle)
        .map_err(|e| format!("resolve app update store path: {e}"))?;
    let mut store = if path.exists() {
        let raw = fs::read(&path).map_err(|e| format!("read app update store: {e}"))?;
        serde_json::from_slice::<AppUpdateStore>(&raw)
            .map_err(|e| format!("parse app update store: {e}"))?
    } else {
        AppUpdateStore::default()
    };
    reconcile_update_store_with_current_version_impl(&mut store);
    push_runtime_log(
        state,
        format!(
            "[updater] update store snapshot refreshed status={} latest={} staged={} handoff={} pending_apply={}",
            store.status,
            store.latest_version.as_deref().unwrap_or("none"),
            store.staged_version.as_deref().unwrap_or("none"),
            store.updater_handoff_version.as_deref().unwrap_or("none"),
            store.pending_apply_on_exit
        ),
    );
    {
        let mut guard = state
            .app_update_store
            .lock()
            .map_err(|e| format!("lock app update store: {e}"))?;
        *guard = store.clone();
    }
    Ok(store)
}

pub(crate) fn reconcile_update_store_with_current_version_impl(store: &mut AppUpdateStore) {
    let staged_is_current_or_older = store
        .staged_version
        .as_deref()
        .map(|version| !is_version_newer_impl(version, CURRENT_VERSION))
        .unwrap_or(false);
    let handoff_is_current_or_older = store
        .updater_handoff_version
        .as_deref()
        .map(|version| !is_version_newer_impl(version, CURRENT_VERSION))
        .unwrap_or(false);
    let stale_handoff_status = matches!(
        store.status.as_str(),
        "applying" | "downloaded" | "applied_pending_relaunch"
    );
    if staged_is_current_or_older || (handoff_is_current_or_older && stale_handoff_status) {
        clear_staged_update(store);
        store.updater_handoff_version = None;
        if stale_handoff_status {
            store.status = "up_to_date".to_string();
        }
        store.last_error = None;
        store.latest_version = Some(CURRENT_VERSION.to_string());
    }
}

pub(crate) fn write_update_store_snapshot_impl(
    state: &AppState,
    store: &AppUpdateStore,
) -> Result<(), String> {
    {
        let mut guard = state
            .app_update_store
            .lock()
            .map_err(|e| format!("lock app update store: {e}"))?;
        *guard = store.clone();
    }
    persist_update_store_from_state_impl(state, store)
}

pub(crate) fn to_view_impl(store: &AppUpdateStore) -> AppUpdateView {
    AppUpdateView {
        current_version: CURRENT_VERSION.to_string(),
        repository_url: REPOSITORY_URL.to_string(),
        auto_update_enabled: store.auto_update_enabled,
        last_checked_at: store.last_checked_at.clone(),
        latest_version: store.latest_version.clone(),
        release_url: store.release_url.clone(),
        has_update: store.has_update,
        status: if store.status.trim().is_empty() {
            "idle".to_string()
        } else {
            store.status.clone()
        },
        last_error: store.last_error.clone(),
        staged_version: store.staged_version.clone(),
        staged_asset_name: store.staged_asset_name.clone(),
        selected_asset_type: store.selected_asset_type.clone(),
        selected_asset_reason: store.selected_asset_reason.clone(),
        install_handoff_mode: store.install_handoff_mode.clone(),
        can_auto_apply: store
            .staged_asset_name
            .as_deref()
            .map(can_auto_apply_asset)
            .unwrap_or(false),
    }
}

pub(crate) fn now_epoch_ms_impl() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub(crate) fn now_iso_impl() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    seconds.to_string()
}

pub(crate) fn normalize_version_impl(raw: &str) -> String {
    raw.trim().trim_start_matches('v').to_string()
}

pub(crate) fn is_version_newer_impl(candidate: &str, current: &str) -> bool {
    let c = parse_version_parts_impl(&normalize_version_impl(candidate));
    let p = parse_version_parts_impl(&normalize_version_impl(current));
    c > p
}

pub(crate) fn parse_version_parts_impl(value: &str) -> Vec<u64> {
    let normalized = normalize_version_impl(value);
    let mut parts = normalized.splitn(2, '-');
    let base = parts.next().unwrap_or_default();
    let hotfix_suffix = parts.next().filter(|suffix| {
        !suffix.is_empty()
            && suffix
                .split('.')
                .all(|segment| !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_digit()))
    });

    let mut parsed = base
        .split('.')
        .map(|item| item.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    if let Some(suffix) = hotfix_suffix {
        parsed.extend(
            suffix
                .split('.')
                .map(|part| part.parse::<u64>().unwrap_or(0)),
        );
    }
    parsed
}

pub(crate) fn sha256_hex_impl(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(crate) fn powershell_quote_impl(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

pub(crate) fn resolve_latest_release_api_url_impl() -> String {
    std::env::var(RELEASE_LATEST_API_URL_ENV).unwrap_or_else(|_| GITHUB_LATEST_RELEASE_API.to_string())
}
