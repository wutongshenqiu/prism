# Technical Design: Health State & Selection Strategies

| Field     | Value                                         |
|-----------|-----------------------------------------------|
| Spec ID   | SPEC-050                                      |
| Title     | Health State & Selection Strategies            |
| Author    | Claude                                        |
| Status    | Draft                                         |
| Created   | 2026-03-14                                    |
| Updated   | 2026-03-14                                    |

## Overview

Replace `CredentialRouter` with three focused components: `HealthManager` (mutable runtime state), `ProviderCatalog` (inventory), and trait-based selection strategies. Depends on SPEC-048 types and SPEC-049 snapshot types.

## API Design

No new HTTP endpoints. Internal APIs only.

## Backend Implementation

### Module Structure

```
crates/provider/src/
├── catalog.rs              # ProviderCatalog (inventory management)
├── health.rs               # HealthManager (mutable health state)
├── provider_selector.rs    # Provider selection strategies
├── credential_selector.rs  # Credential selection strategies
└── routing.rs              # Remove or reduce to re-exports
```

### HealthManager (`health.rs`)

```rust
pub struct HealthManager {
    states: RwLock<HashMap<String, CredentialHealthState>>,
    retry_budget: RetryBudgetState,
}

struct CredentialHealthState {
    // Circuit breaker
    circuit: ThreeStateCircuitBreaker,
    // Outlier detection
    consecutive_5xx: u32,
    consecutive_local_failures: u32,
    // Ejection
    ejected: bool,
    eject_until: Option<Instant>,
    eject_count: u32, // for exponential backoff
    // Inflight
    inflight: AtomicU64,
    // EWMA
    ewma_latency_ms: f64,
    ewma_cost_micro_usd: f64,
    ewma_alpha: f64,
    // Rate tracking (sliding window)
    recent_429: SlidingWindowCounter,
    recent_5xx: SlidingWindowCounter,
    recent_total: SlidingWindowCounter,
    // Cooldown
    cooldown_until: Option<Instant>,
}

pub struct RetryBudgetState {
    config: RetryBudgetConfig,
    recent_requests: SlidingWindowCounter,
    recent_retries: SlidingWindowCounter,
}

/// Simple sliding window counter
struct SlidingWindowCounter {
    window_seconds: u64,
    buckets: Vec<AtomicU64>,
    // ...
}

impl HealthManager {
    /// Create snapshot for planner consumption (read-only)
    pub fn snapshot(&self) -> HealthSnapshot;

    /// Record attempt start (increment inflight)
    pub fn record_attempt_start(&self, credential_id: &str);

    /// Record attempt end with result
    pub fn record_attempt_result(&self, credential_id: &str, result: &AttemptResult);

    /// Check if retry budget allows another retry
    pub fn retry_budget_allows(&self) -> bool;

    /// Record a retry attempt against the budget
    pub fn record_retry(&self);

    /// Initialize health state for a credential
    pub fn register_credential(&self, credential_id: &str, config: &HealthConfig);

    /// Remove health state for a removed credential
    pub fn unregister_credential(&self, credential_id: &str);

    /// Update health config (on config reload)
    pub fn update_config(&self, config: &HealthConfig);
}

pub struct AttemptResult {
    pub latency_ms: f64,
    pub cost_micro_usd: Option<u64>,
    pub status: AttemptStatus,
}

pub enum AttemptStatus {
    Success,
    RateLimit,    // 429
    ServerError,  // 5xx
    NetworkError,
    Timeout,
    ClientError,  // 4xx (not 429)
}
```

Outlier ejection logic:
- On consecutive 5xx >= threshold OR consecutive local failures >= threshold: eject
- Ejection duration: `min(base_eject * 2^eject_count, max_eject)`
- On success: reset consecutive counters, clear ejection

### ProviderCatalog (`catalog.rs`)

```rust
pub struct ProviderCatalog {
    providers: RwLock<Vec<ProviderEntry>>,
}

impl ProviderCatalog {
    /// Create inventory snapshot for planner
    pub fn snapshot(&self) -> InventorySnapshot;

    /// Rebuild from config (on reload)
    pub fn update_from_config(&self, config: &Config);

    /// Find credential by ID
    pub fn find_credential(&self, id: &str) -> Option<(Format, AuthRecord)>;
}
```

