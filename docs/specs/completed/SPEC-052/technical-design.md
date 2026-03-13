# Technical Design: Preview API & Dashboard UX

| Field     | Value                                    |
|-----------|------------------------------------------|
| Spec ID   | SPEC-052                                 |
| Title     | Preview API & Dashboard UX               |
| Author    | Claude                                   |
| Status    | Draft                                    |
| Created   | 2026-03-14                               |
| Updated   | 2026-03-14                               |

## Overview

Add dashboard API endpoints for routing config management and route preview/explain. Build a new dashboard routing page with presets, rules, advanced editor, and live preview. Depends on all prior specs (SPEC-048 through SPEC-051).

## API Design

### Endpoints

```
GET    /api/dashboard/routing       # Get routing config
PATCH  /api/dashboard/routing       # Update routing config
POST   /api/dashboard/routing/preview   # Preview route plan
POST   /api/dashboard/routing/explain   # Explain route decision
```

Uses new profile-based config shape.

### Preview Request

```json
{
  "model": "gpt-5",
  "endpoint": "chat-completions",
  "source_format": "openai",
  "tenant_id": "enterprise-acme",
  "api_key_id": "sk-proxy-123",
  "region": "us-east",
  "stream": false,
  "headers": {
    "x-job-class": "interactive"
  }
}
```

### Preview Response

```json
{
  "profile": "balanced",
  "matched_rule": "enterprise-latency",
  "model_chain": ["gpt-5", "gpt-5-mini", "claude-sonnet-4-5"],
  "selected": {
    "provider": "openai",
    "credential_name": "prod-openai-us-1",
    "model": "gpt-5",
    "score": {
      "weight": 100.0,
      "latency_ms": 245.3,
      "inflight": 12,
      "health_penalty": 0.0
    }
  },
  "alternates": [
    {
      "provider": "openai",
      "credential_name": "prod-openai-us-2",
      "model": "gpt-5",
      "score": { "weight": 100.0, "latency_ms": 312.1, "inflight": 8, "health_penalty": 0.0 }
    }
  ],
  "rejections": [
    {
      "candidate": "gemini/eu-1",
      "reason": "region_mismatch"
    }
  ],
  "model_resolution": [
    { "step": "fallback_chain_built", "primary": "gpt-5", "fallbacks": ["gpt-5-mini", "claude-sonnet-4-5"] }
  ]
}
```

### Explain Response

Same as preview response with additional scoring details:

```json
{
  "...preview fields...",
  "scoring": [
    {
      "candidate": "openai/prod-openai-us-1",
      "score": { "weight": 100.0, "latency_ms": 245.3 },
      "rank": 1
    },
    {
      "candidate": "openai/prod-openai-us-2",
      "score": { "weight": 100.0, "latency_ms": 312.1 },
      "rank": 2
    }
  ]
}
```

## Backend Implementation

### Module Structure

```
crates/server/src/handler/dashboard/
├── routing.rs          # Profile-based routing handlers

web/src/
├── pages/
│   └── Routing.tsx     # Profile-based routing page
├── components/routing/
│   ├── PresetCards.tsx
│   ├── RuleTable.tsx
│   ├── AdvancedEditor.tsx
│   └── RoutePreview.tsx
├── services/
│   └── api.ts          # Add preview/explain API calls
```

### Backend Handlers (`routing.rs`)

