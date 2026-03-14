use dashmap::DashMap;
use prism_core::circuit_breaker::{
    CircuitBreakerConfig, CircuitBreakerPolicy, CircuitState, NoopCircuitBreaker,
    ThreeStateCircuitBreaker,
};
use prism_core::config::Config;
use prism_core::provider::{AuthRecord, Format, ModelEntry, ModelInfo};
use prism_core::routing::config::CredentialStrategy;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Tracks when a credential's quota cooldown expires.
pub struct QuotaCooldown {
    pub until: Instant,
}

/// Check if a credential is allowed by the given patterns.
/// Empty patterns = allow all. Non-empty patterns require the credential to have
/// a name matching at least one pattern (unnamed credentials are excluded).
pub fn check_credential_access(patterns: &[String], credential_name: Option<&str>) -> bool {
    if patterns.is_empty() {
        return true;
    }
    let Some(name) = credential_name else {
        return false;
    };
    patterns
        .iter()
        .any(|pattern| prism_core::glob::glob_match(pattern, name))
}

pub struct CredentialRouter {
    credentials: RwLock<HashMap<Format, Vec<AuthRecord>>>,
    /// Index: credential_id → (Format, index in Vec) for O(1) lookup.
    credential_index: RwLock<HashMap<String, (Format, usize)>>,
    counters: RwLock<HashMap<String, AtomicUsize>>,
    strategy: RwLock<CredentialStrategy>,
    /// EWMA latency per credential_id (ms).
    latency_ewma: RwLock<HashMap<String, f64>>,
    /// EWMA smoothing factor (0.0-1.0).
    ewma_alpha: RwLock<f64>,
    /// Circuit breaker config (used when building new records).
    cb_config: RwLock<CircuitBreakerConfig>,
    /// Quota cooldowns: credential_id → cooldown expiry.
    cooldowns: DashMap<String, QuotaCooldown>,
}

impl CredentialRouter {
    pub fn new(strategy: CredentialStrategy) -> Self {
        Self {
            credentials: RwLock::new(HashMap::new()),
            credential_index: RwLock::new(HashMap::new()),
            counters: RwLock::new(HashMap::new()),
            strategy: RwLock::new(strategy),
            latency_ewma: RwLock::new(HashMap::new()),
            ewma_alpha: RwLock::new(0.3),
            cb_config: RwLock::new(CircuitBreakerConfig::default()),
            cooldowns: DashMap::new(),
        }
    }

