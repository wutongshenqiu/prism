# Technical Design: Routing Config & Core Types

| Field     | Value                              |
|-----------|------------------------------------|
| Spec ID   | SPEC-048                           |
| Title     | Routing Config & Core Types        |
| Author    | Claude                             |
| Status    | Draft                              |
| Created   | 2026-03-14                         |
| Updated   | 2026-03-14                         |

## Overview

Replace the current flat `RoutingConfig` / `RoutingStrategy` with a layered, profile-based config model. All types are pure data with serde support. No runtime behavior changes.

## API Design

No new API endpoints in this spec.

## Backend Implementation

### Module Structure

```
crates/core/src/
├── routing/
│   ├── mod.rs          # re-exports
│   ├── config.rs       # RoutingConfig, RouteProfile, policies, rules, model resolution
│   └── types.rs        # RouteRequestFeatures, RoutePlan, RouteTrace, etc.
└── config.rs           # top-level Config references routing::RoutingConfig
```

### Key Types

#### Config Types (`routing/config.rs`)

```rust
/// Top-level routing configuration
pub struct RoutingConfig {
    /// Default profile name (must exist in profiles map)
    pub default_profile: String,
    /// Named routing profiles
    pub profiles: HashMap<String, RouteProfile>,
    /// Request-to-profile matching rules (evaluated in order)
    pub rules: Vec<RouteRule>,
    /// Model resolution config
    pub model_resolution: ModelResolution,
}

/// A complete routing policy
pub struct RouteProfile {
    pub provider_policy: ProviderPolicy,
    pub credential_policy: CredentialPolicy,
    pub health: HealthConfig,
    pub failover: FailoverConfig,
}

/// Provider-level selection policy
pub struct ProviderPolicy {
    pub strategy: ProviderStrategy,
    /// Optional sticky key expression (e.g. "tenant-id")
    pub sticky_key: Option<String>,
    /// Provider weights (provider name -> weight)
    pub weights: HashMap<String, u32>,
    /// Explicit provider ordering (for ordered-fallback)
    pub order: Vec<String>,
}

pub enum ProviderStrategy {
    OrderedFallback,
    WeightedRoundRobin,
    EwmaLatency,
    LowestEstimatedCost,
    StickyHash,
}

/// Credential-level selection policy
pub struct CredentialPolicy {
    pub strategy: CredentialStrategy,
}

pub enum CredentialStrategy {
    PriorityWeightedRR,
    FillFirst,
    LeastInflight,
    EwmaLatency,
    StickyHash,
    RandomTwoChoices,
}

/// Health monitoring configuration
pub struct HealthConfig {
    pub circuit_breaker: CircuitBreakerHealthConfig,
    pub outlier_detection: OutlierDetectionConfig,
}

pub struct CircuitBreakerHealthConfig {
    pub enabled: bool,
    pub failure_threshold: u32,
    pub cooldown_seconds: u64,
}

pub struct OutlierDetectionConfig {
    pub consecutive_5xx: u32,
    pub consecutive_local_failures: u32,
    pub base_eject_seconds: u64,
    pub max_eject_seconds: u64,
}

/// Failover configuration
pub struct FailoverConfig {
    pub credential_attempts: u32,
    pub provider_attempts: u32,
    pub model_attempts: u32,
    pub retry_budget: RetryBudgetConfig,
    pub retry_on: Vec<RetryCondition>,
}

pub struct RetryBudgetConfig {
    /// Max ratio of retries to total requests
    pub ratio: f64,
    /// Minimum retries per second regardless of ratio
    pub min_retries_per_second: u32,
}

pub enum RetryCondition {
    Network,
    RateLimit,   // 429
    ServerError, // 5xx
}

/// Request matching rule
pub struct RouteRule {
    pub name: String,
    pub priority: Option<i32>,
    pub match_conditions: RouteMatch,
    pub use_profile: String,
}

pub struct RouteMatch {
    pub models: Vec<String>,       // glob patterns
    pub tenants: Vec<String>,      // glob patterns
    pub endpoints: Vec<String>,    // e.g. "chat-completions"
    pub regions: Vec<String>,
    pub stream: Option<bool>,
    pub headers: HashMap<String, Vec<String>>,
}

/// Model resolution configuration
pub struct ModelResolution {
    pub aliases: Vec<ModelAlias>,
    pub rewrites: Vec<ModelRewrite>,
    pub fallbacks: Vec<ModelFallback>,
    pub provider_pins: Vec<ProviderPin>,
}

pub struct ModelAlias {
    pub from: String,
    pub to: String,
}

pub struct ModelRewrite {
    pub pattern: String, // glob
    pub to: String,
}

pub struct ModelFallback {
    pub pattern: String, // glob
    pub to: Vec<String>,
}

pub struct ProviderPin {
    pub pattern: String, // glob
    pub providers: Vec<String>,
}
```

