use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use crate::service_domains_data::SERVICE_DOMAIN_DATA;

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
    let mut map = BTreeMap::<String, BTreeSet<String>>::new();
    for (service, domains) in SERVICE_DOMAIN_DATA {
        let normalized_service = normalize_service_key(service);
        if !is_service_key(&normalized_service) {
            continue;
        }
        let set = map.entry(normalized_service).or_default();
        for domain in *domains {
            let normalized_domain = normalize_domain(domain);
            if is_valid_domain(&normalized_domain) {
                set.insert(normalized_domain);
            }
        }
    }

    map.into_iter()
        .map(|(service, domains)| (service, domains.into_iter().collect::<Vec<_>>()))
        .collect()
}

fn normalize_service_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn normalize_domain(value: &str) -> String {
    value
        .trim()
        .trim_start_matches("*.")
        .trim_start_matches('.')
        .trim_end_matches('.')
        .to_lowercase()
}

fn is_service_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '+')
}

fn is_valid_domain(value: &str) -> bool {
    if value.is_empty() || value == "-" || value.contains('/') || value.contains('*') {
        return false;
    }
    if !value.contains('.') {
        return false;
    }
    value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '.')
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
