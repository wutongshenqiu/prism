# Prism Control Plane with Pencil Dev

This note defines the recommended Pencil Dev workflow for the Prism control-plane redesign.

Related structural notes:

- [../gateway-benchmark-analysis.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/gateway-benchmark-analysis.md)
- [../cutover-scope-review.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/cutover-scope-review.md)
- [../remaining-additions-roadmap.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/remaining-additions-roadmap.md)
- [../../../playbooks/repo-layout-and-artifacts.md](/Users/qiufeng/work/proxy/prism/docs/playbooks/repo-layout-and-artifacts.md)
- [CONVENTIONS.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/pencil-dev/CONVENTIONS.md)
- [ARCHITECTURE.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/pencil-dev/ARCHITECTURE.md)
- [QUALITY-GATES.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/pencil-dev/QUALITY-GATES.md)
- [workspaces/README.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/pencil-dev/workspaces/README.md)

## Why Pencil Dev

Pencil Dev fits this project better than a detached mockup tool because:

- `.pen` files are JSON-based and Git-friendly
- Pencil exposes a local MCP server when running
- Codex CLI can connect to Pencil through MCP
- variables and components map well to Prism's dashboard shell and design tokens

Official references used for this workflow:

- `AI Integration`
- `Installation`
- `.pen Files`
- `Design as Code`
- `Design ↔ Code`
- `Variables`
- `Components`
- `Pencil CLI`

## Current Local Status

As of `2026-03-17`, the local Pencil workflow is functional:

- `Pencil.app` is installed
- Pencil MCP is enabled in Codex
- Claude authentication is complete
- `Prism Foundations`, `Platform Patterns`, `Entity Editors`, `Command Center`, `Traffic Lab`, `Change Studio`, `Route Studio`, and `Provider Atlas` boards have been built in the active Pencil document
- `Prism Foundations` now also covers operating objects, shell contracts, evidence modes, workflow patterns, and state coverage

What is still missing is a repo-local saved `.pen` file.

At the moment, that final save step still requires a manual desktop action inside Pencil.
In this workflow, MCP is sufficient for editing, inspecting, and exporting active boards, but it does not currently provide a reliable "save current document as repo-local `.pen` file" action.

Until that save step is done, the durable review artifacts are the exported PNG files under:

- [output/pencil/prism-control-plane](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane)

Key review artifacts now include:

- [foundations-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/foundations-overview--latest.png)
- [platform-patterns-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/platform-patterns-overview--latest.png)
- [entity-editors-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/entity-editors-overview--latest.png)
- [command-center-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/command-center-overview--latest.png)
- [traffic-lab-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/traffic-lab-overview--latest.png)
- [provider-atlas-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/provider-atlas-overview--latest.png)
- [route-studio-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/route-studio-overview--latest.png)
- [change-studio-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/change-studio-overview--latest.png)

## Recommended Setup

For Prism, the best starting path is `Pencil Desktop App + Codex CLI`.

This is a pragmatic recommendation, not a hard requirement:

- official docs say Codex works by starting Pencil first and then running `/mcp`
- official `pencil` terminal support is still marked experimental and desktop-only
- desktop app gives the most predictable MCP surface for early design work

## Setup Steps

1. Install Pencil Dev.
   On macOS, use the official `.dmg` from `pencil.dev` and move it to `Applications`.

2. Complete Pencil activation.
   The installation guide says Pencil needs activation by email before normal use.

3. Back up Codex config before first MCP use.
   Official AI integration docs note that Pencil may modify or duplicate `~/.codex/config.toml`.

4. Launch Pencil.

5. Create or save the first design file into:

   `docs/design/prism-control-plane/pencil-dev/workspaces/prism-control-plane-traffic-lab.pen`

6. Open that `.pen` file in Pencil.

7. In Codex CLI, run:

   ```text
   /mcp
   ```

8. Verify `Pencil` appears in the MCP server list.

9. Start iterative design work from Codex against the open `.pen` file.

## Claude Code Requirement

The current official installation page still says Pencil AI features require Claude Code login.

At the same time, the official AI integration page explicitly lists `Codex CLI` as supported and documents the `/mcp` workflow.

My recommendation is:

- first verify the native `Pencil -> MCP -> Codex` path
- if activation or AI actions are blocked, add Claude Code authentication as a fallback

This is an inference from the current docs, because the documentation is still slightly Claude-centric while already documenting Codex support.

## Repository Layout

Recommended file layout for Prism:

```text
docs/design/prism-control-plane/
├── prototype.html
├── prototype.css
├── prototype.js
└── pencil-dev/
    ├── README.md
    ├── CONVENTIONS.md
    ├── ARCHITECTURE.md
    ├── prompts/
        ├── shell-kit.md
        ├── command-center.md
        ├── foundations.md
        ├── provider-atlas.md
        ├── route-studio.md
        ├── traffic-lab.md
        └── change-studio.md
    └── workspaces/
        ├── README.md
        ├── prism-control-plane-foundations.pen
        ├── prism-control-plane-traffic-lab.pen
        ├── prism-control-plane-change-studio.pen
        ├── prism-control-plane-route-studio.pen
        ├── prism-control-plane-provider-atlas.pen
        ├── prism-control-plane-command-center.pen
        └── prism-control-plane-explorations.pen
```

Recommended responsibilities:

