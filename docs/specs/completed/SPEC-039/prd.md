# SPEC-039: Routing & Auth Key Enhancement

## Problem

1. Auth keys can only restrict models, not which credentials are used
2. Routing strategy is global — no per-model configuration
3. No server-side model fallback chain; relies on client `models` array
4. Per-key rate limits (rpm/tpm/cost) are configured but not enforced

## Features

### Feature 1: Auth Key Credential Restriction
Allow `allowed-credentials` on auth keys to restrict which provider credentials a key can use (glob matching by credential `name`).

### Feature 2: Per-Model Routing Strategy
Support `model-strategies` map in routing config for model-specific routing strategies (glob pattern matching).

### Feature 3: Server-Side Model Fallback Chain
Support `model-fallbacks` map in routing config to define server-side fallback models, appended after any client-provided `models` array.

### Feature 4: Per-Key Rate Limit Enforcement
Enforce per-key `rate_limit` (rpm/tpm/cost_per_day_usd) and `budget` settings that are already configurable but not checked at runtime.

## Success Criteria

- All existing tests pass
- New unit tests for each feature
- `cargo clippy --workspace --tests -- -D warnings` clean
- Backward-compatible: empty/missing new fields preserve existing behavior
