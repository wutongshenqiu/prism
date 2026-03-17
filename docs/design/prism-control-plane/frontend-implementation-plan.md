# Frontend Implementation Plan for Prism Control Plane

This note turns the approved design package into a frontend implementation plan.

It is intentionally more concrete than the visual design notes, but it still does not force the production implementation to preserve the current page inventory.

Read this together with:

- [README.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/README.md)
- [gateway-benchmark-analysis.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/gateway-benchmark-analysis.md)
- [remaining-additions-roadmap.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/remaining-additions-roadmap.md)
- [../specs/active/SPEC-071/technical-design.md](/Users/qiufeng/work/proxy/prism/docs/specs/active/SPEC-071/technical-design.md)

## Implementation Goal

The production dashboard should migrate from page-local CRUD and page-local filters to:

1. one shared shell
2. one shared global context model
3. one shared inspector model
4. one shared workbench pattern
5. workspace-specific data composition on top of current runtime primitives

The target is not "replace every page at once".
The target is "stabilize one control-plane shell, then migrate capability slices into it".

Current implementation path:

- treat `web/` as the canonical control-plane frontend
- continue filling remaining workspaces and workflows in place

## Clean-Slate Stance

The frontend should be treated as a clean control-plane implementation.

That means:

- do not preserve the current page tree for compatibility
- do not preserve current modal, drawer, and tab conventions if they conflict with the new shell
- do not treat old page boundaries as architectural boundaries
- do preserve truthful capability logic and API semantics when they are already correct

In short:

- product model: clean-slate
- shell and interaction model: clean-slate
- primitive capability reuse: selective
- rollout: full cutover after complete readiness

## Current Code Baseline

The existing frontend already contains most of the capability primitives that the redesign needs.
The problem is mostly composition, not missing surface area.

### Traffic and debug primitives

- `RequestLogs.tsx` already has URL-backed filters, live mode, and drawer routing.
- `LogDrawer.tsx` already provides request-detail drill-down.
- `Replay.tsx` already provides route explanation and simulation.

Implication:

- `Traffic Lab` should absorb these three surfaces into one workspace instead of keeping them as separate pages.

### Config and publish primitives

- `Config.tsx` already supports runtime snapshot, raw YAML, validate, apply, and config-version conflict handling.

Implication:

- `Change Studio` should not throw away this truth model.
- It should wrap it in registry, draft, review, publish, and watch-window workflows.

### Provider and auth primitives

- `Providers.tsx` already supports provider CRUD, model fetch, health check, and presentation preview.
- `AuthProfiles.tsx` already supports managed auth flows such as connect, import, browser OAuth, device flow, and refresh.
- `ModelsCapabilities.tsx` and `Protocols.tsx` already expose capability and protocol truth.

Implication:

- `Provider Atlas` should become the runtime entity graph built from these capabilities.
- It should not remain a disconnected set of inventory pages.

### Access and tenant primitives

- `AuthKeys.tsx` already supports key CRUD, reveal, model restrictions, rate limits, and budgets.
- `Tenants.tsx` already supports tenant summaries and per-tenant metrics.

Implication:

- `Change Studio` and the new `Entity Editors` patterns should absorb these as structured objects, not as isolated tabs.

## Recommended Frontend Module Shape

The current `web/` layout should evolve toward workspace-first modules.

```text
web/src/
├── shell/
│   ├── AppShell.tsx
│   ├── GlobalContextBar.tsx
│   ├── WorkspaceNavigation.tsx
│   ├── WorkspaceHeader.tsx
│   ├── InspectorRail.tsx
│   ├── WorkbenchHost.tsx
│   └── CommandPalette.tsx
├── workspaces/
│   ├── command-center/
│   ├── traffic-lab/
│   ├── provider-atlas/
│   ├── route-studio/
│   └── change-studio/
├── entities/
│   ├── providers/
│   ├── auth-profiles/
│   ├── auth-keys/
│   ├── tenants/
│   ├── routes/
│   ├── changes/
│   ├── signals/
│   └── sources/
├── features/
│   ├── investigations/
│   ├── saved-lenses/
│   ├── compare/
│   ├── replay/
│   └── publish-watch/
├── stores/
│   ├── shellStore.ts
│   ├── contextStore.ts
│   ├── inspectorStore.ts
│   ├── commandPaletteStore.ts
│   ├── workflowStore.ts
│   └── dataSourceStore.ts
├── queries/
│   ├── traffic.ts
│   ├── providers.ts
│   ├── routing.ts
│   ├── changes.ts
│   ├── signals.ts
│   └── sources.ts
└── ui/
    ├── cards/
    ├── tables/
    ├── badges/
    ├── inspector/
    └── workbench/
```

## What to Preserve From Current Frontend

Do not rewrite stable capability logic just because the layout changes.

Preserve and re-home:

- log query and URL filter behavior from `web/src/pages/RequestLogs.tsx`
- request-detail drill-down from `web/src/components/LogDrawer.tsx`
- route explain and preview behavior from `web/src/pages/Replay.tsx`
- config validate/apply/version behavior from `web/src/pages/Config.tsx`
- provider model fetch, health check, and presentation preview from `web/src/pages/Providers.tsx`
- managed auth flows from `web/src/pages/AuthProfiles.tsx`
- auth-key budgeting and rate-limit forms from `web/src/pages/AuthKeys.tsx`
- tenant summary and detail behavior from `web/src/pages/Tenants.tsx`

