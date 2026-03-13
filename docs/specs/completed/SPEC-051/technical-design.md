# Technical Design: Execution Controller & Dispatch Cutover

| Field     | Value                                               |
|-----------|-----------------------------------------------------|
| Spec ID   | SPEC-051                                            |
| Title     | Execution Controller & Dispatch Cutover              |
| Author    | Claude                                              |
| Status    | Draft                                               |
| Created   | 2026-03-14                                          |
| Updated   | 2026-03-14                                          |

## Overview

Replace the inline routing loop in `dispatch.rs` with an `ExecutionController` that consumes a `RoutePlan` and executes attempts in stage order: credential retry -> provider failover -> model fallback. Each stage has independent limits. Depends on SPEC-048 (types), SPEC-049 (planner), SPEC-050 (health + selectors).

## API Design

### Modified Response Headers (debug mode)

```
x-prism-route-id: <uuid>
x-prism-route-summary: profile=balanced provider=openai credential=prod-us-1 model=gpt-5
x-prism-route-attempts: 2
x-prism-route-fallback: false
```

Replaces current `x-debug-*` headers.

## Backend Implementation

### Module Structure

```
crates/server/src/dispatch/
├── mod.rs              # dispatch() entry point (simplified)
├── features.rs         # Extract RouteRequestFeatures from DispatchRequest
├── executor.rs         # ExecutionController
├── helpers.rs          # (existing, updated)
├── streaming.rs        # (existing, unchanged)
└── retry.rs            # Remove (replaced by ExecutionController)
```

### Features Extraction (`features.rs`)

```rust
pub fn extract_features(req: &DispatchRequest) -> RouteRequestFeatures {
    RouteRequestFeatures {
        requested_model: req.model.clone(),
        endpoint: match req.source_format { /* ... */ },
        source_format: req.source_format,
        tenant_id: req.tenant_id.clone(),
        api_key_id: req.api_key_id.clone(),
        region: req.client_region.clone(),
        stream: req.stream,
        headers: /* extract from req */,
    }
}
```

### ExecutionController (`executor.rs`)

```rust
pub struct ExecutionController<'a> {
    state: &'a AppState,
    health: &'a HealthManager,
    catalog: &'a ProviderCatalog,
}

pub struct ExecutionResult {
    pub response: Response<Body>,
    pub trace: RouteTrace,
}

impl<'a> ExecutionController<'a> {
    pub async fn execute(
        &self,
        plan: &RoutePlan,
        req: &DispatchRequest,
    ) -> Result<ExecutionResult, ProxyError> {
        let failover_config = &plan.failover;
        let mut trace = plan.trace.clone();

        // Stage 1: Walk the model chain
        for (model_idx, model) in plan.model_chain.iter().enumerate() {
            if model_idx >= failover_config.model_attempts as usize {
                break;
            }

            // Stage 2: Walk providers for this model
            let provider_attempts = self.attempts_for_model(&plan.attempts, model);
            for (prov_idx, provider_group) in provider_attempts.iter().enumerate() {
                if prov_idx >= failover_config.provider_attempts as usize {
                    break;
                }

                // Stage 3: Walk credentials for this provider
                for (cred_idx, attempt) in provider_group.iter().enumerate() {
                    if cred_idx >= failover_config.credential_attempts as usize {
                        break;
                    }

                    // Check retry budget
                    if model_idx > 0 || prov_idx > 0 || cred_idx > 0 {
                        if !self.health.retry_budget_allows() {
                            trace.add_event("retry_budget_exhausted");
                            return Err(ProxyError::RetryBudgetExhausted);
                        }
                        self.health.record_retry();
                    }

                    // Execute single attempt with per-try timeout
                    match self.execute_attempt(attempt, req, &mut trace).await {
                        Ok(response) => return Ok(ExecutionResult { response, trace }),
                        Err(err) => {
                            trace.fallback_events.push(RouteFallbackEvent {
                                from_model: model.clone(),
                                to_model: model.clone(),
                                reason: format!("{err}"),
                            });
                            continue;
                        }
                    }
                }
            }

            // Model fallback event
            if model_idx + 1 < plan.model_chain.len() {
                trace.fallback_events.push(RouteFallbackEvent {
                    from_model: model.clone(),
                    to_model: plan.model_chain[model_idx + 1].clone(),
                    reason: "all_providers_exhausted".into(),
                });
            }
        }

        Err(ProxyError::AllAttemptsExhausted { trace })
    }

    async fn execute_attempt(
        &self,
        attempt: &RouteAttemptPlan,
        req: &DispatchRequest,
        trace: &mut RouteTrace,
    ) -> Result<Response<Body>, ProxyError> {
        // 1. Find credential from catalog
        let (format, auth) = self.catalog.find_credential(&attempt.credential_id)?;

        // 2. Record attempt start
        self.health.record_attempt_start(&attempt.credential_id);

        let start = Instant::now();

        // 3. Get executor
        let executor = self.state.executors.get(&format)?;

        // 4. Translate request (reuse existing translation logic)
        let translated = self.translate_request(req, &auth, &attempt.model)?;

        // 5. Apply cloaking + payload rules (reuse existing logic)
        let final_payload = self.apply_transforms(translated, &auth, req)?;

        // 6. Execute with per-try timeout
        let per_try_timeout = Duration::from_secs(30); // configurable
        let result = tokio::time::timeout(
            per_try_timeout,
            if req.stream {
                executor.execute_stream(/* ... */)
            } else {
                executor.execute(/* ... */)
            }
        ).await;

        // 7. Record result
        let latency = start.elapsed().as_millis() as f64;
        let attempt_result = match &result {
            Ok(Ok(_)) => AttemptResult::success(latency),
            Ok(Err(e)) => AttemptResult::from_error(e, latency),
            Err(_) => AttemptResult::timeout(latency),
        };
        self.health.record_attempt_result(&attempt.credential_id, &attempt_result);

        result.map_err(|_| ProxyError::Timeout)?
    }
}
```

