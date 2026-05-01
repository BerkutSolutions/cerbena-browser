use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCategoryState {
    pub category: String,
    pub block_all: bool,
    pub services: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePolicyState {
    pub categories: BTreeMap<String, ServiceCategoryState>,
    pub exceptions: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub struct ServiceCatalog {
    pub state: ServicePolicyState,
}

#[derive(Debug, Error)]
pub enum ServiceCatalogError {
    #[error("unknown category: {0}")]
    UnknownCategory(String),
}

impl ServiceCatalog {
    pub fn from_seed(seed: BTreeMap<String, Vec<String>>) -> Self {
        let mut categories = BTreeMap::new();
        for (category, services) in seed {
            let mut map = BTreeMap::new();
            for s in services {
                map.insert(s.to_lowercase(), true);
            }
            categories.insert(
                category.to_lowercase(),
                ServiceCategoryState {
                    category: category.to_lowercase(),
                    block_all: false,
                    services: map,
                },
            );
        }
        Self {
            state: ServicePolicyState {
                categories,
                exceptions: BTreeSet::new(),
            },
        }
    }

    pub fn set_category_block_all(
        &mut self,
        category: &str,
        block_all: bool,
    ) -> Result<(), ServiceCatalogError> {
        let c = self
            .state
            .categories
            .get_mut(&category.to_lowercase())
            .ok_or_else(|| ServiceCatalogError::UnknownCategory(category.to_string()))?;
        c.block_all = block_all;
        Ok(())
    }

    pub fn set_service_allowed(
        &mut self,
        category: &str,
        service: &str,
        allowed: bool,
    ) -> Result<(), ServiceCatalogError> {
        let c = self
            .state
            .categories
            .get_mut(&category.to_lowercase())
            .ok_or_else(|| ServiceCatalogError::UnknownCategory(category.to_string()))?;
        c.services.insert(service.to_lowercase(), allowed);
        Ok(())
    }

    pub fn add_exception(&mut self, domain_or_service: &str) {
        self.state
            .exceptions
            .insert(domain_or_service.to_lowercase());
    }

    pub fn is_allowed(&self, category: &str, service: &str) -> bool {
        if self.state.exceptions.contains(&service.to_lowercase()) {
            return true;
        }
        let Some(cat) = self.state.categories.get(&category.to_lowercase()) else {
            return true;
        };
        if cat.block_all {
            return false;
        }
        cat.services
            .get(&service.to_lowercase())
            .copied()
            .unwrap_or(true)
    }
}
