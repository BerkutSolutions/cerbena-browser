use std::{collections::BTreeMap, sync::OnceLock};

use crate::service_domains_assets::load_service_domain_map;

static SERVICE_DOMAIN_MAP: OnceLock<BTreeMap<String, Vec<String>>> = OnceLock::new();

pub fn service_domain_seeds(service: &str) -> Vec<String> {
    let key = normalize_service_key(service);
    SERVICE_DOMAIN_MAP
        .get_or_init(build_service_domain_map)
        .get(&key)
        .cloned()
        .unwrap_or_default()
}

fn build_service_domain_map() -> BTreeMap<String, Vec<String>> {
    match load_service_domain_map() {
        Ok(map) => map,
        Err(error) => {
            panic!("service domains asset load failed: {error}");
        }
    }
}

fn normalize_service_key(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::service_domain_seeds;

    #[test]
    fn contains_vk_core_domains() {
        let domains = service_domain_seeds("vk_com");
        assert!(domains.iter().any(|item| item == "vk.com"));
        assert!(domains.iter().any(|item| item == "userapi.com"));
    }

    #[test]
    fn unknown_service_returns_empty_set() {
        assert!(service_domain_seeds("unknown_service_key").is_empty());
    }
}