#### Runtime Types (`routing/types.rs`)

```rust
/// Extracted request features for route planning
pub struct RouteRequestFeatures {
    pub requested_model: String,
    pub endpoint: RouteEndpoint,
    pub source_format: Format,
    pub tenant_id: Option<String>,
    pub api_key_id: Option<String>,
    pub region: Option<String>,
    pub stream: bool,
    pub headers: BTreeMap<String, String>,
}

pub enum RouteEndpoint {
    ChatCompletions,
    Messages,
    Responses,
    Models,
}

/// Complete route plan (pure data, no side effects)
pub struct RoutePlan {
    pub profile: String,
    pub model_chain: Vec<String>,
    pub attempts: Vec<RouteAttemptPlan>,
    pub trace: RouteTrace,
}

pub struct RouteAttemptPlan {
    pub model: String,
    pub provider: Format,
    pub credential_id: String,
    pub credential_name: String,
    pub rank: u32,
    pub score: RouteScore,
}

pub struct RouteScore {
    pub weight: f64,
    pub latency_ms: Option<f64>,
    pub inflight: Option<u64>,
    pub estimated_cost: Option<f64>,
    pub health_penalty: f64,
}

/// Full trace of routing decision
pub struct RouteTrace {
    pub matched_rule: Option<String>,
    pub resolved_profile: String,
    pub model_resolution_steps: Vec<ModelResolutionStep>,
    pub candidates: Vec<RouteCandidate>,
    pub rejections: Vec<RouteRejection>,
    pub scoring: Vec<RouteScoringEntry>,
    pub fallback_events: Vec<RouteFallbackEvent>,
}

pub struct RouteCandidate {
    pub provider: String,
    pub credential_id: String,
    pub credential_name: String,
    pub model: String,
}

pub enum ModelResolutionStep {
    AliasResolved { from: String, to: String },
    RewriteApplied { from: String, to: String, rule: String },
    FallbackChainBuilt { primary: String, fallbacks: Vec<String> },
    ProviderPinned { model: String, providers: Vec<String> },
}

pub struct RouteRejection {
    pub candidate: String,
    pub reason: RejectReason,
}

pub enum RejectReason {
    ModelNotSupported,
    RegionMismatch,
    ProviderPinExcluded,
    CircuitBreakerOpen,
    OutlierEjected,
    CredentialDisabled,
    AccessDenied,
    CooldownActive,
}

pub struct RouteScoringEntry {
    pub candidate: String,
    pub score: RouteScore,
    pub rank: u32,
}

pub struct RouteFallbackEvent {
    pub from_model: String,
    pub to_model: String,
    pub reason: String,
}
```

### Preset Profiles

```rust
impl RoutingConfig {
    pub fn default_profiles() -> HashMap<String, RouteProfile> {
        // balanced, stable, lowest-latency, lowest-cost
        // See TD section 7.1 for definitions
    }
}
```

