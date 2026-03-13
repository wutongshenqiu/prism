# Technical Design: Route Planner & Match Engine

| Field     | Value                                    |
|-----------|------------------------------------------|
| Spec ID   | SPEC-049                                 |
| Title     | Route Planner & Match Engine             |
| Author    | Claude                                   |
| Status    | Draft                                    |
| Created   | 2026-03-14                               |
| Updated   | 2026-03-14                               |

## Overview

Implement the pure route planning layer: match engine, model resolver, planner, and explainer. All functions take immutable data and return deterministic results. Depends on SPEC-048 types.

## API Design

No new HTTP endpoints in this spec (Preview API is SPEC-052).

### Internal API

```rust
// Entry point
impl RoutePlanner {
    pub fn plan(
        features: &RouteRequestFeatures,
        config: &RoutingConfig,
        inventory: &InventorySnapshot,
        health: &HealthSnapshot,
    ) -> RoutePlan;
}
```

## Backend Implementation

### Module Structure

```
crates/core/src/routing/
├── mod.rs
├── config.rs           # (from SPEC-048)
├── types.rs            # (from SPEC-048)
├── match_engine.rs     # Rule matching with specificity
├── model_resolver.rs   # Alias, rewrite, fallback, pin
├── planner.rs          # Pure route planner
└── explain.rs          # Structured explanation builder
```

### Key Types

#### Inventory & Health Snapshots

```rust
/// Point-in-time snapshot of available providers and credentials
pub struct InventorySnapshot {
    pub providers: Vec<ProviderEntry>,
}

pub struct ProviderEntry {
    pub format: Format,
    pub name: String, // e.g. "openai", "claude"
    pub credentials: Vec<CredentialEntry>,
}

pub struct CredentialEntry {
    pub id: String,
    pub name: String,
    pub models: Vec<String>,
    pub excluded_models: Vec<String>,
    pub region: Option<String>,
    pub weight: u32,
    pub priority: u32,
    pub disabled: bool,
}

/// Point-in-time snapshot of health state
pub struct HealthSnapshot {
    pub credentials: HashMap<String, CredentialHealth>,
}

pub struct CredentialHealth {
    pub circuit_state: CircuitState,
    pub ejected: bool,
    pub eject_until: Option<Instant>,
    pub inflight: u64,
    pub ewma_latency_ms: f64,
    pub ewma_cost_micro_usd: f64,
    pub recent_429_rate: f64,
    pub recent_5xx_rate: f64,
    pub cooldown_until: Option<Instant>,
}

pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}
```

### Match Engine (`match_engine.rs`)

```rust
pub fn match_rule<'a>(
    features: &RouteRequestFeatures,
    rules: &'a [RouteRule],
) -> Option<&'a RouteRule>;

pub fn resolve_profile<'a>(
    features: &RouteRequestFeatures,
    config: &'a RoutingConfig,
) -> (&'a str, &'a RouteProfile);
```

Specificity calculation:
1. Exact model > glob model > no model constraint
2. More match dimensions > fewer match dimensions
3. Exact tenant/header/region > wildcard
4. Higher explicit `priority` wins ties
5. Declaration order breaks remaining ties

### Model Resolver (`model_resolver.rs`)

```rust
pub struct ResolvedModel {
    pub model_chain: Vec<String>,     // primary + fallbacks
    pub pinned_providers: Option<Vec<String>>,
    pub resolution_steps: Vec<ModelResolutionStep>,
}

pub fn resolve_model(
    requested: &str,
    resolution: &ModelResolution,
) -> ResolvedModel;
```

Resolution order:
1. Apply alias (exact match only, single pass)
2. Apply rewrite (glob match, first match wins)
3. Build fallback chain (glob match, primary model is first)
4. Apply provider pin (glob match)

### Planner (`planner.rs`)

```rust
pub struct RoutePlanner;

impl RoutePlanner {
    pub fn plan(
        features: &RouteRequestFeatures,
        config: &RoutingConfig,
        inventory: &InventorySnapshot,
        health: &HealthSnapshot,
    ) -> RoutePlan {
        // 1. Resolve profile via match engine
        // 2. Resolve model chain
        // 3. For each model in chain:
        //    a. Filter eligible providers (model support, pin, health)
        //    b. Filter eligible credentials per provider (model support, region, health, access)
        //    c. Record rejections with reasons
        //    d. Score and rank candidates using profile strategy
        // 4. Build ordered RouteAttemptPlan list
        // 5. Build RouteTrace
    }
}
```

