use super::*;

pub(crate) fn load_rules_store_impl(path: &PathBuf) -> Result<TrafficRulesStore, String> {
    if !path.exists() {
        return Ok(TrafficRulesStore::default());
    }
    let bytes = fs::read(path).map_err(|e| format!("read traffic rules: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse traffic rules: {e}"))
}

pub(crate) fn persist_rules_store_impl(
    path: &PathBuf,
    rules: &TrafficRulesStore,
) -> Result<(), String> {
    let bytes =
        serde_json::to_vec_pretty(rules).map_err(|e| format!("serialize traffic rules: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("write traffic rules: {e}"))
}

pub(crate) fn load_traffic_log_impl(path: &PathBuf) -> Result<Vec<TrafficLogEntry>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(path).map_err(|e| format!("read traffic log: {e}"))?;
    let mut parsed = match serde_json::from_slice::<Vec<TrafficLogEntry>>(&bytes) {
        Ok(entries) => entries,
        Err(error) => {
            backup_corrupt_traffic_log_impl(path, &bytes);
            eprintln!(
                "[traffic-gateway] ignoring corrupt traffic log at {}: {error}",
                path.display()
            );
            return Ok(Vec::new());
        }
    };
    prune_traffic_log(&mut parsed);
    Ok(parsed)
}

fn backup_corrupt_traffic_log_impl(path: &PathBuf, bytes: &[u8]) {
    let backup_path = path.with_extension(format!("corrupt-{}.json", now_epoch_ms()));
    if fs::rename(path, &backup_path).is_err() {
        let _ = fs::write(backup_path, bytes);
        let _ = fs::remove_file(path);
    }
}

pub(crate) fn list_traffic_log_impl(state: &AppState) -> Result<Vec<TrafficLogEntry>, String> {
    let mut gateway = state
        .traffic_gateway
        .lock()
        .map_err(|_| "traffic gateway lock poisoned".to_string())?;
    let original_len = gateway.traffic_log.len();
    prune_traffic_log(&mut gateway.traffic_log);
    let changed = gateway.traffic_log.len() != original_len;
    let snapshot = gateway.traffic_log.clone();
    drop(gateway);
    if changed {
        let path = state.traffic_gateway_log_path(&state.app_handle)?;
        let _ = persist_traffic_log(&path, &snapshot);
    }
    Ok(snapshot.iter().rev().cloned().collect())
}
