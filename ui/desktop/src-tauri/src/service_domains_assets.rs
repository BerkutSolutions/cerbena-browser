use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

const EMBEDDED_SERVICE_DOMAINS_V1: &str =
    include_str!("../assets/service-catalog/v1/service-domains.json");

#[derive(Debug, Deserialize)]
struct ServiceDomainsAssetV1 {
    version: String,
    services: Vec<ServiceDomainsEntry>,
}

#[derive(Debug, Deserialize)]
struct ServiceDomainsEntry {
    service: String,
    domains: Vec<String>,
}

pub fn load_service_domain_map() -> Result<BTreeMap<String, Vec<String>>, String> {
    let asset = parse_domains_asset(EMBEDDED_SERVICE_DOMAINS_V1)?;
    validate_domains_asset(&asset)?;
    Ok(build_domain_map(&asset))
}

fn parse_domains_asset(input: &str) -> Result<ServiceDomainsAssetV1, String> {
    serde_json::from_str::<ServiceDomainsAssetV1>(input)
        .map_err(|error| format!("service domains asset parse failed: {error}"))
}

fn validate_domains_asset(asset: &ServiceDomainsAssetV1) -> Result<(), String> {
    if asset.version.trim() != "1" {
        return Err(format!(
            "unsupported service domains asset version: {}",
            asset.version
        ));
    }
    if asset.services.is_empty() {
        return Err("service domains asset has no services".to_string());
    }
    let mut seen_services = BTreeSet::new();
    for entry in &asset.services {
        let service = normalize_service_key(&entry.service);
        if !is_service_key(&service) {
            return Err(format!("invalid service key in domains asset: {}", entry.service));
        }
        if !seen_services.insert(service.clone()) {
            return Err(format!("duplicate service key in domains asset: {service}"));
        }
        let mut seen_domains = BTreeSet::new();
        for raw_domain in &entry.domains {
            let domain = normalize_domain(raw_domain);
            if !is_valid_domain(&domain) {
                return Err(format!(
                    "invalid domain '{raw_domain}' for service '{service}'"
                ));
            }
            if !seen_domains.insert(domain.clone()) {
                return Err(format!(
                    "duplicate domain '{domain}' for service '{service}'"
                ));
            }
        }
    }
    Ok(())
}

fn build_domain_map(asset: &ServiceDomainsAssetV1) -> BTreeMap<String, Vec<String>> {
    let mut map = BTreeMap::<String, BTreeSet<String>>::new();
    for entry in &asset.services {
        let service = normalize_service_key(&entry.service);
        let set = map.entry(service).or_default();
        for raw_domain in &entry.domains {
            let domain = normalize_domain(raw_domain);
            if is_valid_domain(&domain) {
                set.insert(domain);
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
    use super::load_service_domain_map;

    #[test]
    fn loads_vk_domains_from_asset() {
        let map = load_service_domain_map().expect("asset loads");
        let vk = map.get("vk_com").expect("vk_com exists");
        assert!(vk.iter().any(|item| item == "vk.com"));
        assert!(vk.iter().any(|item| item == "userapi.com"));
    }
}
