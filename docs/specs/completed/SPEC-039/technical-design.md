# SPEC-039: Technical Design

## Feature 1: Auth Key Credential Restriction
- Add `allowed_credentials: Vec<String>` to `AuthKeyEntry`
- Add `check_credential_access()` function using glob matching
- Filter credentials in `CredentialRouter::pick()` via new `allowed_credentials` parameter
- Pass through `DispatchRequest` from auth key context

## Feature 2: Per-Model Routing Strategy
- Add `model_strategies: HashMap<String, RoutingStrategy>` to `RoutingConfig`
- Add `resolve_strategy()` method: exact match → glob match → default
- Update `CredentialRouter` to store and use per-model strategies

## Feature 3: Server-Side Model Fallback
- Add `model_fallbacks: HashMap<String, Vec<String>>` to `RoutingConfig`
- Add `resolve_fallbacks()` method with glob support
- Append server fallbacks to model chain in dispatch, deduplicated

## Feature 4: Per-Key Rate Limit Enforcement
- Add `check_key_with_limit()` to `SlidingWindowLimiter` and `CostLimiter`
- Add `check_key_overrides()` and `check_budget()` to `CompositeRateLimiter`
- Enforce in rate limit middleware using auth key from request context
- Record tokens and cost in dispatch helpers and streaming Drop handler