```rust
/// GET /api/dashboard/routing
pub async fn get_routing(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    Json(&config.routing) // new RoutingConfig from SPEC-048
}

/// PATCH /api/dashboard/routing
pub async fn update_routing(
    State(state): State<AppState>,
    Json(update): Json<RoutingConfigUpdate>,
) -> impl IntoResponse {
    // Validate config
    // Apply to config
    // Trigger hot reload
}

/// POST /api/dashboard/routing/preview
pub async fn preview_route(
    State(state): State<AppState>,
    Json(req): Json<PreviewRequest>,
) -> impl IntoResponse {
    let features = RouteRequestFeatures::from(req);
    let config = state.config.load();
    let inventory = state.catalog.snapshot();
    let health = state.health.snapshot();

    let plan = RoutePlanner::plan(&features, &config.routing, &inventory, &health);
    let explanation = explain(&plan);

    Json(explanation)
}

/// POST /api/dashboard/routing/explain
pub async fn explain_route(
    State(state): State<AppState>,
    Json(req): Json<PreviewRequest>,
) -> impl IntoResponse {
    // Same as preview but includes scoring details
    let features = RouteRequestFeatures::from(req);
    let config = state.config.load();
    let inventory = state.catalog.snapshot();
    let health = state.health.snapshot();

    let plan = RoutePlanner::plan(&features, &config.routing, &inventory, &health);
    let explanation = explain_detailed(&plan);

    Json(explanation)
}
```

### Frontend Components

#### Page Layout (`Routing.tsx`)

```
+------------------------------------------+
| Section 1: Routing Mode                  |
| +--------+ +--------+ +--------+ +----+ |
| |Balanced| | Stable | |Low Lat | |Cost| |
| |  (*)   | |        | |        | |    | |
| +--------+ +--------+ +--------+ +----+ |
+------------------------------------------+
| Section 2: Rules                         |
| +------+--------+----------+--------+   |
| | Name | Match  | Profile  | Action |   |
| +------+--------+----------+--------+   |
| | ent  | gpt-*  | low-lat  | [edit] |   |
| | bg   | header | low-cost | [edit] |   |
| +------+--------+----------+--------+   |
| [+ Add Rule]                             |
+------------------------------------------+
| Section 3: Advanced Policy               |
| [Provider] [Credential] [Health] [Fail]  |
| +--------------------------------------+ |
| | Strategy: weighted-round-robin       | |
| | Weights: openai=100 claude=100       | |
| +--------------------------------------+ |
+------------------------------------------+
| Section 4: Route Preview                 |
| +------------------+-------------------+ |
| | Model: [gpt-5  ] | Selected:         | |
| | Endpoint: [chat ] |   openai/prod-1   | |
| | Tenant: [acme   ] | Alternates:       | |
| | Region: [us-east] |   openai/prod-2   | |
| | Stream: [ ] No   |   claude/prod-1   | |
| | [Preview]         | Rejected:         | |
| |                   |   gemini/eu-1:    | |
| |                   |   region_mismatch | |
| +------------------+-------------------+ |
+------------------------------------------+
```

#### PresetCards.tsx

- 4 cards with icon, title, description
- Descriptions use human intent language:
  - Balanced: "Distribute requests evenly across providers and credentials"
  - Stable: "Always use the same provider, failover only when unhealthy"
  - Lowest Latency: "Route to the fastest responding provider"
  - Lowest Cost: "Route to the cheapest available provider"
- Selecting a preset updates the active profile

#### RuleTable.tsx

- CRUD table for route rules
- Match columns: model (glob), tenant (glob), endpoint, stream, region, headers
- Profile column: dropdown of available profiles
- Priority: drag-to-reorder or explicit number
- Inline validation

#### AdvancedEditor.tsx

- 4 tabs: Provider Policy, Credential Policy, Health, Failover
- Each tab shows the effective config for the selected profile
- Strategy dropdown with description
- Parameter editors (weights, thresholds, timeouts)

#### RoutePreview.tsx

- Left: form with model, endpoint, tenant, region, stream, headers
- Right: preview result (calls `POST /api/dashboard/routing/preview`)
- Shows: selected route, alternates, rejections with reasons, model chain
- Auto-refreshes on form change (debounced)

### UX Rules

1. Show human intent label (e.g., "Distribute evenly") before algorithm name (e.g., "weighted-round-robin")
2. Display effective policy (merged defaults + overrides), not raw config fragments
3. Validate impossible combinations before save:
   - `ordered-fallback` requires non-empty `order` list
   - `weighted-round-robin` requires non-empty `weights`
   - Profile referenced by rule must exist
4. Preview uses same planner code path as production