### Simplified dispatch entry point (`mod.rs`)

```rust
pub async fn dispatch(state: &AppState, req: DispatchRequest) -> Result<Response<Body>, ProxyError> {
    // 1. Extract features
    let features = extract_features(&req);

    // 2. Plan route (pure)
    let config = state.config.load();
    let inventory = state.catalog.snapshot();
    let health = state.health.snapshot();
    let plan = RoutePlanner::plan(&features, &config.routing, &inventory, &health);

    // 3. Check cache (existing logic, reuse)
    if let Some(cached) = try_cache(&req, &plan) {
        return Ok(cached);
    }

    // 4. Execute plan
    let controller = ExecutionController::new(state);
    let result = controller.execute(&plan, &req).await?;

    // 5. Write to request log with trace
    state.request_log.record(/* ... include result.trace ... */);

    // 6. Add debug headers if requested
    if req.debug {
        inject_route_headers(&mut result.response, &result.trace);
    }

    Ok(result.response)
}
```

### Changes to Existing Code

1. **Remove** inline routing loop from `dispatch.rs` (provider iteration, `router.pick()` calls)
2. **Remove** `dispatch/retry.rs` (replaced by ExecutionController stage logic)
3. **Remove** `DispatchDebug` struct (replaced by `RouteTrace`)
4. **Update** `helpers.rs`: replace `inject_debug_headers` with `inject_route_headers`
5. **Keep** `streaming.rs` (SSE translation, keepalive, usage capture unchanged)
6. **Update** `AppState` to hold `HealthManager` + `ProviderCatalog` instead of `CredentialRouter`

### Flow

```
DispatchRequest
    |
    v
[extract_features] --> RouteRequestFeatures
    |
    v
[RoutePlanner::plan] --> RoutePlan (pure, from SPEC-049)
    |
    v
[try_cache] --> cache hit? return early
    |
    v
[ExecutionController::execute]
    |
    ├─ Model Chain Loop (model_attempts limit)
    │   ├─ Provider Loop (provider_attempts limit)
    │   │   ├─ Credential Loop (credential_attempts limit)
    │   │   │   ├─ retry_budget check
    │   │   │   ├─ health.record_attempt_start()
    │   │   │   ├─ translate + cloak + payload rules
    │   │   │   ├─ executor.execute() with per-try timeout
    │   │   │   ├─ health.record_attempt_result()
    │   │   │   └─ success? return / failure? continue
    │   │   └─ all creds exhausted -> next provider
    │   └─ all providers exhausted -> next model (fallback event)
    └─ all models exhausted -> AllAttemptsExhausted
    |
    v
[record request log with RouteTrace]
    |
    v
[inject route debug headers if debug mode]
    |
    v
Response
```

## Configuration Changes

No new config fields. Consumes `FailoverConfig` from SPEC-048 profiles.

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | Translation and execution unchanged |
| Claude   | Yes       | Cloaking logic reused as-is |
| Gemini   | Yes       | Translation and execution unchanged |

