use super::config::{ProviderStrategy, RouteProfile};
use super::match_engine;
use super::model_resolver;
use super::types::*;
use crate::glob::glob_match;
use crate::provider::Format;
use crate::routing::config::RoutingConfig;
use std::collections::HashMap;

// ─── Inventory snapshot ────────────────────────────────────────────────────

/// Point-in-time snapshot of available providers and credentials.
#[derive(Debug, Clone, Default)]
pub struct InventorySnapshot {
    pub providers: Vec<ProviderEntry>,
}

#[derive(Debug, Clone)]
pub struct ProviderEntry {
    pub format: Format,
    pub name: String,
    pub credentials: Vec<CredentialEntry>,
}

#[derive(Debug, Clone)]
pub struct CredentialEntry {
    pub id: String,
    pub name: String,
    pub models: Vec<String>,
    pub excluded_models: Vec<String>,
    pub region: Option<String>,
    pub weight: u32,
    pub disabled: bool,
}

// ─── Health snapshot ───────────────────────────────────────────────────────

/// Point-in-time snapshot of health state.
#[derive(Debug, Clone, Default)]
pub struct HealthSnapshot {
    pub credentials: HashMap<String, CredentialHealth>,
}

#[derive(Debug, Clone)]
pub struct CredentialHealth {
    pub circuit_open: bool,
    pub ejected: bool,
    pub inflight: u64,
    pub ewma_latency_ms: f64,
    pub ewma_cost_micro_usd: f64,
    pub cooldown_active: bool,
}

impl Default for CredentialHealth {
    fn default() -> Self {
        Self {
            circuit_open: false,
            ejected: false,
            inflight: 0,
            ewma_latency_ms: 0.0,
            ewma_cost_micro_usd: 0.0,
            cooldown_active: false,
        }
    }
}

// ─── Planner ───────────────────────────────────────────────────────────────

pub struct RoutePlanner;