- `prism-control-plane-foundations.pen`: tokens, typography, shell primitives, reusable cards, pills, tables, inspector blocks
- `prism-control-plane-traffic-lab.pen`: the canonical debugging workspace
- `prism-control-plane-change-studio.pen`: config CRUD, publish, observe, rollback
- `prism-control-plane-route-studio.pen`: route editing and explain
- `prism-control-plane-explorations.pen`: alternative layouts and rejected experiments

Generated PNG exports should not live next to the `.pen` sources.

Use:

- `output/pencil/prism-control-plane/`

They should also stay out of Git.
The committed source of truth is the `.pen` workspace, not the exported image.

See [CONVENTIONS.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/pencil-dev/CONVENTIONS.md) for naming and revision policy, and [ARCHITECTURE.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/pencil-dev/ARCHITECTURE.md) for the reusable workspace layering model.

## Best-Practice Rules for Prism

These come directly from Pencil Dev's documented workflow and are adapted to Prism's control-plane use case.

1. Keep `.pen` files in the repo next to code and specs.
2. Save frequently because Pencil currently has no auto-save.
3. Commit `.pen` files like code and review diffs in Git.
4. Start with `variables` before drawing lots of screens.
5. Create reusable `components` before duplicating shell UI.
6. Use Pencil for `design first`, then decide whether to sync back to code.
7. Keep the design file in the same workspace as the codebase so the AI can see both.

## Visual QA Bar

For Prism, export hygiene is part of the design system, not a finishing pass.

Use [QUALITY-GATES.md](/Users/qiufeng/work/proxy/prism/docs/design/prism-control-plane/pencil-dev/QUALITY-GATES.md) as the mandatory acceptance bar before refreshing any `--latest` PNG.

The short version:

1. no visible overflow or cropped text
2. no floating badges or buttons competing with prose
3. no weak gray-on-gray contrast in dense dark panels
4. no divergence between workspace patterns and `Shell Kit`
5. run layout snapshots on the full screen and the highest-risk panels before export

## Prism-Specific Design Sequence

Do not start by drawing every page.

Start in this order:

1. `Foundations`
   Define color, spacing, typography, radius, status, and elevation variables.
   The first source of truth should be the current prototype tokens and the dashboard CSS tokens.

2. `Shell`
   Build reusable components for:
   - global context bar
   - left navigation
   - workspace header
   - KPI card
   - filter row
   - table row
   - inspector section
   - status pill
   - action button

3. `Traffic Lab`
   Make request debugging the first full workspace, because it is the fastest operator loop.

4. `Change Studio`
   Add config CRUD, publish review, canary observe, and rollback evidence.

5. `Route Studio`
   Add route draft editing, explain, blast radius, and publish linkage.

## Recommended Codex Prompts

These prompts are a good fit for Pencil MCP and Prism's design scope:

```text
Create variables for a data-dense AI gateway control plane using the existing prototype styling.
```

```text
Build reusable shell components for a control plane: sidebar, top context bar, workspace header, inspector panel, KPI cards, table rows, status pills, and action buttons.
```

```text
Design the Traffic Lab workspace for request-session debugging with filters, trace chain, fallback reasoning, upstream transform inspection, and replay controls.
```

```text
Design the Change Studio workspace for config registry, object detail, diff review, staged rollout, watch window, and rollback criteria.
```

```text
Import design tokens from the current CSS files and align the .pen variables to them.
```

## First Implementation Goal

The first useful deliverable is not a fully polished system.

It is:

- one working `.pen` file
- one variable set
- one reusable shell kit
- one high-value workspace: `Traffic Lab`

Once that is stable, `Change Studio` should be the second workspace, followed by `Route Studio`.

## Current Review Artifacts

The current Pencil review set now includes `Prism Foundations`, `Command Center`, `Traffic Lab`, `Change Studio`, `Route Studio`, and `Provider Atlas`.

- [foundations-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/foundations-overview--latest.png)
- [command-center-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/command-center-overview--latest.png)
- [command-center-signal-queue--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/command-center-signal-queue--latest.png)
- [command-center-side-stack--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/command-center-side-stack--latest.png)
- [command-center-inspector--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/command-center-inspector--latest.png)
- [shell-kit-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/shell-kit-overview--latest.png)
- [traffic-lab-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/traffic-lab-overview--latest.png)
- [traffic-lab-sessions-panel--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/traffic-lab-sessions-panel--latest.png)
- [traffic-lab-trace-panel--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/traffic-lab-trace-panel--latest.png)
- [traffic-lab-inspector--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/traffic-lab-inspector--latest.png)
- [change-studio-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/change-studio-overview--latest.png)
- [change-studio-registry--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/change-studio-registry--latest.png)
- [change-studio-workbench--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/change-studio-workbench--latest.png)
- [change-studio-watch-window--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/change-studio-watch-window--latest.png)
- [change-studio-inspector--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/change-studio-inspector--latest.png)
- [route-studio-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/route-studio-overview--latest.png)
- [route-studio-workbench--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/route-studio-workbench--latest.png)
- [route-studio-scenario-matrix--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/route-studio-scenario-matrix--latest.png)
- [route-studio-inspector--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/route-studio-inspector--latest.png)
- [provider-atlas-overview--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/provider-atlas-overview--latest.png)
- [provider-atlas-workbench--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/provider-atlas-workbench--latest.png)
- [provider-atlas-coverage-matrix--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/provider-atlas-coverage-matrix--latest.png)
- [provider-atlas-inspector--latest.png](/Users/qiufeng/work/proxy/prism/output/pencil/prism-control-plane/provider-atlas-inspector--latest.png)