### Provider Selection (`provider_selector.rs`)

```rust
pub trait ProviderSelector: Send + Sync {
    fn select(
        &self,
        candidates: &[ProviderCandidate],
        context: &SelectionContext,
    ) -> Vec<ProviderCandidate>; // ordered by preference
}

pub struct ProviderCandidate {
    pub format: Format,
    pub name: String,
    pub weight: u32,
    pub health: AggregatedProviderHealth, // aggregated from credentials
}

pub struct SelectionContext {
    pub sticky_key: Option<String>,
    pub request_features: RouteRequestFeatures,
}

// Implementations
pub struct OrderedFallbackSelector { pub order: Vec<String> }
pub struct WeightedRoundRobinSelector { pub counter: AtomicU64 }
pub struct EwmaLatencySelector;
pub struct LowestEstimatedCostSelector;
pub struct StickyHashSelector;
```

### Credential Selection (`credential_selector.rs`)

```rust
pub trait CredentialSelector: Send + Sync {
    fn select(
        &self,
        candidates: &[CredentialCandidate],
        context: &SelectionContext,
    ) -> Vec<CredentialCandidate>; // ordered by preference
}

pub struct CredentialCandidate {
    pub id: String,
    pub name: String,
    pub weight: u32,
    pub priority: u32,
    pub health: CredentialHealth,
}

// Implementations
pub struct PriorityWeightedRRSelector { pub counter: AtomicU64 }
pub struct FillFirstSelector;
pub struct LeastInflightSelector;
pub struct EwmaLatencyCredSelector;
pub struct StickyHashCredSelector;
pub struct RandomTwoChoicesSelector;
```

Priority tiering (applies to all credential strategies):
```rust
fn apply_priority_tiers(candidates: &[CredentialCandidate]) -> Vec<Vec<CredentialCandidate>> {
    // Group by priority, sort groups by priority (lower = higher priority)
    // Within each tier, apply the credential strategy
    // Only fall to next tier if current tier is exhausted (all unhealthy)
}
```

### Integration with Planner

The planner (SPEC-049) calls selectors with snapshot data:

```rust
// In planner.rs, scoring step:
fn score_candidates(
    profile: &RouteProfile,
    providers: Vec<ProviderCandidate>,
    credentials: HashMap<String, Vec<CredentialCandidate>>,
) -> Vec<RouteAttemptPlan> {
    let provider_selector = build_provider_selector(&profile.provider_policy);
    let credential_selector = build_credential_selector(&profile.credential_policy);

    let ordered_providers = provider_selector.select(&providers, &ctx);
    for provider in ordered_providers {
        let creds = credentials.get(&provider.name);
        let ordered_creds = credential_selector.select(creds, &ctx);
        // Build RouteAttemptPlan entries
    }
}
```

### Changes to Existing Code

1. **Remove** `CredentialRouter` (replaced by ProviderCatalog + HealthManager + selectors)
2. **Remove** `routing.rs` round-robin counter, latency_ewma, strategy-specific pick methods
3. **Migrate** circuit breaker state preservation logic to `HealthManager.update_config()`
4. **Migrate** `all_models()`, `resolve_providers()` to `ProviderCatalog`

## Configuration Changes

No new config fields. Consumes `HealthConfig` and `FailoverConfig` from SPEC-048.

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | Selectors are provider-agnostic |
| Claude   | Yes       | Selectors are provider-agnostic |
| Gemini   | Yes       | Selectors are provider-agnostic |

## Task Breakdown

