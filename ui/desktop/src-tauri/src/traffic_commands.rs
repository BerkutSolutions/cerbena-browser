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

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListTrafficEventsRequest {
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub from_epoch_ms: Option<u128>,
    pub to_epoch_ms: Option<u128>,
    pub request_query: Option<String>,
    pub response_query: Option<String>,
    pub profile_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficLogPage {
    pub entries: Vec<TrafficLogEntry>,
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub total_pages: usize,
}

#[tauri::command]
pub fn list_traffic_events(
    state: State<AppState>,
    request: Option<ListTrafficEventsRequest>,
    correlation_id: String,
) -> Result<UiEnvelope<TrafficLogPage>, String> {
    let request = request.unwrap_or_default();
    let page_size = request.page_size.unwrap_or(25).clamp(25, 100);
    let page = request.page.unwrap_or(1).max(1);
    let from_epoch_ms = request.from_epoch_ms;
    let to_epoch_ms = request.to_epoch_ms;
    let request_query = request
        .request_query
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let response_query = request
        .response_query
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let status_filter = request
        .status
        .unwrap_or_else(|| "all".to_string())
        .trim()
        .to_ascii_lowercase();
    let profile_filter = request
        .profile_id
        .unwrap_or_else(|| "all".to_string())
        .trim()
        .to_string();

    let filtered = list_traffic_log(&state)?
        .into_iter()
        .filter(|entry| {
            if let Some(from) = from_epoch_ms {
                if entry.timestamp_epoch_ms < from {
                    return false;
                }
            }
            if let Some(to) = to_epoch_ms {
                if entry.timestamp_epoch_ms > to {
                    return false;
                }
            }
            if !request_query.is_empty()
                && !entry
                    .request_host
                    .to_ascii_lowercase()
                    .contains(request_query.as_str())
            {
                return false;
            }
            if !response_query.is_empty() {
                let response_text = format!("{} {}", entry.status, entry.reason).to_ascii_lowercase();
                if !response_text.contains(response_query.as_str()) {
                    return false;
                }
            }
            if status_filter != "all" && entry.status.to_ascii_lowercase() != status_filter {
                return false;
            }
            if profile_filter != "all" && entry.profile_id != profile_filter {
                return false;
            }
            true
        })
        .collect::<Vec<_>>();

    let total_items = filtered.len();
    let total_pages = total_items.div_ceil(page_size).max(1);
    let page = page.min(total_pages);
    let start = (page - 1) * page_size;
    let entries = filtered
        .into_iter()
        .skip(start)
        .take(page_size)
        .collect::<Vec<_>>();
    Ok(ok(
        correlation_id,
        TrafficLogPage {
            entries,
            page,
            page_size,
            total_items,
            total_pages,
        },
    ))
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
