use std::collections::{BTreeMap, BTreeSet};

use browser_network_policy::ServiceCatalog;
use serde::Deserialize;

const EMBEDDED_SERVICE_CATALOG_V1: &str =
    include_str!("../assets/service-catalog/v1/catalog.json");

#[derive(Debug, Deserialize)]
pub struct ServiceCatalogAssetV1 {
    pub version: String,
    pub categories: Vec<ServiceCategoryAssetV1>,
}

#[derive(Debug, Deserialize)]
pub struct ServiceCategoryAssetV1 {
    pub id: String,
    pub labels: LocalizedLabelAsset,
    pub services: Vec<ServiceEntryAssetV1>,
}

#[derive(Debug, Deserialize)]
pub struct ServiceEntryAssetV1 {
    pub id: String,
    pub labels: LocalizedLabelAsset,
    pub domains: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct LocalizedLabelAsset {
    pub en: String,
    pub ru: String,
}

pub fn parse_service_catalog_asset(input: &str) -> Result<ServiceCatalogAssetV1, String> {
    serde_json::from_str::<ServiceCatalogAssetV1>(input)
        .map_err(|error| format!("service catalog asset parse failed: {error}"))
}

pub fn validate_service_catalog_asset(asset: &ServiceCatalogAssetV1) -> Result<(), String> {
    if asset.version.trim() != "1" {
        return Err(format!(
            "unsupported service catalog asset version: {}",
            asset.version
        ));
    }
    if asset.categories.is_empty() {
        return Err("service catalog asset has no categories".to_string());
    }

    let mut category_ids = BTreeSet::new();
    let mut global_service_ids = BTreeSet::new();
    for category in &asset.categories {
        let category_id = normalize_id(&category.id);
        if !is_valid_id(&category_id) {
            return Err(format!("invalid category id: {}", category.id));
        }
        if !category_ids.insert(category_id.clone()) {
            return Err(format!("duplicate category id: {category_id}"));
        }
        validate_labels("category", &category_id, &category.labels)?;
        if category.services.is_empty() {
            return Err(format!("category has no services: {category_id}"));
        }

        let mut category_service_ids = BTreeSet::new();
        for service in &category.services {
            let service_id = normalize_id(&service.id);
            if !is_valid_id(&service_id) {
                return Err(format!(
                    "invalid service id '{service_id}' in category '{category_id}'"
                ));
            }
            if !category_service_ids.insert(service_id.clone()) {
                return Err(format!(
                    "duplicate service id '{service_id}' in category '{category_id}'"
                ));
            }
            if !global_service_ids.insert(service_id.clone()) {
                return Err(format!("service id used in multiple categories: {service_id}"));
            }
            validate_labels("service", &service_id, &service.labels)?;
            validate_domains(category_id.as_str(), service_id.as_str(), &service.domains)?;
        }
    }
    Ok(())
}

pub fn build_service_catalog_from_assets() -> Result<ServiceCatalog, String> {
    let asset = parse_service_catalog_asset(EMBEDDED_SERVICE_CATALOG_V1)?;
    validate_service_catalog_asset(&asset)?;
    Ok(build_service_catalog_from_asset(&asset))
}

fn build_service_catalog_from_asset(asset: &ServiceCatalogAssetV1) -> ServiceCatalog {
    let mut seed = BTreeMap::<String, Vec<String>>::new();
    for category in &asset.categories {
        let category_id = normalize_id(&category.id);
        let mut services = Vec::with_capacity(category.services.len());
        for service in &category.services {
            services.push(normalize_id(&service.id));
        }
        seed.insert(category_id, services);
    }
    ServiceCatalog::from_seed(seed)
}

fn validate_labels(kind: &str, id: &str, labels: &LocalizedLabelAsset) -> Result<(), String> {
    if labels.en.trim().is_empty() {
        return Err(format!("{kind} '{id}' has empty EN label"));
    }
    if labels.ru.trim().is_empty() {
        return Err(format!("{kind} '{id}' has empty RU label"));
    }
    Ok(())
}

fn validate_domains(category_id: &str, service_id: &str, domains: &[String]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for raw in domains {
        let domain = normalize_domain(raw);
        if !is_valid_domain(domain.as_str()) {
            return Err(format!(
                "invalid domain '{raw}' for service '{service_id}' in category '{category_id}'"
            ));
        }
        if !seen.insert(domain.clone()) {
            return Err(format!(
                "duplicate domain '{domain}' for service '{service_id}' in category '{category_id}'"
            ));
        }
    }
    Ok(())
}

fn normalize_id(value: &str) -> String {
    value.trim().to_lowercase()
}

fn is_valid_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '+')
}

fn normalize_domain(value: &str) -> String {
    value
        .trim()
        .trim_start_matches("*.")
        .trim_start_matches('.')
        .trim_end_matches('.')
        .to_lowercase()
}

fn is_valid_domain(value: &str) -> bool {
    if value.is_empty() || value.contains('/') || value.contains('*') || !value.contains('.') {
        return false;
    }
    value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '.')
}

#[cfg(test)]
mod tests {
    use super::{
        build_service_catalog_from_assets, parse_service_catalog_asset, validate_service_catalog_asset,
        ServiceCatalogAssetV1,
    };

    #[test]
    fn embedded_asset_loads_and_validates() {
        let catalog = build_service_catalog_from_assets().expect("catalog loads");
        assert!(catalog.state.categories.contains_key("artificial_intelligence"));
        assert!(
            catalog
                .state
                .categories
                .get("social_networks_and_communities")
                .is_some_and(|category| category.services.contains_key("vk_com"))
        );
    }

    #[test]
    fn rejects_duplicate_service_ids() {
        let input = r#"{
          "version":"1",
          "categories":[{"id":"social","labels":{"en":"Social","ru":"Соц"},"services":[
            {"id":"vk","labels":{"en":"VK","ru":"ВК"},"domains":[]},
            {"id":"vk","labels":{"en":"VK2","ru":"ВК2"},"domains":[]}
          ]}]
        }"#;
        let asset: ServiceCatalogAssetV1 = parse_service_catalog_asset(input).expect("parse");
        let error = validate_service_catalog_asset(&asset).expect_err("must fail");
        assert!(error.contains("duplicate service id"));
    }

    #[test]
    fn rejects_missing_ru_label() {
        let input = r#"{
          "version":"1",
          "categories":[{"id":"social","labels":{"en":"Social","ru":"Соц"},"services":[
            {"id":"vk","labels":{"en":"VK","ru":" "},"domains":[]}
          ]}]
        }"#;
        let asset: ServiceCatalogAssetV1 = parse_service_catalog_asset(input).expect("parse");
        let error = validate_service_catalog_asset(&asset).expect_err("must fail");
        assert!(error.contains("empty RU label"));
    }

    #[test]
    fn rejects_invalid_domain_shape() {
        let input = r#"{
          "version":"1",
          "categories":[{"id":"social","labels":{"en":"Social","ru":"Соц"},"services":[
            {"id":"vk","labels":{"en":"VK","ru":"ВК"},"domains":["https://vk.com/path"]}
          ]}]
        }"#;
        let asset: ServiceCatalogAssetV1 = parse_service_catalog_asset(input).expect("parse");
        let error = validate_service_catalog_asset(&asset).expect_err("must fail");
        assert!(error.contains("invalid domain"));
    }
}