### Explain (`explain.rs`)

```rust
pub struct RouteExplanation {
    pub profile: String,
    pub matched_rule: Option<String>,
    pub model_chain: Vec<String>,
    pub selected: Option<SelectedRoute>,
    pub alternates: Vec<SelectedRoute>,
    pub rejections: Vec<RouteRejection>,
}

pub struct SelectedRoute {
    pub provider: String,
    pub credential_name: String,
    pub model: String,
    pub score: RouteScore,
}

pub fn explain(plan: &RoutePlan) -> RouteExplanation;
```

### Flow

```
RouteRequestFeatures
    |
    v
[Match Engine] -- rules + specificity --> profile name
    |
    v
[Model Resolver] -- aliases, rewrites, fallbacks, pins --> model chain
    |
    v
[Planner] -- inventory + health snapshots --> RoutePlan
    |
    v
[Explain] -- RoutePlan --> RouteExplanation (structured JSON)
```

## Configuration Changes

No config changes (uses SPEC-048 types).

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | Planner is provider-agnostic |
| Claude   | Yes       | Planner is provider-agnostic |
| Gemini   | Yes       | Planner is provider-agnostic |

## Task Breakdown

- [ ] Create `crates/core/src/routing/match_engine.rs` with specificity-based rule matching
- [ ] Create `crates/core/src/routing/model_resolver.rs` with alias/rewrite/fallback/pin
- [ ] Define `InventorySnapshot` and `HealthSnapshot` types
- [ ] Create `crates/core/src/routing/planner.rs` with pure planning logic
- [ ] Create `crates/core/src/routing/explain.rs` with explanation builder
- [ ] Unit tests: match engine specificity (exact > glob, more dims > fewer)
- [ ] Unit tests: match engine — no rule matched falls back to default profile
- [ ] Unit tests: match engine — multiple dimensions beat single dimension
- [ ] Unit tests: match engine — priority field overrides specificity tie
- [ ] Unit tests: match engine — declaration order breaks final tie
- [ ] Unit tests: model resolver — alias resolution (single pass, no chaining)
- [ ] Unit tests: model resolver — rewrite with glob pattern (first match wins)
- [ ] Unit tests: model resolver — fallback chain construction
- [ ] Unit tests: model resolver — provider pin filters providers
- [ ] Unit tests: model resolver — no matching alias/rewrite returns original
- [ ] Unit tests: planner determinism (same input -> same output, run 100x)
- [ ] Unit tests: planner — empty inventory returns empty attempts with trace
- [ ] Unit tests: planner — all credentials unhealthy returns all rejected
- [ ] Unit tests: planner — provider pin excludes non-pinned providers
- [ ] Unit tests: planner — region mismatch produces RegionMismatch rejection
- [ ] Unit tests: planner — disabled credential produces CredentialDisabled rejection
- [ ] Unit tests: planner — circuit breaker open produces CircuitBreakerOpen rejection
- [ ] Unit tests: planner — model not supported produces ModelNotSupported rejection
- [ ] Unit tests: planner — scoring ranks candidates correctly per strategy
- [ ] Unit tests: explain output matches plan data

## Test Strategy

- **Unit tests:**
  - **Match engine:** Specificity-based rule selection with all precedence levels. Cases: exact model > glob model, more dimensions > fewer, priority tiebreak, declaration order tiebreak, no rule matched -> default profile, empty rules list -> default profile.
  - **Model resolver:** Alias (exact match only, no alias chaining), rewrite (glob, first match wins), fallback chain (glob), provider pin (glob). Edge cases: no match returns original, multiple rewrites only first applies.
  - **Planner determinism:** Same `(features, config, inventory, health)` inputs produce identical `RoutePlan` across 100 invocations.
  - **Planner edge cases:** Empty inventory, all credentials unhealthy, all providers pinned-away, single candidate, model chain with 0 fallbacks.
  - **Planner rejections:** Each `RejectReason` variant produced under the correct condition.
  - **Planner scoring:** Candidates ranked correctly for each provider/credential strategy given known health snapshot values.
  - **Explain:** Output fields match plan data; rejections list matches plan rejections.
- **Integration tests:** None (pure functions only)
- **Manual verification:** None needed

## Rollout Plan

1. Implement and test all pure planner code
2. No runtime impact until SPEC-051 wires it into dispatch
