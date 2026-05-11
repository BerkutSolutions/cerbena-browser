use super::*;

pub(crate) async fn ensure_engine_binaries_impl(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<String>>, String> {
    let runtime =
        EngineRuntime::new(state.engine_runtime_root.clone()).map_err(|e| e.to_string())?;
    let mut ready = Vec::new();
    for engine in [
        EngineKind::Chromium,
        EngineKind::UngoogledChromium,
        EngineKind::Librewolf,
    ] {
        let installation = ensure_engine_ready(&app_handle, &state, &runtime, engine).await?;
        ready.push(format!(
            "{} {}",
            installation.engine.as_key(),
            installation.version
        ));
    }
    Ok(ok(correlation_id, ready))
}

pub(crate) fn cancel_engine_download_impl(
    state: State<AppState>,
    engine: String,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    let normalized = engine.trim().to_lowercase();
    if normalized.is_empty() {
        return Err("engine is required".to_string());
    }
    let mut cancelled = state
        .cancelled_engine_downloads
        .lock()
        .map_err(|_| "cancelled engine download lock poisoned".to_string())?;
    cancelled.insert(normalized);
    Ok(ok(correlation_id, true))
}
