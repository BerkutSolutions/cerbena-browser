use serde::{Deserialize, Serialize};

use crate::{
    dns::{DnsConfig, DnsMode, DnsResolverAdapter},
    service_catalog::ServiceCatalog,
    updater::DnsListSnapshot,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsTabPayload {
    pub profile_id: String,
    pub dns_config: DnsConfig,
    pub selected_blocklists: Vec<DnsListSnapshot>,
    pub selected_services: Vec<(String, String)>, // (category, service)
    pub domain_allowlist: Vec<String>,
    pub domain_denylist: Vec<String>,
    pub domain_exceptions: Vec<String>,
}

pub fn validate_dns_tab(
    payload: &DnsTabPayload,
    catalog: Option<&ServiceCatalog>,
) -> Result<(), String> {
    let adapter = DnsResolverAdapter {
        profile_id: payload.profile_id.clone(),
        config: payload.dns_config.clone(),
    };
    adapter.validate().map_err(|e| e.to_string())?;

    if payload.dns_config.mode == DnsMode::Custom && payload.selected_blocklists.is_empty() {
        // Not a hard requirement for runtime, but for this stage we enforce explicit source selection.
        return Err("custom DNS mode requires at least one selected blocklist".to_string());
    }

    for d in &payload.domain_allowlist {
        if d.trim().is_empty() {
            return Err("domain allowlist contains empty value".to_string());
        }
    }
    for d in &payload.domain_denylist {
        if d.trim().is_empty() {
            return Err("domain denylist contains empty value".to_string());
        }
    }

    if let Some(catalog) = catalog {
        for (category, service) in &payload.selected_services {
            let Some(category_state) = catalog.state.categories.get(&category.to_lowercase())
            else {
                return Err(format!("unknown service category: {category}"));
            };
            if !category_state
                .services
                .contains_key(&service.to_lowercase())
            {
                return Err(format!("unknown service key: {category}/{service}"));
            }
        }
    }
    Ok(())
}