    /// Pick the next available credential for the given provider and model.
    /// Skips credentials whose IDs are in `tried`.
    /// If `allowed_credentials` is non-empty, only credentials matching those
    /// glob patterns (by credential name) are considered.
    pub fn pick(
        &self,
        provider: Format,
        model: &str,
        tried: &[String],
        _client_region: Option<&str>,
        allowed_credentials: &[String],
    ) -> Option<AuthRecord> {
        let creds = self.credentials.read().ok()?;
        let entries = creds.get(&provider)?;

        // Filter to available credentials that support the model and haven't been tried
        let candidates: Vec<&AuthRecord> = entries
            .iter()
            .filter(|a| {
                a.is_available()
                    && a.supports_model(model)
                    && !tried.contains(&a.id)
                    && !self.is_cooled_down(&a.id)
                    && check_credential_access(allowed_credentials, a.credential_name.as_deref())
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        let strategy = self.strategy.read().ok().map(|s| *s)?;
        match strategy {
            CredentialStrategy::FillFirst => candidates.first().cloned().cloned(),
            CredentialStrategy::PriorityWeightedRR => {
                self.pick_round_robin(provider, model, &candidates)
            }
            CredentialStrategy::EwmaLatency => self.pick_latency_aware(&candidates),
            CredentialStrategy::LeastInflight
            | CredentialStrategy::StickyHash
            | CredentialStrategy::RandomTwoChoices => {
                // These strategies will be fully implemented in SPEC-050.
                // For now, fall back to round-robin behavior.
                self.pick_round_robin(provider, model, &candidates)
            }
        }
    }

    fn pick_round_robin(
        &self,
        provider: Format,
        model: &str,
        candidates: &[&AuthRecord],
    ) -> Option<AuthRecord> {
        let key = format!("{}:{}", provider.as_str(), model);
        let counters = self.counters.read().ok()?;
        let idx = if let Some(counter) = counters.get(&key) {
            counter.fetch_add(1, Ordering::Relaxed)
        } else {
            drop(counters);
            let mut counters = self.counters.write().ok()?;
            let counter = counters.entry(key).or_insert_with(|| AtomicUsize::new(0));
            counter.fetch_add(1, Ordering::Relaxed)
        };

        // Weighted round-robin: build expanded index based on weights
        let total_weight: u32 = candidates.iter().map(|c| c.weight.max(1)).sum();
        if total_weight == 0 {
            return candidates.first().cloned().cloned();
        }
        let slot = (idx as u32) % total_weight;
        let mut cumulative = 0u32;
        for &c in candidates {
            cumulative += c.weight.max(1);
            if slot < cumulative {
                return Some(c.clone());
            }
        }
        // Fallback (shouldn't reach here)
        Some(candidates[idx % candidates.len()].clone())
    }

    fn pick_latency_aware(&self, candidates: &[&AuthRecord]) -> Option<AuthRecord> {
        if candidates.len() == 1 {
            return candidates.first().cloned().cloned();
        }

        let ewma = self.latency_ewma.read().ok()?;
        let mut best: Option<&AuthRecord> = None;
        let mut best_latency = f64::MAX;

        for &c in candidates {
            let latency = ewma.get(&c.id).copied().unwrap_or(0.0);
            if latency < best_latency {
                best_latency = latency;
                best = Some(c);
            }
        }

        best.cloned()
    }

    /// Record a credential's request latency for EWMA tracking.
    pub fn record_latency(&self, credential_id: &str, latency_ms: f64) {
        let Ok(alpha_guard) = self.ewma_alpha.read() else {
            return;
        };
        let alpha = *alpha_guard;
        drop(alpha_guard);
        let Ok(mut ewma) = self.latency_ewma.write() else {
            return;
        };
        let current = ewma.entry(credential_id.to_string()).or_insert(latency_ms);
        *current = alpha * latency_ms + (1.0 - alpha) * *current;
    }

    /// Record a successful request for a credential.
    pub fn record_success(&self, auth_id: &str) {
        if let Some(auth) = self.find_credential(auth_id) {
            auth.circuit_breaker.record_success();
        }
    }

    /// Record a failure for a credential (circuit breaker).
    pub fn record_failure(&self, auth_id: &str) {
        if let Some(auth) = self.find_credential(auth_id) {
            auth.circuit_breaker.record_failure();
        }
    }

    /// Set a quota cooldown for a credential, temporarily excluding it from selection.
    pub fn set_quota_cooldown(&self, credential_id: &str, duration: Duration) {
        self.cooldowns.insert(
            credential_id.to_string(),
            QuotaCooldown {
                until: Instant::now() + duration,
            },
        );
    }

    /// Check if a credential is currently in quota cooldown.
    pub fn is_cooled_down(&self, credential_id: &str) -> bool {
        if let Some(entry) = self.cooldowns.get(credential_id) {
            if Instant::now() < entry.until {
                return true;
            }
            // Cooldown expired — remove it
            drop(entry);
            self.cooldowns.remove(credential_id);
        }
        false
    }

    /// O(1) credential lookup by ID using the index.
    pub fn find_credential(&self, auth_id: &str) -> Option<AuthRecord> {
        let index = self.credential_index.read().ok()?;
        let &(format, idx) = index.get(auth_id)?;
        let creds = self.credentials.read().ok()?;
        creds.get(&format)?.get(idx).cloned()
    }

    /// Get circuit breaker states for all credentials (for Prometheus).
    pub fn circuit_breaker_states(&self) -> Vec<(String, bool)> {
        let mut states = Vec::new();
        if let Ok(creds) = self.credentials.read() {
            for entries in creds.values() {
                for auth in entries {
                    let name = auth
                        .credential_name
                        .clone()
                        .unwrap_or_else(|| auth.id[..8].to_string());
                    states.push((name, auth.circuit_state() == CircuitState::Open));
                }
            }
        }
        states
    }

    /// Rebuild credentials from config, preserving circuit breaker state.
    pub fn update_from_config(&self, config: &Config) {
        // Update CB config
        if let Ok(mut cb) = self.cb_config.write() {
            *cb = config.circuit_breaker.clone();
        }

        let cb_config = config.circuit_breaker.clone();

        let mut map: HashMap<Format, Vec<AuthRecord>> = HashMap::new();

        // Claude credentials
        for entry in &config.claude_api_key {
            let auth = build_auth_record(entry, Format::Claude, &cb_config);
            map.entry(Format::Claude).or_default().push(auth);
        }

        // OpenAI credentials
        for entry in &config.openai_api_key {
            let auth = build_auth_record(entry, Format::OpenAI, &cb_config);
            map.entry(Format::OpenAI).or_default().push(auth);
        }

        // Gemini credentials
        for entry in &config.gemini_api_key {
            let auth = build_auth_record(entry, Format::Gemini, &cb_config);
            map.entry(Format::Gemini).or_default().push(auth);
        }

        // OpenAI-compatible credentials
        for entry in &config.openai_compatibility {
            let auth = build_auth_record(entry, Format::OpenAICompat, &cb_config);
            map.entry(Format::OpenAICompat).or_default().push(auth);
        }

        if let Ok(mut creds) = self.credentials.write() {
            // Preserve circuit breaker state from existing credentials
            for (format, new_entries) in map.iter_mut() {
                if let Some(old_entries) = creds.get(format) {
                    for new_auth in new_entries.iter_mut() {
                        if let Some(old_auth) =
                            old_entries.iter().find(|o| o.api_key == new_auth.api_key)
                        {
                            new_auth.circuit_breaker = old_auth.circuit_breaker.clone();
                        }
                    }
                }
            }
            *creds = map;

            // Rebuild credential index for O(1) lookups
            if let Ok(mut index) = self.credential_index.write() {
                index.clear();
                for (format, entries) in creds.iter() {
                    for (i, auth) in entries.iter().enumerate() {
                        index.insert(auth.id.clone(), (*format, i));
                    }
                }
            }
        }

        // Update credential strategy from default profile
        if let Ok(mut strategy) = self.strategy.write() {
            let profile_name = &config.routing.default_profile;
            if let Some(profile) = config.routing.profiles.get(profile_name) {
                *strategy = profile.credential_policy.strategy;
            }
        }
    }

    /// Get all available models across all providers.
    pub fn all_models(&self) -> Vec<ModelInfo> {
        let mut models = Vec::new();
        if let Ok(creds) = self.credentials.read() {
            for (format, entries) in creds.iter() {
                for auth in entries {
                    if !auth.is_available() {
                        continue;
                    }
                    for model_entry in &auth.models {
                        let model_id = if let Some(ref alias) = model_entry.alias {
                            alias.clone()
                        } else {
                            model_entry.id.clone()
                        };
                        // Avoid duplicates
                        if !models.iter().any(|m: &ModelInfo| m.id == model_id) {
                            models.push(ModelInfo {
                                id: model_id,
                                provider: format.as_str().to_string(),
                                owned_by: format.as_str().to_string(),
                            });
                        }
                    }
                }
            }
        }
        models
    }

    /// Check if the model name matches any credential that has a prefix configured.
    pub fn model_has_prefix(&self, model: &str) -> bool {
        if let Ok(creds) = self.credentials.read() {
            for entries in creds.values() {
                for auth in entries {
                    if auth.prefix.is_some() && auth.is_available() && auth.supports_model(model) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Resolve which provider(s) can handle a given model name.
    pub fn resolve_providers(&self, model: &str) -> Vec<Format> {
        let mut formats = Vec::new();
        if let Ok(creds) = self.credentials.read() {
            for (format, entries) in creds.iter() {
                for auth in entries {
                    if auth.is_available() && auth.supports_model(model) {
                        if !formats.contains(format) {
                            formats.push(*format);
                        }
                        break;
                    }
                }
            }
        }
        formats
    }

    /// Get a snapshot of all credentials grouped by format.
    /// Used by ProviderCatalog to build inventory snapshots.
    pub fn credential_map(&self) -> HashMap<Format, Vec<AuthRecord>> {
        self.credentials
            .read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }
}

fn build_auth_record(
    entry: &prism_core::config::ProviderKeyEntry,
    format: Format,
    cb_config: &CircuitBreakerConfig,
) -> AuthRecord {
    let models = entry
        .models
        .iter()
        .map(|m| ModelEntry {
            id: m.id.clone(),
            alias: m.alias.clone(),
        })
        .collect();

    let circuit_breaker: Arc<dyn CircuitBreakerPolicy> = if cb_config.enabled {
        Arc::new(ThreeStateCircuitBreaker::new(cb_config.clone()))
    } else {
        Arc::new(NoopCircuitBreaker)
    };

    AuthRecord {
        id: uuid::Uuid::new_v4().to_string(),
        provider: format,
        api_key: entry.api_key.clone(),
        base_url: entry.base_url.clone(),
        proxy_url: entry.proxy_url.clone(),
        headers: entry.headers.clone(),
        models,
        excluded_models: entry.excluded_models.clone(),
        prefix: entry.prefix.clone(),
        disabled: entry.disabled,
        circuit_breaker,
        cloak: if matches!(format, Format::Claude) {
            Some(entry.cloak.clone())
        } else {
            None
        },
        wire_api: entry.wire_api,
        credential_name: entry.name.clone(),
        weight: entry.weight.max(1),
        region: entry.region.clone(),
        vertex: entry.vertex,
        vertex_project: entry.vertex_project.clone(),
        vertex_location: entry.vertex_location.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::routing::config::CredentialStrategy;

    /// Build a test AuthRecord with sensible defaults.
    fn make_auth(id: &str, format: Format, models: Vec<&str>) -> AuthRecord {
        AuthRecord {
            id: id.to_string(),
            provider: format,
            api_key: format!("key-{id}"),
            base_url: None,
            proxy_url: None,
            headers: Default::default(),
            models: models
                .into_iter()
                .map(|m| ModelEntry {
                    id: m.to_string(),
                    alias: None,
                })
                .collect(),
            excluded_models: Vec::new(),
            prefix: None,
            disabled: false,
            circuit_breaker: Arc::new(NoopCircuitBreaker),
            cloak: None,
            wire_api: Default::default(),
            credential_name: Some(id.to_string()),
            weight: 1,
            region: None,
            vertex: false,
            vertex_project: None,
            vertex_location: None,
        }
    }

    fn setup_router(strategy: CredentialStrategy, creds: Vec<AuthRecord>) -> CredentialRouter {
        let router = CredentialRouter::new(strategy);
        let mut map: HashMap<Format, Vec<AuthRecord>> = HashMap::new();
        for auth in creds {
            map.entry(auth.provider).or_default().push(auth);
        }
        *router.credentials.write().unwrap() = map;
        router
    }

    // === FillFirst Strategy ===

    #[test]
    fn test_fill_first_picks_first() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![
                make_auth("a", Format::OpenAI, vec!["gpt-4"]),
                make_auth("b", Format::OpenAI, vec!["gpt-4"]),
            ],
        );
        let picked = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &[])
            .unwrap();
        assert_eq!(picked.id, "a");
    }

    #[test]
    fn test_fill_first_skips_tried() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![
                make_auth("a", Format::OpenAI, vec!["gpt-4"]),
                make_auth("b", Format::OpenAI, vec!["gpt-4"]),
            ],
        );
        let picked = router
            .pick(Format::OpenAI, "gpt-4", &["a".to_string()], None, &[])
            .unwrap();
        assert_eq!(picked.id, "b");
    }

    #[test]
    fn test_fill_first_no_available() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![make_auth("a", Format::OpenAI, vec!["gpt-4"])],
        );
        let picked = router.pick(Format::OpenAI, "gpt-4", &["a".to_string()], None, &[]);
        assert!(picked.is_none());
    }

