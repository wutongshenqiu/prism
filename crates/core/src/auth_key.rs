use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AuthKeyEntry {
    pub key: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub allowed_models: Vec<String>,
    /// Restrict which provider credentials this key can use (glob patterns by credential name).
    /// Empty = allow all credentials.
    #[serde(default)]
    pub allowed_credentials: Vec<String>,
    #[serde(default)]
    pub rate_limit: Option<KeyRateLimitConfig>,
    #[serde(default)]
    pub budget: Option<BudgetConfig>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct KeyRateLimitConfig {
    pub rpm: Option<u32>,
    pub tpm: Option<u64>,
    pub cost_per_day_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BudgetConfig {
    pub total_usd: f64,
    pub period: BudgetPeriod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetPeriod {
    Daily,
    Monthly,
}

/// Runtime fast-lookup index for auth keys.
#[derive(Debug, Clone)]
pub struct AuthKeyStore {
    entries: Vec<AuthKeyEntry>,
    by_key: HashMap<String, usize>,
}

impl AuthKeyStore {
    pub fn new(entries: Vec<AuthKeyEntry>) -> Self {
        let by_key = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.key.clone(), i))
            .collect();
        Self { entries, by_key }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// O(1) lookup by key string.
    pub fn lookup(&self, key: &str) -> Option<&AuthKeyEntry> {
        self.by_key.get(key).map(|&i| &self.entries[i])
    }

    /// Check if an auth key entry has expired.
    pub fn is_expired(entry: &AuthKeyEntry) -> bool {
        if let Some(expires_at) = entry.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Check if the entry grants access to the given model (glob patterns).
    pub fn check_model_access(entry: &AuthKeyEntry, model: &str) -> bool {
        if entry.allowed_models.is_empty() {
            return true;
        }
        entry
            .allowed_models
            .iter()
            .any(|pattern| crate::glob::glob_match(pattern, model))
    }

    /// Replace entries from a new config reload.
    pub fn update_from_config(&mut self, entries: Vec<AuthKeyEntry>) {
        self.by_key = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.key.clone(), i))
            .collect();
        self.entries = entries;
    }

    pub fn entries(&self) -> &[AuthKeyEntry] {
        &self.entries
    }

    /// Mask a key for display: show first 4 + last 4 chars.
    pub fn mask_key(key: &str) -> String {
        if key.len() <= 8 {
            return "****".to_string();
        }
        format!("{}****{}", &key[..4], &key[key.len() - 4..])
    }
}

impl Default for AuthKeyStore {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_key_store_lookup() {
        let entries = vec![
            AuthKeyEntry {
                key: "sk-proxy-abc123".to_string(),
                name: Some("Team Alpha".to_string()),
                tenant_id: Some("alpha".to_string()),
                allowed_models: vec!["claude-*".to_string(), "gpt-4o*".to_string()],
                allowed_credentials: vec![],
                rate_limit: None,
                budget: None,
                expires_at: None,
                metadata: HashMap::new(),
            },
            AuthKeyEntry {
                key: "sk-proxy-def456".to_string(),
                name: Some("Team Beta".to_string()),
                tenant_id: Some("beta".to_string()),
                allowed_models: vec![],
                allowed_credentials: vec![],
                rate_limit: None,
                budget: None,
                expires_at: None,
                metadata: HashMap::new(),
            },
        ];
        let store = AuthKeyStore::new(entries);

        assert!(store.lookup("sk-proxy-abc123").is_some());
        assert!(store.lookup("sk-proxy-def456").is_some());
        assert!(store.lookup("sk-proxy-nonexistent").is_none());
    }

    #[test]
    fn test_model_access_check() {
        let entry = AuthKeyEntry {
            key: "sk-proxy-test".to_string(),
            name: None,
            tenant_id: None,
            allowed_models: vec!["claude-*".to_string(), "gpt-4o".to_string()],
            allowed_credentials: vec![],
            rate_limit: None,
            budget: None,
            expires_at: None,
            metadata: HashMap::new(),
        };
        assert!(AuthKeyStore::check_model_access(&entry, "claude-3-opus"));
        assert!(AuthKeyStore::check_model_access(&entry, "gpt-4o"));
        assert!(!AuthKeyStore::check_model_access(&entry, "gpt-3.5-turbo"));
    }

    #[test]
    fn test_empty_allowed_models_allows_all() {
        let entry = AuthKeyEntry {
            key: "sk-proxy-test".to_string(),
            name: None,
            tenant_id: None,
            allowed_models: vec![],
            allowed_credentials: vec![],
            rate_limit: None,
            budget: None,
            expires_at: None,
            metadata: HashMap::new(),
        };
        assert!(AuthKeyStore::check_model_access(&entry, "anything"));
    }

    #[test]
    fn test_mask_key() {
        assert_eq!(
            AuthKeyStore::mask_key("sk-proxy-abc123def456"),
            "sk-p****f456"
        );
        assert_eq!(AuthKeyStore::mask_key("short"), "****");
    }

    #[test]
    fn test_is_expired() {
        let not_expired = AuthKeyEntry {
            key: "k".to_string(),
            name: None,
            tenant_id: None,
            allowed_models: vec![],
            allowed_credentials: vec![],
            rate_limit: None,
            budget: None,
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
            metadata: HashMap::new(),
        };
        assert!(!AuthKeyStore::is_expired(&not_expired));

        let expired = AuthKeyEntry {
            key: "k".to_string(),
            name: None,
            tenant_id: None,
            allowed_models: vec![],
            allowed_credentials: vec![],
            rate_limit: None,
            budget: None,
            expires_at: Some(Utc::now() - chrono::Duration::hours(1)),
            metadata: HashMap::new(),
        };
        assert!(AuthKeyStore::is_expired(&expired));

        let no_expiry = AuthKeyEntry {
            key: "k".to_string(),
            name: None,
            tenant_id: None,
            allowed_models: vec![],
            allowed_credentials: vec![],
            rate_limit: None,
            budget: None,
            expires_at: None,
            metadata: HashMap::new(),
        };
        assert!(!AuthKeyStore::is_expired(&no_expiry));
    }
}