| Preset | Provider Strategy | Credential Strategy | Failover |
|--------|-------------------|---------------------|----------|
| Balanced | WeightedRoundRobin | PriorityWeightedRR | 2/2/2 |
| Stable | OrderedFallback | FillFirst | 1/1/1 |
| Lowest Latency | EwmaLatency | LeastInflight | 2/2/1 |
| Lowest Cost | LowestEstimatedCost | PriorityWeightedRR | 1/2/1 |

### Changes to Existing Code

1. **Remove** `RoutingStrategy` enum from `config.rs`
2. **Replace** `RoutingConfig` in `config.rs` with new profile-based version
3. **Remove** `ModelRewriteRule` from `config.rs` (replaced by `ModelResolution`)
4. **Replace** `Config.routing` field type with new `routing::RoutingConfig`
5. **Remove** `model_strategies`, `model_fallbacks`, `model_rewrites` from config

## Configuration Changes

Old config (removed):

```yaml
routing:
  strategy: round-robin
  fallback_enabled: true
  ewma_alpha: 0.3
  default_region: us-east
  model_strategies: {}
  model_fallbacks: {}
  model_rewrites: []
```

New config:

```yaml
routing:
  default-profile: balanced
  profiles:
    balanced:
      provider-policy:
        strategy: weighted-round-robin
        weights:
          openai: 100
          claude: 100
      credential-policy:
        strategy: priority-weighted-rr
      health:
        circuit-breaker:
          enabled: true
          failure-threshold: 5
          cooldown-seconds: 30
        outlier-detection:
          consecutive-5xx: 3
          consecutive-local-failures: 2
          base-eject-seconds: 30
          max-eject-seconds: 300
      failover:
        credential-attempts: 2
        provider-attempts: 2
        model-attempts: 2
        retry-budget:
          ratio: 0.2
          min-retries-per-second: 5
        retry-on: [network, 429, 5xx]
  rules: []
  model-resolution:
    aliases: []
    rewrites: []
    fallbacks: []
    provider-pins: []
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | No provider-specific config type changes |
| Claude   | Yes       | No provider-specific config type changes |
| Gemini   | Yes       | No provider-specific config type changes |

## Task Breakdown

- [ ] Create `crates/core/src/routing/mod.rs`
- [ ] Create `crates/core/src/routing/config.rs` with all config types + serde
- [ ] Create `crates/core/src/routing/types.rs` with all runtime types + serde
- [ ] Implement 4 preset profile factory methods
- [ ] Implement `Default` for `RoutingConfig` (uses balanced preset)
- [ ] Replace `RoutingConfig`, `RoutingStrategy`, `ModelRewriteRule` in `config.rs`
- [ ] Update `Config.routing` to reference new type
- [ ] Unit tests: YAML deserialization, serialization round-trip, preset defaults
- [ ] Unit tests: config validation (rule references non-existent profile, empty profiles map)
- [ ] Unit tests: default/missing field handling (omitted optional fields deserialize to defaults)
- [ ] Unit tests: strategy + parameter consistency (ordered-fallback requires non-empty order, weighted-rr requires non-empty weights)
- [ ] Unit tests: model resolution config (alias, rewrite, fallback, provider-pin serialization)
- [ ] Unit tests: `serde(deny_unknown_fields)` rejects unknown keys

## Test Strategy

- **Unit tests:**
  - Config deserialization from YAML: full config, minimal config, each preset profile
  - Round-trip serialization: serialize -> deserialize -> assert equality
  - Preset factory: each of 4 presets produces valid, non-empty config
  - Default values: omitted fields get correct defaults (e.g., `credential-attempts` defaults to 1)
  - Validation: rule referencing missing profile returns error, empty profiles map returns error
  - Strategy parameter consistency: `ordered-fallback` without `order` returns error, `weighted-round-robin` without `weights` returns error
  - Unknown fields: YAML with unknown keys is rejected
- **Integration tests:** None (types only)
- **Manual verification:** None needed

## Rollout Plan

1. Replace config types in a single commit
2. Update all downstream references (CredentialRouter, dispatch) in the same commit to compile