impl RoutePlanner {
    /// Build a deterministic route plan from immutable inputs.
    /// Pure function — no side effects, no I/O.
    pub fn plan(
        features: &RouteRequestFeatures,
        config: &RoutingConfig,
        inventory: &InventorySnapshot,
        health: &HealthSnapshot,
    ) -> RoutePlan {
        // 1. Resolve profile via match engine
        let (profile_name, profile) = match_engine::resolve_profile(features, config);
        let matched_rule =
            match_engine::match_rule(features, &config.rules).map(|r| r.name.clone());

        // 2. Resolve model chain
        let resolved =
            model_resolver::resolve_model(&features.requested_model, &config.model_resolution);

        let mut trace = RouteTrace {
            matched_rule: matched_rule.clone(),
            resolved_profile: profile_name.to_string(),
            model_resolution_steps: resolved.resolution_steps,
            ..Default::default()
        };

        // 3. For each model in chain, find eligible candidates
        let mut all_candidates = Vec::new();
        let mut all_rejections = Vec::new();

        for model in &resolved.model_chain {
            collect_candidates(
                model,
                &resolved.pinned_providers,
                features,
                inventory,
                health,
                &mut all_candidates,
                &mut all_rejections,
            );
        }

        // Record in trace
        trace.candidates = all_candidates
            .iter()
            .map(|c| RouteCandidate {
                provider: c.provider_name.clone(),
                credential_id: c.credential_id.clone(),
                credential_name: c.credential_name.clone(),
                model: c.model.clone(),
            })
            .collect();
        trace.rejections = all_rejections;

        // 4. Score and rank candidates
        let mut scored = score_candidates(&all_candidates, profile, health);
        // Stable sort to preserve deterministic ordering
        scored.sort_by(|a, b| {
            b.score
                .weight
                .partial_cmp(&a.score.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Assign ranks
        for (i, entry) in scored.iter_mut().enumerate() {
            entry.rank = (i + 1) as u32;
        }

        trace.scoring = scored
            .iter()
            .map(|s| RouteScoringEntry {
                candidate: format!("{}/{}", s.provider_name, s.credential_name),
                score: s.score.clone(),
                rank: s.rank,
            })
            .collect();

        // 5. Build attempt list
        let attempts: Vec<RouteAttemptPlan> = scored
            .iter()
            .map(|s| RouteAttemptPlan {
                model: s.model.clone(),
                provider: s.format,
                credential_id: s.credential_id.clone(),
                credential_name: s.credential_name.clone(),
                rank: s.rank,
                score: s.score.clone(),
            })
            .collect();

        RoutePlan {
            profile: profile_name.to_string(),
            model_chain: resolved.model_chain,
            attempts,
            trace,
        }
    }
}

// ─── Internal candidate ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct CandidateInfo {
    format: Format,
    provider_name: String,
    credential_id: String,
    credential_name: String,
    model: String,
    weight: u32,
    _region: Option<String>,
}

#[derive(Debug, Clone)]
struct ScoredCandidate {
    format: Format,
    provider_name: String,
    credential_id: String,
    credential_name: String,
    model: String,
    score: RouteScore,
    rank: u32,
}

fn collect_candidates(
    model: &str,
    pinned_providers: &Option<Vec<String>>,
    features: &RouteRequestFeatures,
    inventory: &InventorySnapshot,
    health: &HealthSnapshot,
    candidates: &mut Vec<CandidateInfo>,
    rejections: &mut Vec<RouteRejection>,
) {
    for provider in &inventory.providers {
        // Check provider pin
        if pinned_providers
            .as_ref()
            .is_some_and(|pins| !pins.iter().any(|p| glob_match(p, &provider.name)))
        {
            rejections.push(RouteRejection {
                candidate: provider.name.clone(),
                reason: RejectReason::ProviderPinExcluded,
            });
            continue;
        }

        for cred in &provider.credentials {
            let cand_label = format!("{}/{}", provider.name, cred.name);

            // Disabled
            if cred.disabled {
                rejections.push(RouteRejection {
                    candidate: cand_label,
                    reason: RejectReason::CredentialDisabled,
                });
                continue;
            }

            // Model support
            let supports =
                cred.models.is_empty() || cred.models.iter().any(|m| glob_match(m, model));
            let excluded = cred.excluded_models.iter().any(|m| glob_match(m, model));
            if !supports || excluded {
                rejections.push(RouteRejection {
                    candidate: cand_label,
                    reason: RejectReason::ModelNotSupported,
                });
                continue;
            }

            // Region mismatch (if request and credential both specify region)
            if let (Some(req_region), Some(cred_region)) = (&features.region, &cred.region)
                && !glob_match(cred_region, req_region)
                && !glob_match(req_region, cred_region)
            {
                rejections.push(RouteRejection {
                    candidate: cand_label,
                    reason: RejectReason::RegionMismatch,
                });
                continue;
            }

            // Health checks
            if let Some(ch) = health.credentials.get(&cred.id) {
                if ch.circuit_open {
                    rejections.push(RouteRejection {
                        candidate: cand_label,
                        reason: RejectReason::CircuitBreakerOpen,
                    });
                    continue;
                }
                if ch.ejected {
                    rejections.push(RouteRejection {
                        candidate: cand_label,
                        reason: RejectReason::OutlierEjected,
                    });
                    continue;
                }
                if ch.cooldown_active {
                    rejections.push(RouteRejection {
                        candidate: cand_label,
                        reason: RejectReason::CooldownActive,
                    });
                    continue;
                }
            }

            candidates.push(CandidateInfo {
                format: provider.format,
                provider_name: provider.name.clone(),
                credential_id: cred.id.clone(),
                credential_name: cred.name.clone(),
                model: model.to_string(),
                weight: cred.weight,
                _region: cred.region.clone(),
            });
        }
    }
}

fn score_candidates(
    candidates: &[CandidateInfo],
    profile: &RouteProfile,
    health: &HealthSnapshot,
) -> Vec<ScoredCandidate> {
    candidates
        .iter()
        .map(|c| {
            let ch = health.credentials.get(&c.credential_id);
            let weight = compute_weight(c, profile, ch);
            let latency_ms = ch.map(|h| h.ewma_latency_ms);
            let inflight = ch.map(|h| h.inflight);
            let estimated_cost = ch.map(|h| h.ewma_cost_micro_usd);

            ScoredCandidate {
                format: c.format,
                provider_name: c.provider_name.clone(),
                credential_id: c.credential_id.clone(),
                credential_name: c.credential_name.clone(),
                model: c.model.clone(),
                score: RouteScore {
                    weight,
                    latency_ms,
                    inflight,
                    estimated_cost,
                    health_penalty: 0.0,
                },
                rank: 0,
            }
        })
        .collect()
}

fn compute_weight(
    candidate: &CandidateInfo,
    profile: &RouteProfile,
    health: Option<&CredentialHealth>,
) -> f64 {
    let base = candidate.weight as f64;
    match profile.provider_policy.strategy {
        ProviderStrategy::WeightedRoundRobin => {
            // Use configured weight, fall back to credential weight
            let w = profile
                .provider_policy
                .weights
                .get(&candidate.provider_name)
                .copied()
                .unwrap_or(candidate.weight);
            w as f64
        }
        ProviderStrategy::EwmaLatency => {
            if let Some(h) = health.filter(|h| h.ewma_latency_ms > 0.0) {
                return 1000.0 / h.ewma_latency_ms;
            }
            base
        }
        ProviderStrategy::LowestEstimatedCost => {
            if let Some(h) = health.filter(|h| h.ewma_cost_micro_usd > 0.0) {
                return 1_000_000.0 / h.ewma_cost_micro_usd;
            }
            base
        }
        ProviderStrategy::OrderedFallback => {
            // Weight by position in order list (first = highest weight)
            let order = &profile.provider_policy.order;
            if order.is_empty() {
                return base;
            }
            match order.iter().position(|p| p == &candidate.provider_name) {
                Some(pos) => (order.len() - pos) as f64 * 1000.0,
                None => 0.0, // Not in order list
            }
        }
        ProviderStrategy::StickyHash => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::config::RoutingConfig;
    use crate::routing::types::RouteEndpoint;
    use std::collections::BTreeMap;

    fn test_features(model: &str) -> RouteRequestFeatures {
        RouteRequestFeatures {
            requested_model: model.to_string(),
            endpoint: RouteEndpoint::ChatCompletions,
            source_format: Format::OpenAI,
            tenant_id: None,
            api_key_id: None,
            region: None,
            stream: false,
            headers: BTreeMap::new(),
        }
    }

    fn test_inventory() -> InventorySnapshot {
        InventorySnapshot {
            providers: vec![
                ProviderEntry {
                    format: Format::OpenAI,
                    name: "openai".to_string(),
                    credentials: vec![CredentialEntry {
                        id: "cred-openai-1".to_string(),
                        name: "prod-openai".to_string(),
                        models: vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()],
                        excluded_models: vec![],
                        region: None,
                        weight: 100,
                        disabled: false,
                    }],
                },
                ProviderEntry {
                    format: Format::Claude,
                    name: "claude".to_string(),
                    credentials: vec![CredentialEntry {
                        id: "cred-claude-1".to_string(),
                        name: "prod-claude".to_string(),
                        models: vec!["claude-3-opus".to_string()],
                        excluded_models: vec![],
                        region: None,
                        weight: 100,
                        disabled: false,
                    }],
                },
            ],
        }
    }

    fn healthy() -> HealthSnapshot {
        HealthSnapshot::default()
    }

    #[test]
    fn test_plan_basic() {
        let features = test_features("gpt-4");
        let config = RoutingConfig::default();
        let inventory = test_inventory();
        let health = healthy();

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        assert_eq!(plan.profile, "balanced");
        assert_eq!(plan.model_chain, vec!["gpt-4"]);
        assert!(!plan.attempts.is_empty());
        assert_eq!(plan.attempts[0].model, "gpt-4");
    }

    #[test]
    fn test_plan_determinism() {
        let features = test_features("gpt-4");
        let config = RoutingConfig::default();
        let inventory = test_inventory();
        let health = healthy();

        let plan1 = RoutePlanner::plan(&features, &config, &inventory, &health);
        for _ in 0..100 {
            let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
            assert_eq!(plan.attempts.len(), plan1.attempts.len());
            for (a, b) in plan.attempts.iter().zip(plan1.attempts.iter()) {
                assert_eq!(a.credential_id, b.credential_id);
                assert_eq!(a.model, b.model);
                assert_eq!(a.rank, b.rank);
            }
        }
    }

    #[test]
    fn test_plan_empty_inventory() {
        let features = test_features("gpt-4");
        let config = RoutingConfig::default();
        let inventory = InventorySnapshot::default();
        let health = healthy();

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        assert!(plan.attempts.is_empty());
        assert!(plan.trace.candidates.is_empty());
    }

    #[test]
    fn test_plan_all_credentials_unhealthy() {
        let features = test_features("gpt-4");
        let config = RoutingConfig::default();
        let inventory = test_inventory();
        let mut health = HealthSnapshot::default();
        health.credentials.insert(
            "cred-openai-1".to_string(),
            CredentialHealth {
                circuit_open: true,
                ..Default::default()
            },
        );

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        assert!(plan.attempts.is_empty());
        assert!(
            plan.trace
                .rejections
                .iter()
                .any(|r| r.reason == RejectReason::CircuitBreakerOpen)
        );
    }

    #[test]
    fn test_plan_provider_pin_excludes() {
        let features = test_features("gpt-4");
        let mut config = RoutingConfig::default();
        config
            .model_resolution
            .provider_pins
            .push(crate::routing::config::ProviderPin {
                pattern: "gpt-*".to_string(),
                providers: vec!["openai".to_string()],
            });
        let inventory = test_inventory();
        let health = healthy();

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        // Claude should be excluded
        assert!(
            plan.trace
                .rejections
                .iter()
                .any(|r| r.reason == RejectReason::ProviderPinExcluded)
        );
        assert!(plan.attempts.iter().all(|a| a.provider == Format::OpenAI));
    }

    #[test]
    fn test_plan_disabled_credential() {
        let features = test_features("gpt-4");
        let config = RoutingConfig::default();
        let mut inventory = test_inventory();
        inventory.providers[0].credentials[0].disabled = true;
        let health = healthy();

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        assert!(
            plan.trace
                .rejections
                .iter()
                .any(|r| r.reason == RejectReason::CredentialDisabled)
        );
    }

    #[test]
    fn test_plan_model_not_supported() {
        let features = test_features("unknown-model");
        let config = RoutingConfig::default();
        let inventory = test_inventory();
        let health = healthy();

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        // All credentials should reject with ModelNotSupported
        assert!(plan.attempts.is_empty());
        assert!(
            plan.trace
                .rejections
                .iter()
                .any(|r| r.reason == RejectReason::ModelNotSupported)
        );
    }

    #[test]
    fn test_plan_region_mismatch() {
        let mut features = test_features("gpt-4");
        features.region = Some("eu-west-1".to_string());
        let config = RoutingConfig::default();
        let mut inventory = test_inventory();
        inventory.providers[0].credentials[0].region = Some("us-east-1".to_string());
        let health = healthy();

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        assert!(
            plan.trace
                .rejections
                .iter()
                .any(|r| r.reason == RejectReason::RegionMismatch)
        );
    }

    #[test]
    fn test_plan_outlier_ejected() {
        let features = test_features("gpt-4");
        let config = RoutingConfig::default();
        let inventory = test_inventory();
        let mut health = HealthSnapshot::default();
        health.credentials.insert(
            "cred-openai-1".to_string(),
            CredentialHealth {
                ejected: true,
                ..Default::default()
            },
        );

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        assert!(
            plan.trace
                .rejections
                .iter()
                .any(|r| r.reason == RejectReason::OutlierEjected)
        );
    }

    #[test]
    fn test_plan_cooldown_active() {
        let features = test_features("gpt-4");
        let config = RoutingConfig::default();
        let inventory = test_inventory();
        let mut health = HealthSnapshot::default();
        health.credentials.insert(
            "cred-openai-1".to_string(),
            CredentialHealth {
                cooldown_active: true,
                ..Default::default()
            },
        );

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        assert!(
            plan.trace
                .rejections
                .iter()
                .any(|r| r.reason == RejectReason::CooldownActive)
        );
    }

    #[test]
    fn test_plan_latency_scoring() {
        let features = test_features("gpt-4");
        let config = RoutingConfig {
            default_profile: "lowest-latency".to_string(),
            ..Default::default()
        };

        let inventory = InventorySnapshot {
            providers: vec![ProviderEntry {
                format: Format::OpenAI,
                name: "openai".to_string(),
                credentials: vec![
                    CredentialEntry {
                        id: "fast".to_string(),
                        name: "fast".to_string(),
                        models: vec!["gpt-4".to_string()],
                        excluded_models: vec![],
                        region: None,
                        weight: 100,
                        disabled: false,
                    },
                    CredentialEntry {
                        id: "slow".to_string(),
                        name: "slow".to_string(),
                        models: vec!["gpt-4".to_string()],
                        excluded_models: vec![],
                        region: None,
                        weight: 100,
                        disabled: false,
                    },
                ],
            }],
        };

        let mut health = HealthSnapshot::default();
        health.credentials.insert(
            "fast".to_string(),
            CredentialHealth {
                ewma_latency_ms: 50.0,
                ..Default::default()
            },
        );
        health.credentials.insert(
            "slow".to_string(),
            CredentialHealth {
                ewma_latency_ms: 500.0,
                ..Default::default()
            },
        );

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        assert_eq!(plan.attempts.len(), 2);
        // Fast should rank higher (lower latency = higher weight)
        assert_eq!(plan.attempts[0].credential_id, "fast");
        assert_eq!(plan.attempts[1].credential_id, "slow");
    }

    #[test]
    fn test_plan_with_fallback_chain() {
        let features = test_features("gpt-4");
        let mut config = RoutingConfig::default();
        config
            .model_resolution
            .fallbacks
            .push(crate::routing::config::ModelFallback {
                pattern: "gpt-4".to_string(),
                to: vec!["gpt-3.5-turbo".to_string()],
            });

        let inventory = test_inventory();
        let health = healthy();

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        assert_eq!(plan.model_chain, vec!["gpt-4", "gpt-3.5-turbo"]);
        // Should have candidates for both models
        assert!(!plan.attempts.is_empty());
    }

    #[test]
    fn test_plan_empty_models_means_all() {
        let features = test_features("anything-goes");
        let config = RoutingConfig::default();
        let inventory = InventorySnapshot {
            providers: vec![ProviderEntry {
                format: Format::OpenAI,
                name: "openai".to_string(),
                credentials: vec![CredentialEntry {
                    id: "cred-1".to_string(),
                    name: "prod".to_string(),
                    models: vec![], // empty = supports all models
                    excluded_models: vec![],
                    region: None,
                    weight: 100,
                    disabled: false,
                }],
            }],
        };
        let health = healthy();

        let plan = RoutePlanner::plan(&features, &config, &inventory, &health);
        assert_eq!(plan.attempts.len(), 1);
    }
}