### Changes to Existing Code

1. **Rewrite** `handler/dashboard/routing.rs` with profile-based handlers
2. **Rewrite** `web/src/pages/Routing.tsx` with presets + rules + preview layout
3. **Remove** strategy dropdown UI
4. **Update** `web/src/services/api.ts` with preview/explain API calls
5. **Update** Axum router to register new endpoints

## Configuration Changes

No new config fields beyond SPEC-048.

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | Preview is provider-agnostic |
| Claude   | Yes       | Preview is provider-agnostic |
| Gemini   | Yes       | Preview is provider-agnostic |

## Task Breakdown

- [ ] Implement `preview_route` handler
- [ ] Implement `explain_route` handler
- [ ] Rewrite `get_routing` handler for profile-based config shape
- [ ] Rewrite `update_routing` handler with validation
- [ ] Register new routes in Axum router
- [ ] Create `PresetCards.tsx` component
- [ ] Create `RuleTable.tsx` component with CRUD
- [ ] Create `AdvancedEditor.tsx` component with 4 tabs
- [ ] Create `RoutePreview.tsx` component with live preview
- [ ] Rewrite `Routing.tsx` page with presets + rules + preview layout
- [ ] Update `api.ts` with preview/explain API calls
- [ ] Remove legacy routing strategy dropdown code
- [ ] Integration tests: preview API — returns correct selected route, alternates, rejections
- [ ] Integration tests: preview API — empty inventory returns empty selected with rejections
- [ ] Integration tests: preview API — invalid request body returns 400 with error detail
- [ ] Integration tests: preview API — missing required fields return 400
- [ ] Integration tests: explain API — includes scoring entries for all candidates
- [ ] Integration tests: explain API — scoring rank order matches selected + alternates order
- [ ] Integration tests: GET routing — returns current config in profile-based shape
- [ ] Integration tests: PATCH routing — valid update applies and persists after reload
- [ ] Integration tests: PATCH routing — rule references non-existent profile returns 422
- [ ] Integration tests: PATCH routing — ordered-fallback without order list returns 422
- [ ] Integration tests: PATCH routing — weighted-rr without weights returns 422
- [ ] Integration tests: PATCH routing — empty profiles map returns 422
- [ ] Integration tests: config hot-reload — updated routing config reflected in subsequent preview calls
- [ ] Frontend: `npx tsc --noEmit` passes (type-check)
- [ ] Frontend: API type definitions match backend response shapes
- [ ] E2E test: full routing stack — config update via PATCH, then preview reflects the change

## Test Strategy

- **Unit tests:** None (handler + UI layer)
- **Integration tests:**
  - **Preview API:** Mock inventory with 3 providers, 5 credentials (some unhealthy, some region-mismatched). Verify selected, alternates, and rejections match expected. Edge case: empty inventory returns no selected with empty alternates.
  - **Explain API:** Same setup as preview, verify scoring entries present for all non-rejected candidates, rank order consistent with selected + alternates ordering.
  - **GET routing:** Returns complete `RoutingConfig` with all profiles, rules, model-resolution.
  - **PATCH routing:** Valid update persists. Invalid updates (missing profile ref, missing strategy params, empty profiles) return 422 with specific error message.
  - **Config hot-reload:** PATCH config, then call preview, verify new config is active.
  - **Error responses:** Invalid JSON body returns 400, missing required fields returns 400 with field-level errors.
- **Frontend:**
  - TypeScript type-check: `npx tsc --noEmit` passes.
  - API type consistency: frontend type definitions for preview/explain response match backend serde output (verified by shared type generation or manual comparison).
- **E2E test:**
  - Full stack: update config via PATCH -> preview via POST -> verify preview uses new config. This validates the config -> planner -> preview pipeline end-to-end.
- **Manual verification:** Dashboard routing page layout, preset card interaction, rule CRUD, advanced editor tabs, route preview live updates.

## Rollout Plan

1. Backend handlers first
2. Frontend components
3. Clean up legacy routing code
4. End-to-end manual verification
