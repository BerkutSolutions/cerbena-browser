use browser_network_policy::ServiceCatalog;
use crate::service_catalog_assets::build_service_catalog_from_assets;

pub fn build_service_catalog() -> ServiceCatalog {
    match build_service_catalog_from_assets() {
        Ok(catalog) => catalog,
        Err(error) => panic!("service catalog asset load failed: {error}"),
    }
}
