use prism_core::provider::{AuthRecord, Format};
use prism_core::routing::planner::{CredentialEntry, InventorySnapshot, ProviderEntry};
use prism_domain::capability::default_capabilities_for_protocol;
use std::collections::HashMap;
use std::sync::RwLock;

/// Manages the inventory of available providers and credentials.
/// Provides snapshots for the route planner.
pub struct ProviderCatalog {
    providers: RwLock<Vec<CatalogProvider>>,
}

struct CatalogProvider {
    format: Format,
    name: String,
    credentials: Vec<CatalogCredential>,
}

struct CatalogCredential {
    record: AuthRecord,
}

impl ProviderCatalog {
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(Vec::new()),
        }
    }

    /// Create an inventory snapshot for the planner.
    pub fn snapshot(&self) -> InventorySnapshot {
        let providers = self.providers.read().unwrap_or_else(|e| e.into_inner());
        InventorySnapshot {
            providers: providers
                .iter()
                .map(|p| {
                    let up = prism_core::provider::upstream_protocol(p.format);
                    ProviderEntry {
                        format: p.format,
                        name: p.name.clone(),
                        credentials: p
                            .credentials
                            .iter()
                            .map(|c| CredentialEntry {
                                id: c.record.id.clone(),
                                name: c
                                    .record
                                    .credential_name
                                    .clone()
                                    .unwrap_or_else(|| c.record.id.clone()),
                                models: c.record.models.iter().map(|m| m.id.clone()).collect(),
                                excluded_models: c.record.excluded_models.clone(),
                                region: c.record.region.clone(),
                                weight: c.record.weight,
                                disabled: c.record.disabled,
                            })
                            .collect(),
                        capabilities: default_capabilities_for_protocol(up),
                        upstream_protocol: up,
                    }
                })
                .collect(),
        }
    }

    /// Update catalog from a pre-built credential map (keyed by provider name).
    pub fn update_from_credentials(&self, credentials: &HashMap<String, Vec<AuthRecord>>) {
        let mut providers = Vec::new();
        for (provider_name, records) in credentials {
            let format = records
                .first()
                .map(|r| r.provider)
                .unwrap_or(Format::OpenAI);
            providers.push(CatalogProvider {
                format,
                name: provider_name.clone(),
                credentials: records
                    .iter()
                    .map(|r| CatalogCredential { record: r.clone() })
                    .collect(),
            });
        }
        let mut catalog = self.providers.write().unwrap_or_else(|e| e.into_inner());
        *catalog = providers;
    }

    /// Find a credential by ID across all providers.
    pub fn find_credential(&self, id: &str) -> Option<(String, AuthRecord)> {
        let providers = self.providers.read().unwrap_or_else(|e| e.into_inner());
        for provider in providers.iter() {
            for cred in &provider.credentials {
                if cred.record.id == id {
                    return Some((provider.name.clone(), cred.record.clone()));
                }
            }
        }
        None
    }

    /// Get all available model names across all providers.
    pub fn all_models(&self) -> Vec<String> {
        let providers = self.providers.read().unwrap_or_else(|e| e.into_inner());
        let mut models = std::collections::HashSet::new();
        for provider in providers.iter() {
            for cred in &provider.credentials {
                if cred.record.disabled {
                    continue;
                }
                for model in &cred.record.models {
                    models.insert(model.id.clone());
                }
            }
        }
        let mut result: Vec<_> = models.into_iter().collect();
        result.sort();
        result
    }
}

impl Default for ProviderCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::circuit_breaker::NoopCircuitBreaker;
    use prism_core::provider::ModelEntry;
    use std::sync::Arc;

    fn test_record(id: &str, format: Format, models: &[&str]) -> AuthRecord {
        AuthRecord {
            id: id.to_string(),
            provider: format,
            provider_name: format!("provider-{id}"),
            api_key: format!("key-{id}"),
            base_url: None,
            proxy_url: None,
            headers: Default::default(),
            models: models
                .iter()
                .map(|m| ModelEntry {
                    id: m.to_string(),
                    alias: None,
                })
                .collect(),
            excluded_models: vec![],
            prefix: None,
            disabled: false,
            circuit_breaker: Arc::new(NoopCircuitBreaker),
            cloak: None,
            wire_api: Default::default(),
            credential_name: Some(format!("name-{id}")),
            weight: 100,
            region: None,
            upstream_presentation: Default::default(),
            vertex: false,
            vertex_project: None,
            vertex_location: None,
        }
    }

    #[test]
    fn test_catalog_empty() {
        let catalog = ProviderCatalog::new();
        let snapshot = catalog.snapshot();
        assert!(snapshot.providers.is_empty());
    }

    #[test]
    fn test_catalog_snapshot() {
        let catalog = ProviderCatalog::new();
        {
            let mut providers = catalog.providers.write().unwrap();
            providers.push(CatalogProvider {
                format: Format::OpenAI,
                name: "openai".to_string(),
                credentials: vec![CatalogCredential {
                    record: test_record("c1", Format::OpenAI, &["gpt-4"]),
                }],
            });
        }
        let snapshot = catalog.snapshot();
        assert_eq!(snapshot.providers.len(), 1);
        assert_eq!(snapshot.providers[0].name, "openai");
        assert_eq!(snapshot.providers[0].credentials.len(), 1);
        assert_eq!(snapshot.providers[0].credentials[0].id, "c1");
    }

    #[test]
    fn test_catalog_find_credential() {
        let catalog = ProviderCatalog::new();
        {
            let mut providers = catalog.providers.write().unwrap();
            providers.push(CatalogProvider {
                format: Format::OpenAI,
                name: "openai".to_string(),
                credentials: vec![CatalogCredential {
                    record: test_record("c1", Format::OpenAI, &["gpt-4"]),
                }],
            });
        }
        let found = catalog.find_credential("c1");
        assert!(found.is_some());
        let (provider_name, record) = found.unwrap();
        assert_eq!(provider_name, "openai");
        assert_eq!(record.id, "c1");

        assert!(catalog.find_credential("nonexistent").is_none());
    }

    #[test]
    fn test_catalog_all_models() {
        let catalog = ProviderCatalog::new();
        {
            let mut providers = catalog.providers.write().unwrap();
            providers.push(CatalogProvider {
                format: Format::OpenAI,
                name: "openai".to_string(),
                credentials: vec![
                    CatalogCredential {
                        record: test_record("c1", Format::OpenAI, &["gpt-4", "gpt-3.5-turbo"]),
                    },
                    CatalogCredential {
                        record: test_record("c2", Format::OpenAI, &["gpt-4"]),
                    },
                ],
            });
        }
        let models = catalog.all_models();
        assert!(models.contains(&"gpt-4".to_string()));
        assert!(models.contains(&"gpt-3.5-turbo".to_string()));
    }
}