## Task Breakdown

- [ ] Create `dispatch/features.rs` with `extract_features()`
- [ ] Create `dispatch/executor.rs` with `ExecutionController`
- [ ] Implement stage-aware failover loop (credential -> provider -> model)
- [ ] Implement per-try timeout
- [ ] Integrate retry budget checks
- [ ] Rewrite `dispatch/mod.rs` to use planner + executor pattern
- [ ] Update `AppState` to hold `HealthManager` + `ProviderCatalog`
- [ ] Update debug headers to use `RouteTrace`
- [ ] Update request log to include route trace data
- [ ] Remove `dispatch/retry.rs`
- [ ] Remove `DispatchDebug`, inline routing loop
- [ ] Unit tests: extract_features maps all DispatchRequest fields correctly
- [ ] Unit tests: extract_features handles missing optional fields
- [ ] Integration tests: single attempt success — no failover triggered
- [ ] Integration tests: credential failover — first credential fails, second succeeds within same provider
- [ ] Integration tests: provider failover — all credentials of first provider fail, second provider succeeds
- [ ] Integration tests: model fallback — all providers fail for primary model, fallback model succeeds
- [ ] Integration tests: credential-attempts limit — stops trying credentials after limit reached
- [ ] Integration tests: provider-attempts limit — stops trying providers after limit reached
- [ ] Integration tests: model-attempts limit — stops trying models after limit reached
- [ ] Integration tests: retry budget exhaustion — returns RetryBudgetExhausted when budget is spent
- [ ] Integration tests: per-try timeout — slow executor triggers timeout, failover to next attempt
- [ ] Integration tests: streaming failover — stream attempt fails, failover to next credential
- [ ] Integration tests: cache hit short-circuits — no executor called when cache hits
- [ ] Integration tests: health feedback loop — failed attempt updates health, next plan reflects unhealthy credential
- [ ] Integration tests: all attempts exhausted — returns AllAttemptsExhausted with complete trace
- [ ] Integration tests: route trace completeness — trace contains all attempted credentials with results
- [ ] Integration tests: debug headers — response contains x-prism-route-id, x-prism-route-summary
- [ ] Integration tests: request log — trace data written to request log store
- [ ] Regression tests: existing /v1/chat/completions behavior unchanged for single-provider config
- [ ] Regression tests: existing /v1/messages behavior unchanged
- [ ] Regression tests: existing streaming + keepalive behavior unchanged

## Test Strategy

- **Unit tests:**
  - Feature extraction: all `DispatchRequest` fields mapped to `RouteRequestFeatures`. Missing optional fields default correctly.
- **Integration tests (mock executors):**
  - **Success path:** Single attempt succeeds, response returned, health updated with success, trace shows 1 attempt.
  - **Credential failover:** Mock executor fails for credential-1, succeeds for credential-2. Verify attempt order matches plan, health updated for both.
  - **Provider failover:** All credentials of provider-1 fail (up to credential-attempts limit), provider-2 credential succeeds. Trace shows cross-provider fallback event.
  - **Model fallback:** All providers fail for model-1, model-2 succeeds. Trace shows model fallback event.
  - **Stage limits:** Verify credential-attempts, provider-attempts, model-attempts limits are respected exactly.
  - **Retry budget:** Exhaust budget mid-failover, verify `RetryBudgetExhausted` error with partial trace.
  - **Per-try timeout:** Mock executor sleeps beyond timeout, verify timeout error, failover to next attempt.
  - **Streaming failover:** Stream executor returns error, failover to next credential with non-stream or stream retry.
  - **Cache interaction:** Pre-populate cache, verify executor never called, response served from cache.
  - **Health feedback loop:** Execute a request that fails (updates health), execute a second request, verify planner avoids the failed credential.
  - **All exhausted:** All attempts fail, verify `AllAttemptsExhausted` error with complete trace listing every attempt.
  - **Debug headers:** `x-prism-route-id` is a valid UUID, `x-prism-route-summary` contains provider/credential/model.
  - **Request log:** After dispatch, `RequestLogStore` contains entry with route trace data.
- **Regression tests:**
  - Single-provider config: `/v1/chat/completions`, `/v1/messages`, `/v1/responses` produce same response shape as before.
  - Streaming + keepalive: SSE stream with keepalive spaces works as before.
- **Manual verification:** None needed (all automated).

## Rollout Plan

1. Replace dispatch entry point with planner + executor pattern
2. Remove inline routing loop and retry.rs in the same commit
