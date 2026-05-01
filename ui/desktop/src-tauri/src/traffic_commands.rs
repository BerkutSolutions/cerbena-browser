use serde::Deserialize;
use tauri::State;

use crate::{
    envelope::{ok, UiEnvelope},
    state::AppState,
    traffic_gateway::{list_traffic_log, set_domain_block_rule, TrafficLogEntry},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleTrafficRuleRequest {
    pub profile_id: Option<String>,
    pub domain: String,
    pub blocked: bool,
}

#[tauri::command]
pub fn list_traffic_events(
    state: State<AppState>,
    correlation_id: String,
) -> Result<UiEnvelope<Vec<TrafficLogEntry>>, String> {
    Ok(ok(correlation_id, list_traffic_log(&state)?))
}

#[tauri::command]
pub fn set_traffic_rule(
    state: State<AppState>,
    request: ToggleTrafficRuleRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    set_domain_block_rule(&state, request.profile_id, &request.domain, request.blocked)?;
    Ok(ok(correlation_id, true))
}