    #[test]
    fn test_fill_first_wrong_model() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![make_auth("a", Format::OpenAI, vec!["gpt-4"])],
        );
        let picked = router.pick(Format::OpenAI, "gpt-3.5", &[], None, &[]);
        assert!(picked.is_none());
    }

    #[test]
    fn test_fill_first_wrong_provider() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![make_auth("a", Format::OpenAI, vec!["gpt-4"])],
        );
        let picked = router.pick(Format::Claude, "gpt-4", &[], None, &[]);
        assert!(picked.is_none());
    }

    // === RoundRobin Strategy ===

    #[test]
    fn test_round_robin_cycles() {
        let router = setup_router(
            CredentialStrategy::PriorityWeightedRR,
            vec![
                make_auth("a", Format::OpenAI, vec!["gpt-4"]),
                make_auth("b", Format::OpenAI, vec!["gpt-4"]),
                make_auth("c", Format::OpenAI, vec!["gpt-4"]),
            ],
        );

        let first = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &[])
            .unwrap();
        let second = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &[])
            .unwrap();
        let third = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &[])
            .unwrap();
        let fourth = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &[])
            .unwrap();

        assert_eq!(first.id, "a");
        assert_eq!(second.id, "b");
        assert_eq!(third.id, "c");
        assert_eq!(fourth.id, "a"); // Wraps around
    }

    #[test]
    fn test_round_robin_weighted() {
        let mut auth_a = make_auth("a", Format::OpenAI, vec!["gpt-4"]);
        auth_a.weight = 2;
        let auth_b = make_auth("b", Format::OpenAI, vec!["gpt-4"]);

        let router = setup_router(CredentialStrategy::PriorityWeightedRR, vec![auth_a, auth_b]);

        // With weights 2:1, total weight = 3
        // slots: a(0), a(1), b(2)
        let picks: Vec<String> = (0..6)
            .map(|_| {
                router
                    .pick(Format::OpenAI, "gpt-4", &[], None, &[])
                    .unwrap()
                    .id
            })
            .collect();
        assert_eq!(picks, vec!["a", "a", "b", "a", "a", "b"]);
    }

    // === LatencyAware Strategy ===

    #[test]
    fn test_latency_aware_picks_lowest() {
        let router = setup_router(
            CredentialStrategy::EwmaLatency,
            vec![
                make_auth("slow", Format::OpenAI, vec!["gpt-4"]),
                make_auth("fast", Format::OpenAI, vec!["gpt-4"]),
            ],
        );

        router.record_latency("slow", 500.0);
        router.record_latency("fast", 100.0);

        let picked = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &[])
            .unwrap();
        assert_eq!(picked.id, "fast");
    }

    #[test]
    fn test_latency_aware_unrecorded_defaults_to_zero() {
        let router = setup_router(
            CredentialStrategy::EwmaLatency,
            vec![
                make_auth("recorded", Format::OpenAI, vec!["gpt-4"]),
                make_auth("unrecorded", Format::OpenAI, vec!["gpt-4"]),
            ],
        );

        router.record_latency("recorded", 200.0);
        // unrecorded defaults to 0.0, so should be picked
        let picked = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &[])
            .unwrap();
        assert_eq!(picked.id, "unrecorded");
    }

    // === Disabled credentials ===

    #[test]
    fn test_disabled_credential_skipped() {
        let mut disabled = make_auth("disabled", Format::OpenAI, vec!["gpt-4"]);
        disabled.disabled = true;
        let enabled = make_auth("enabled", Format::OpenAI, vec!["gpt-4"]);

        let router = setup_router(CredentialStrategy::FillFirst, vec![disabled, enabled]);

        let picked = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &[])
            .unwrap();
        assert_eq!(picked.id, "enabled");
    }

    // === record_latency / EWMA ===

    #[test]
    fn test_record_latency_ewma() {
        let router = CredentialRouter::new(CredentialStrategy::EwmaLatency);
        // alpha = 0.3 by default

        router.record_latency("cred1", 100.0);
        let ewma = router.latency_ewma.read().unwrap();
        assert!((ewma["cred1"] - 100.0).abs() < 0.01);
        drop(ewma);

        // Second recording: 0.3 * 200 + 0.7 * 100 = 60 + 70 = 130
        router.record_latency("cred1", 200.0);
        let ewma = router.latency_ewma.read().unwrap();
        assert!((ewma["cred1"] - 130.0).abs() < 0.01);
    }

    // === resolve_providers ===

    #[test]
    fn test_resolve_providers() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![
                make_auth("oai", Format::OpenAI, vec!["gpt-4"]),
                make_auth("ds", Format::OpenAICompat, vec!["gpt-4"]),
                make_auth("claude", Format::Claude, vec!["claude-3"]),
            ],
        );

        let providers = router.resolve_providers("gpt-4");
        assert!(providers.contains(&Format::OpenAI));
        assert!(providers.contains(&Format::OpenAICompat));
        assert!(!providers.contains(&Format::Claude));
    }

    #[test]
    fn test_resolve_providers_no_match() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![make_auth("a", Format::OpenAI, vec!["gpt-4"])],
        );
        let providers = router.resolve_providers("nonexistent-model");
        assert!(providers.is_empty());
    }

    // === all_models ===

    #[test]
    fn test_all_models() {
        let mut auth_with_alias = make_auth("a", Format::OpenAI, vec!["gpt-4"]);
        auth_with_alias.models[0].alias = Some("my-gpt4".to_string());

        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![
                auth_with_alias,
                make_auth("b", Format::Claude, vec!["claude-3"]),
            ],
        );

        let models = router.all_models();
        assert_eq!(models.len(), 2);
        // Alias should be used as the id
        assert!(models.iter().any(|m| m.id == "my-gpt4"));
        assert!(models.iter().any(|m| m.id == "claude-3"));
    }

    #[test]
    fn test_all_models_dedup() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![
                make_auth("a", Format::OpenAI, vec!["gpt-4"]),
                make_auth("b", Format::OpenAI, vec!["gpt-4"]),
            ],
        );

        let models = router.all_models();
        assert_eq!(models.len(), 1);
    }

    // === model_has_prefix ===

    #[test]
    fn test_model_has_prefix() {
        let mut auth = make_auth("a", Format::OpenAI, vec!["gpt-4"]);
        auth.prefix = Some("myprefix".to_string());

        let router = setup_router(CredentialStrategy::FillFirst, vec![auth]);

        assert!(router.model_has_prefix("gpt-4"));
        assert!(!router.model_has_prefix("nonexistent"));
    }

    // === check_credential_access ===

    #[test]
    fn test_credential_access_empty_allows_all() {
        assert!(check_credential_access(&[], Some("any-name")));
        assert!(check_credential_access(&[], None));
    }

    #[test]
    fn test_credential_access_unnamed_excluded() {
        assert!(!check_credential_access(&["my-*".to_string()], None));
    }

    #[test]
    fn test_credential_access_glob_match() {
        let patterns = vec!["my-claude-*".to_string(), "shared-*".to_string()];
        assert!(check_credential_access(&patterns, Some("my-claude-key1")));
        assert!(check_credential_access(&patterns, Some("shared-team")));
        assert!(!check_credential_access(&patterns, Some("other-key")));
    }

    #[test]
    fn test_credential_access_exact_match() {
        let patterns = vec!["exact-key".to_string()];
        assert!(check_credential_access(&patterns, Some("exact-key")));
        assert!(!check_credential_access(&patterns, Some("exact-key-2")));
    }

    // === allowed_credentials filtering in pick ===

    #[test]
    fn test_pick_with_allowed_credentials() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![
                make_auth("a", Format::OpenAI, vec!["gpt-4"]),
                make_auth("b", Format::OpenAI, vec!["gpt-4"]),
            ],
        );

        // With restriction, only "b" matches
        let picked = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &["b".to_string()])
            .unwrap();
        assert_eq!(picked.id, "b");

        // With restriction that matches nothing
        let picked = router.pick(
            Format::OpenAI,
            "gpt-4",
            &[],
            None,
            &["nonexistent".to_string()],
        );
        assert!(picked.is_none());
    }

    // === Quota cooldown ===

    #[test]
    fn test_set_and_check_cooldown() {
        let router = CredentialRouter::new(CredentialStrategy::FillFirst);
        assert!(!router.is_cooled_down("cred-1"));

        router.set_quota_cooldown("cred-1", Duration::from_secs(60));
        assert!(router.is_cooled_down("cred-1"));
        assert!(!router.is_cooled_down("cred-2"));
    }

    #[test]
    fn test_cooldown_expires() {
        let router = CredentialRouter::new(CredentialStrategy::FillFirst);

        // Set a very short cooldown
        router.set_quota_cooldown("cred-1", Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(5));
        assert!(!router.is_cooled_down("cred-1"));
    }

    #[test]
    fn test_cooled_down_credential_skipped_in_pick() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![
                make_auth("a", Format::OpenAI, vec!["gpt-4"]),
                make_auth("b", Format::OpenAI, vec!["gpt-4"]),
            ],
        );

        // Cool down credential "a"
        router.set_quota_cooldown("a", Duration::from_secs(60));

        // Should skip "a" and pick "b"
        let picked = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &[])
            .unwrap();
        assert_eq!(picked.id, "b");
    }

    #[test]
    fn test_all_credentials_cooled_down_returns_none() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![
                make_auth("a", Format::OpenAI, vec!["gpt-4"]),
                make_auth("b", Format::OpenAI, vec!["gpt-4"]),
            ],
        );

        router.set_quota_cooldown("a", Duration::from_secs(60));
        router.set_quota_cooldown("b", Duration::from_secs(60));

        let picked = router.pick(Format::OpenAI, "gpt-4", &[], None, &[]);
        assert!(picked.is_none());
    }

    #[test]
    fn test_cooldown_expired_credential_available_again() {
        let router = setup_router(
            CredentialStrategy::FillFirst,
            vec![make_auth("a", Format::OpenAI, vec!["gpt-4"])],
        );

        router.set_quota_cooldown("a", Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(5));

        let picked = router
            .pick(Format::OpenAI, "gpt-4", &[], None, &[])
            .unwrap();
        assert_eq!(picked.id, "a");
    }

    #[test]
    fn test_cooldown_override_extends_duration() {
        let router = CredentialRouter::new(CredentialStrategy::FillFirst);

        router.set_quota_cooldown("cred-1", Duration::from_millis(1));
        // Override with a longer cooldown
        router.set_quota_cooldown("cred-1", Duration::from_secs(60));
        assert!(router.is_cooled_down("cred-1"));
    }
}
