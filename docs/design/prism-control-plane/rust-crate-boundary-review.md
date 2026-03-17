# Rust Crate Boundary Review

This note reviews whether Prism should split more Rust crates now that the control-plane code has been reworked.

## Current Judgment

Do not split new crates yet.

Current boundaries are still broadly correct:

- `prism-types` owns transport-adjacent shared types with optional `reqwest` and `axum` bindings.
- `prism-domain` owns core domain-level data structures that should stay independent from delivery and runtime concerns.
- `prism-lifecycle` owns daemon, signal, pid, and runtime process lifecycle behavior.
- `prism-protocol` owns protocol shape and request/response normalization helpers.
- `prism-core` owns config, auth/runtime-adjacent primitives, metrics, cache, and routing config composition.
- `prism-provider` owns upstream execution and credential runtime behavior.
- `prism-translator` owns wire-format translation.
- `prism-server` owns Axum handlers, middleware, request dispatch, and dashboard/control-plane composition.

The recent control-plane work showed that the main problem was not top-level crate shape.
The main problem was oversized modules and mixed responsibilities inside `prism-server` and `web/`.

Module splitting was the right move first:

- `dashboard/control_plane/*`
- `dashboard/providers/{mod,mutation,probe}.rs`
- `dashboard/auth_profiles/{mod,managed}.rs`

This already gives a reasonably layered backend without introducing more crate-level coupling, duplicated test harnesses, or fragile public interfaces.

## Why A New Crate Is Not Yet Worth It

### Dashboard handlers are still delivery-layer code

The dashboard/control-plane handlers are tightly coupled to:

- Axum request extraction and response shaping
- `AppState`
- dashboard auth middleware expectations
- config transaction helpers
- runtime stores and router/catalog refresh

That is still delivery code, not a reusable standalone domain package.

Moving it to a new crate now would likely create one of two bad outcomes:

1. a fake "dashboard domain" crate that still depends on most of `prism-server`
2. a large new API surface exported only to preserve an artificial crate boundary

Neither improves extensibility.

### The existing split is already at the point of diminishing returns

The workspace has already extracted the low-friction seams:

- transport types
- domain objects
- lifecycle/process control
- protocol normalization
- runtime/provider execution
- server delivery

Another immediate split would likely produce crates whose public API exists only to satisfy the split itself.
That usually increases churn instead of reducing it.

### The real extensibility problem is workflow composition

The parts still worth improving are:

- provider mutation builders and apply helpers
- provider probe runners and result mappers
- managed-auth workflow orchestration
- control-plane read-model composers

Those should first become smaller modules inside `prism-server`.

## What To Split Next Inside Existing Crates

### In `prism-server`

Keep `prism-server` as the control-plane delivery crate, but continue splitting modules around stable seams:

- `dashboard/providers/summary.rs`
- `dashboard/providers/validation.rs`
- `dashboard/providers/runtime_seed.rs`
- `dashboard/auth_profiles/runtime.rs`
- `dashboard/control_plane/workspaces/*`

Also worth considering later inside `prism-server`:

- `dashboard/providers/probe_runner.rs`
- `dashboard/providers/probe_result.rs`
- `dashboard/providers/apply.rs`
- `dashboard/auth_profiles/device_flow.rs`
- `dashboard/auth_profiles/oauth.rs`

### In `web/`

Keep the current app as one frontend package, but continue splitting by domain workflow:

- `useProviderAtlasSelection`
- `useProviderAtlasAuthWorkflow`
- `useProviderAtlasRegistryWorkflow`
- `useChangeStudioAccessWorkflow`
- `useChangeStudioPublishWorkflow`

That gives reuse and testability faster than package-level splits.

## When A New Crate Would Become Justified

Create a new Rust crate only if one of these becomes true:

1. A module becomes independently reusable outside `prism-server`.
2. The code needs its own dependency graph that should not leak into server delivery code.
3. The code can be tested meaningfully without `AppState`, Axum handlers, or server routing.
4. The public interface is stable enough that extracting it reduces churn instead of increasing it.

## Future Crates That Could Become Legitimate

These are plausible later, but not yet justified now:

- `prism-control-plane-domain`
  - only if signals, changes, investigations, and typed sources become real domain objects used beyond HTTP handlers
- `prism-control-plane-query`
  - only if workspace read models become large, reusable projections independent of Axum
- `prism-runtime-auth`
  - only if managed auth runtime grows beyond current provider/dashboard coupling

## Decision

For the current codebase:

- keep the existing top-level crate split
- keep improving internal module boundaries
- avoid introducing a new crate until a stable reusable domain emerges

That is the higher-leverage path for extensibility and reuse right now.
