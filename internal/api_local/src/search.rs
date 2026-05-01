use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchProvider {
    pub id: String,
    pub display_name: String,
    pub query_template: String,
}

#[derive(Debug, Default, Clone)]
pub struct SearchProviderRegistry {
    providers: BTreeMap<String, SearchProvider>,
}

impl SearchProviderRegistry {
    pub fn import_presets(&mut self, presets: Vec<SearchProvider>) -> Result<(), String> {
        for item in presets {
            self.validate_provider(&item)?;
            self.providers.insert(item.id.clone(), item);
        }
        Ok(())
    }

    pub fn set_default(&self, provider_id: &str) -> Result<&SearchProvider, String> {
        self.providers
            .get(provider_id)
            .ok_or_else(|| "search.provider.not_found".to_string())
    }

    pub fn validate_provider(&self, provider: &SearchProvider) -> Result<(), String> {
        if provider.id.trim().is_empty() {
            return Err("search.provider.id.empty".to_string());
        }
        if !provider.query_template.contains("{query}") {
            return Err("search.provider.template.missing_query".to_string());
        }
        if !(provider.query_template.starts_with("https://")
            || provider.query_template.starts_with("http://"))
        {
            return Err("search.provider.template.scheme_invalid".to_string());
        }
        Ok(())
    }
}