## What Not to Preserve

The redesign should explicitly avoid carrying these patterns forward:

- page-local filter state as the main source of truth
- page-local drawers and modals with custom state machines
- mixed tab/page/modal patterns for the same entity family
- separate overview pages that duplicate the same provider or route facts
- raw YAML as the default entry point for configuration editing
- hard split between monitoring pages and configuration pages

## Workspace Mapping

| Current Surface | New Workspace | Keep | Change |
|-----------------|---------------|------|--------|
| `Dashboard.tsx`, `System.tsx`, `Logs.tsx` | `Command Center` | runtime summary, log stream, system posture | replace metric-wall layout with signal queue and watch stack |
| `RequestLogs.tsx`, `LogDrawer.tsx`, `Replay.tsx` | `Traffic Lab` | log query, request detail, explain preview | compose into request-session debugger and investigation flow |
| `Providers.tsx`, `AuthProfiles.tsx`, `ModelsCapabilities.tsx`, `Protocols.tsx` | `Provider Atlas` | provider CRUD, auth runtime, capabilities, protocol truth | recast as runtime entity graph with coverage and rotation posture |
| `Routing.tsx`, `Replay.tsx` | `Route Studio` | routing config, preview, explain | recast as draft object, scenario matrix, blast radius, publish linkage |
| `Config.tsx`, `AuthKeys.tsx`, `Tenants.tsx`, parts of `Providers.tsx` and `AuthProfiles.tsx` | `Change Studio` | validate/apply/version, key CRUD, tenant metrics, family-specific forms | recast as config registry, staged publish, history, watch, rollback |

## Shared UI Primitives to Build First

Before migrating any workspace, frontend work should extract these primitives from the design pack:

1. `AppShell`
2. `GlobalContextBar`
3. `WorkspaceHeader`
4. `InspectorRail`
5. `WorkbenchHost`
6. `SignalCard`
7. `SessionRow`
8. `DecisionTimeline`
9. `EntityFactList`
10. `StatusBadge` and `SourceBadge`
11. `StateSurface` variants for loading, empty, error, stale, disconnected, compare

These should map to the visual rules already stabilized in:

- `Prism Foundations`
- `Prism Shell Kit`
- `Platform Patterns`
- `Entity Editors`

## State Model Guidance

The current implementation leans on page-local `useState` plus a few Zustand stores.
That is fine as a baseline, but the new shell should centralize only the state that is truly shared.

### Shared state

- global context: environment, time range, provider, tenant, model, live mode, source mode
- inspector selection
- command palette open state
- active workbench workflow
- active investigation
- saved lens identity

### Workspace-local state

- table sort
- compare mode inputs
- draft form values
- per-workspace tabs

Rule:

- if the state must survive navigation, copy URL, or drive the inspector, it should live in shared state and URL search params
- if the state only affects one local panel, keep it inside the workspace

## Delivery Sequence

The recommended order is not the current page order, and it is not a page-by-page migration.

### Slice 0: Shell and route scaffolding

- build the new shell
- add workspace routing for the new control plane
- move existing auth and session protection into the shell
- keep old pages available only as a temporary fallback, not as a design constraint

### Slice 1: Traffic Lab

- highest leverage workspace
- reuses `RequestLogs`, `LogDrawer`, and `Replay` primitives where useful
- validates global context, inspector, workbench, compare, and live update models

### Slice 2: Provider Atlas

- reuses provider inventory, auth runtime, capabilities, and protocol truth
- validates entity-graph and inspector-heavy patterns

### Slice 3: Route Studio

- reuses routing config and explain primitives
- validates draft object and publish linkage patterns

### Slice 4: Change Studio

- reuses config validate/apply and entity CRUD primitives
- validates registry, draft, staged publish, and watch-window patterns

### Slice 5: Command Center

- should be built after the other workspaces exist
- it becomes a compositional summary of already-stable signal, watch, and source patterns

## Launch Strategy

The product should not be released workspace by workspace.

Recommended approach:

1. build the new control-plane shell and all target workspaces behind an internal route or feature flag
2. keep the current production dashboard as the only operator-facing UI until the new control plane is complete
3. use the old version as the production fallback during design validation and implementation
4. switch the production entry point only after all required pages and workflows are ready

This is a clean-slate rebuild with a single production cutover, not a compatibility-driven migration and not a public progressive rollout.

## Testing and Review Gates

Before each migrated slice is considered stable:

1. URL state must round-trip for the core workflow.
2. The same entity must render correctly in both the workbench and inspector.
3. Dense dark surfaces must match the contrast and no-overflow rules in `QUALITY-GATES.md`.
4. The workspace must pass browser screenshot review at desktop widths.
5. The slice must not reintroduce generic blocking modals for long workflows.

## Practical Recommendation

Do not start by rebuilding every current page under the new navigation.

Start by building a thin but real shell, then move:

1. `Traffic Lab`
2. `Provider Atlas`
3. `Route Studio`
4. `Change Studio`
5. `Command Center`

That order gives Prism the fastest path to a real control plane instead of a reskinned dashboard.
