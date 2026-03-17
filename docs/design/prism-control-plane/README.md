# Prism Control Plane Prototype

This directory contains a prototype-first redesign for Prism's dashboard. It is intentionally separate from the production React app.

## Included

- `prototype.html` - standalone high-fidelity prototype
- `prototype.css` - tokens and visual system
- `prototype.js` - lightweight interactions for screen switching and inspector updates
- `figma-handoff.md` - how to recreate and manage the design system in Figma
- `pencil-dev/README.md` - recommended Pencil Dev + Codex MCP workflow and file organization
- `pencil-dev/CONVENTIONS.md` - stable file placement and naming rules for repeated Pencil revisions
- `pencil-dev/ARCHITECTURE.md` - north-star layering for reusable, extensible Pencil workspace files
- `debug-config-deep-dive.md` - current Prism capability mapping + industry best-practice notes
- `debug-config-reference.md` - source-map view of current debug/config power and how the control plane should absorb it
- `gateway-benchmark-analysis.md` - current code baseline + OpenClaw / CLIProxy / gateway best-practice guidance before reuse
- `cutover-scope-review.md` - which capabilities must exist for cutover and which legacy surfaces should explicitly not return
- `remaining-additions-roadmap.md` - complete list of remaining additions across design, product flows, backend, integrations, and verification
- `frontend-implementation-plan.md` - how the current frontend should migrate into the new shell and workspace modules
- `rust-crate-boundary-review.md` - whether the Rust backend should split more crates or continue improving module seams first
- `backend-control-plane-model.md` - the backend object model and aggregate APIs that best fit the approved design
- `rollout-strategy.md` - how to ship the new control plane progressively without carrying legacy UI compatibility
- `extensibility-model.md` - how the shell should handle SLS, external analytics, and future integrations
- `north-star-model.md` - product model without backend or legacy page constraints
- `config-crud-model.md` - how rich configuration management should work beyond a YAML editor

## What Changed

The redesign replaces the current page-list mental model with a control-plane shell and five workspaces:

- `Command Center` - runtime posture, urgent signals, and global status
- `Traffic Lab` - request streams, filters, traces, and saved lenses
- `Provider Atlas` - providers, auth posture, capabilities, and coverage
- `Route Studio` - route reasoning, fallback review, and policy simulation
- `Change Studio` - config diffs, staged rollout, and post-change observation

The shared design layer is now also explicit in Pencil, not only implicit inside the workspaces:

- `Prism Foundations` - operating objects, shell contracts, evidence modes, workflow patterns, and state coverage
- `Platform Patterns` - global command surface, ownership and audit, integration registry, and missing editor families
- `Entity Editors` - detailed patterns for auth keys, tenant policies, data sources, and alert policies

Under the surface, the redesign is also moving away from page-thinking toward object-thinking:

- requests become request sessions
- anomalies become signals or investigations
- config edits become staged changes with evidence and watch windows

Implementation stance:

- UI and interaction model can be rebuilt from a clean slate
- production release should be a full cutover after the new control plane is complete
- current backend capabilities are inputs, not upper bounds

## Interaction Principles

- One global context bar: environment, time range, live state, and operator filters
- One inspector rail: entity detail lives here instead of being reimplemented as modals
- One embedded workbench pattern for multi-step edits and approvals
- One command palette for navigation and frequent actions
- One language toggle in the shell to validate i18n layout behavior early

## Prototype Additions

This revision expands the prototype in three areas the production redesign needs to handle well:

- `Traffic Lab` now includes a dedicated debug workbench for replay, upstream transform inspection, fallback reasoning, and trace comparison
- `Route Studio` now includes a higher-fidelity route draft workbench for profile editing, matcher review, simulation, blast-radius review, and publish linkage
- `Change Studio` now includes a publish workbench for preflight checks, staged rollout, observation, and rollback criteria
- `Change Studio` now also includes a richer config operator workbench: registry browsing, object detail, dependency impact, version trail, and guarded destructive actions
- `Change Studio` now includes family-specific editor patterns for providers, auth profiles, and route profiles so the prototype covers more than generic CRUD
- the shell now supports `EN` and `中文` switching for core navigation and workspace copy, so layout pressure from internationalization is visible before implementation
- the shell now also exposes extensibility direction: native runtime truth, hybrid evidence, and external analytics such as SLS can coexist without changing the workspace model

## Why Debug / Config Come First

For Prism, the redesign should be judged first on debugging and configuration quality.

- debugging is the shortest path from a broken request to a concrete cause
- configuration is the shortest path from a concrete cause to a safe fix
- the operator loop is therefore `inspect -> explain -> change -> observe`, not `navigate -> open page -> open modal`

Use the supporting notes below while reviewing the prototype:

- [debug-config-deep-dive.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/debug-config-deep-dive.md)
- [debug-config-reference.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/debug-config-reference.md)
- [gateway-benchmark-analysis.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/gateway-benchmark-analysis.md)
- [cutover-scope-review.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/cutover-scope-review.md)
- [remaining-additions-roadmap.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/remaining-additions-roadmap.md)
- [frontend-implementation-plan.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/frontend-implementation-plan.md)
- [rust-crate-boundary-review.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/rust-crate-boundary-review.md)
- [backend-control-plane-model.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/backend-control-plane-model.md)
- [extensibility-model.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/extensibility-model.md)
- [north-star-model.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/north-star-model.md)
- [config-crud-model.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/config-crud-model.md)
- [pencil-dev/README.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/pencil-dev/README.md)
- [pencil-dev/CONVENTIONS.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/pencil-dev/CONVENTIONS.md)
- [pencil-dev/ARCHITECTURE.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/pencil-dev/ARCHITECTURE.md)

## Review Flow

Open [prototype.html](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/prototype.html) in a browser, or serve this folder locally and inspect the prototype there.

Useful deep links while reviewing:

- [prototype.html?screen=traffic-lab](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/prototype.html?screen=traffic-lab)
- [prototype.html?screen=change-studio](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/prototype.html?screen=change-studio)

Related specs:

- [SPEC-071 PRD](/Users/qiufeng/work/proxy/prism/docs/specs/active/SPEC-071/prd.md)
- [SPEC-071 Technical Design](/Users/qiufeng/work/proxy/prism/docs/specs/active/SPEC-071/technical-design.md)
- [SPEC-071 Research](/Users/qiufeng/work/proxy/prism/docs/specs/active/SPEC-071/research.md)