- [ ] Implement `SlidingWindowCounter` utility
- [ ] Implement `HealthManager` with all 8 health signals
- [ ] Implement `HealthSnapshot` generation
- [ ] Implement `RetryBudgetState`
- [ ] Implement `ProviderCatalog` with `update_from_config()` and `snapshot()`
- [ ] Implement `ProviderSelector` trait + 5 strategies
- [ ] Implement `CredentialSelector` trait + 6 strategies
- [ ] Implement priority tiering logic
- [ ] Integrate selectors into planner scoring step
- [ ] Remove `CredentialRouter`
- [ ] Unit tests: circuit breaker state transitions (Closed->Open on threshold, Open->HalfOpen on cooldown, HalfOpen->Closed on success, HalfOpen->Open on failure)
- [ ] Unit tests: outlier detection (consecutive 5xx triggers ejection, consecutive local failures triggers ejection, success resets counters)
- [ ] Unit tests: ejection timing (exponential backoff: base * 2^count, capped at max)
- [ ] Unit tests: ejection recovery (success after eject clears ejection, resets eject count)
- [ ] Unit tests: EWMA latency convergence (alpha=1.0 gives instant, alpha=0.0 gives no change)
- [ ] Unit tests: EWMA cost tracking
- [ ] Unit tests: sliding window counter (increment, window expiry, rate calculation)
- [ ] Unit tests: inflight counter (increment on start, decrement on result)
- [ ] Unit tests: retry budget — allows when under ratio
- [ ] Unit tests: retry budget — blocks when over ratio
- [ ] Unit tests: retry budget — min-retries-per-second floor
- [ ] Unit tests: snapshot reflects current state accurately
- [ ] Unit tests: register/unregister credential lifecycle
- [ ] Unit tests: OrderedFallbackSelector — respects configured order
- [ ] Unit tests: WeightedRoundRobinSelector — distributes proportionally over N calls
- [ ] Unit tests: EwmaLatencySelector — picks lowest latency provider
- [ ] Unit tests: LowestEstimatedCostSelector — picks cheapest healthy provider
- [ ] Unit tests: StickyHashSelector — same key always selects same provider
- [ ] Unit tests: PriorityWeightedRRSelector — stays in highest tier until exhausted
- [ ] Unit tests: FillFirstSelector — always picks first available in highest tier
- [ ] Unit tests: LeastInflightSelector — picks credential with fewest inflight
- [ ] Unit tests: EwmaLatencyCredSelector — picks lowest latency credential
- [ ] Unit tests: StickyHashCredSelector — same key always selects same credential
- [ ] Unit tests: RandomTwoChoicesSelector — picks better of two random candidates
- [ ] Unit tests: priority tiering — groups by priority, only falls to next tier when current exhausted
- [ ] Unit tests: all selectors — empty candidates returns empty result
- [ ] Unit tests: all selectors — single candidate returns that candidate
- [ ] Concurrency tests: HealthManager under concurrent record_attempt_start/result from multiple tasks
- [ ] Concurrency tests: snapshot consistency during concurrent writes

## Test Strategy

- **Unit tests:**
  - **Circuit breaker:** All 4 state transitions with exact threshold values. Verify cooldown timing.
  - **Outlier detection:** Consecutive failure counting, ejection trigger, exponential backoff duration (`base * 2^count` capped at max), reset on success.
  - **EWMA:** Convergence behavior with alpha=0.0, 0.5, 1.0. Verify formula `new = alpha * current + (1-alpha) * previous`.
  - **Sliding window:** Counter increment, bucket expiry after window passes, rate calculation accuracy.
  - **Inflight:** Atomic increment on start, decrement on result. Verify counter never goes negative.
  - **Retry budget:** Allow/block based on ratio, min-retries-per-second floor, sliding window tracking.
  - **Snapshot:** Generated snapshot matches current mutable state for all fields.
  - **Provider selectors (5):** Each strategy with 3+ providers, verify ordering. Edge cases: empty list, all equal weights/latency, single provider.
  - **Credential selectors (6):** Each strategy with 3+ credentials, verify ordering. Edge cases: empty list, single credential, all same priority.
  - **Priority tiering:** 3 tiers with mixed health, verify tier-1 used first, tier-2 only when tier-1 exhausted, tier-3 only when tier-1+2 exhausted.
- **Concurrency tests:** Spawn 10 tokio tasks concurrently calling `record_attempt_start`/`record_attempt_result` on same HealthManager. Verify no panics, no data races, snapshot remains internally consistent.
- **Manual verification:** None needed.

## Rollout Plan

1. Implement HealthManager and ProviderCatalog
2. Implement selectors
3. Wire into planner
4. Remove CredentialRouter
