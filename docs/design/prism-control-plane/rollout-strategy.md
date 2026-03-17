# Rollout Strategy for Prism Control Plane

This note answers a specific implementation question:

How should Prism ship the control plane safely without carrying transitional UI constraints?

The answer is:

- rebuild the control-plane UI as the canonical shell
- keep runtime-truth backend primitives where they are already correct
- keep the current production entry stable until the control plane is fully ready
- switch production to the new control plane in one cutover

## Core Principle

Prism does not need a compatibility-driven redesign.

It does need a risk-controlled rollout.

That means the team should separate:

1. the target architecture
2. the release sequence

Target architecture can stay clean and direct.
Implementation can still be staged internally even if production cutover happens once.

## What Clean-Slate Means Here

Clean-slate does not mean "rewrite everything blindly".

It means:

- the new shell is not constrained by the old page list
- the new interaction model is not constrained by current modals, drawers, and tabs
- the new workspace boundaries are not constrained by existing React routes
- the product model is driven by `request session`, `signal`, `investigation`, `change`, `provider identity`, `route draft`, and `data source`

It does not mean:

- throw away correct backend primitives
- freeze rollout until every workspace is perfect
- force one risky big-bang cutover

## Recommended Launch Model

Use a parallel rebuild with one production cutover.

### Phase 0: Parallel shell

- build the new shell behind an internal route or feature flag
- keep the current production entry unchanged
- do not attempt to preserve old navigation as a product constraint

### Phase 1: Complete the full workspace pack

- finish `Traffic Lab`
- finish `Provider Atlas`
- finish `Route Studio`
- finish `Change Studio`
- finish `Command Center`
- finish the shared layers: foundations, shell kit, platform patterns, and entity editors

### Phase 2: End-to-end readiness verification

- validate the full operator loop in the new shell
- validate all required pages and editors
- validate runtime truth, publish flows, auth flows, and fallback paths
- validate visual and interaction quality before cutover

## Launch Readiness Gate

The new control plane should not become the production UI until all required surfaces are ready.

Minimum launch bar:

- `Command Center` is complete
- `Traffic Lab` is complete
- `Provider Atlas` is complete
- `Route Studio` is complete
- `Change Studio` is complete
- shared layers are complete: foundations, shell kit, platform patterns, entity editors
- required auth and config workflows are complete
- required debug and publish workflows are complete
- browser verification and design QA are complete
- production fallback plan is defined before the switch

### Phase 3: One production switch

- change the production entry point to the new control plane
- keep a rollback path available for the previous production entry if needed

## Rules During Rollout

### 1. Do not preserve legacy navigation as product truth

Legacy routes may exist temporarily.
They should not define the new IA.

### 2. Preserve backend truth, not frontend shape

If a current handler is already truthful, reuse it.
If a current page boundary is arbitrary, drop it.

### 3. Prefer new aggregate read models over page-compatible glue

Do not build a new shell that simply renders old page data one page at a time.
Add workspace-level composition where needed.

### 4. Use the existing production entry only as a pre-cutover runtime guard

The existing production entry can remain live while the control plane is still incomplete.
It should not define the design or the final IA.

### 5. Cut over only after the pack is complete

Do not partially expose the new shell to operators as the primary UI.
Finish the full required surface first, then switch.

## What Should Be Feature-Flagged

These are the best candidates for internal gating before cutover:

- new shell route
- command palette
- new inspector routing
- workspace-specific drill-down
- staged publish flow
- investigation objects
- external source registry

## Suggested Release Metrics

Use rollout metrics that reflect control-plane quality, not just page views.

- percentage of required workflows completed in the new shell during internal validation
- percentage of config edits completed in `Change Studio`
- percentage of request debugging completed in `Traffic Lab`
- operator time-to-explain for failed requests
- publish failure and rollback rate
- source freshness and watch-window completion rate

## Practical Recommendation

Prism should make one clean decision:

- architecture: clean-slate
- production launch: single cutover
- interim production entry: current dashboard until readiness

That is the best way to avoid legacy UX baggage while still keeping release risk under control.
