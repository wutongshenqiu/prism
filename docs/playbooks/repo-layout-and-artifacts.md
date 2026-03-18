# Playbook: Repo Layout and Artifact Policy

This playbook defines where Prism source files, tests, design sources, agent skills, and generated artifacts should live.

The goal is simple:

- keep the repository reviewable
- keep durable source-of-truth files in Git
- keep machine-local outputs out of Git

## Decision Rule

Commit source, specification, and reusable workflow assets.

Do not commit:

- machine-local configuration
- generated screenshots and reports
- scratch files
- build outputs
- tool-local state

If a file is required to reproduce, review, or extend the product, it should usually be tracked.
If a file is only an execution byproduct, it should usually stay local.

## Canonical Layout

```text
crates/                       Rust crates
src/                          binary entry point
web/                          canonical control-plane frontend
tests/                        end-to-end and docker integration tests
docs/specs/                   spec registry and active/completed specs
docs/reference/               stable technical reference
docs/playbooks/               workflow and contribution playbooks
docs/design/prism-control-plane/
  prototype.*                 HTML prototype source
  pencil-dev/
    workspaces/               canonical .pen source files
    prompts/                  reusable Pencil/Codex prompts
.agents/skills/               canonical shared agent skills
.claude/commands/             tracked Claude command shims
.opencode/commands/           tracked OpenCode command shims
output/                       local-only generated review artifacts
```

## What Must Be Committed

### Product and implementation source

- `crates/**`
- `src/**`
- `web/src/**`
- `web/scripts/real-flow-check.mjs`
- `web/package.json`
- `web/package-lock.json`
- `Cargo.toml`
- `Cargo.lock`

### Tests

- Rust tests in `crates/**/tests` and inline unit tests
- frontend tests in `web/src/**/*.test.*`
- deterministic browser flow scripts in `web/scripts/`
- docker/e2e cases in `tests/e2e*`

### Specs and docs

- `docs/specs/**`
- `docs/reference/**`
- `docs/playbooks/**`
- design rationale under `docs/design/prism-control-plane/**`

### Pencil source of truth

Commit these:

- `docs/design/prism-control-plane/pencil-dev/workspaces/*.pen`
- `docs/design/prism-control-plane/pencil-dev/prompts/*.md`
- `docs/design/prism-control-plane/pencil-dev/*.md`

Do not commit generated PNG exports from Pencil.

### Shared agent assets

Commit these:

- `.agents/skills/**`
- `.claude/commands/**`
- `.opencode/commands/**`
- `.claude/settings.json`

These are reusable project workflow assets, not personal machine state.

## What Must Stay Local

### Generated outputs

Do not commit:

- `output/**`
- Playwright screenshots
- Playwright `report.json`
- Pencil PNG exports
- ad hoc browser captures
- one-off debug exports

### Local runtime config

Do not commit:

- `config.yaml`
- `config.control-plane-greenfield.yaml`
- `config.yaml.managed-auth.d/**`
- `.env`

Tracked examples should live in:

- `config.example.yaml`
- `config.test.yaml`

### Tool-local state

Do not commit:

- `.codex/**`
- `target/**`
- `web/node_modules/**`
- `web/dist/**`
- top-level `dist/**`

## Screenshots and Reports

Screenshots are useful, but they are review artifacts, not source.

Preferred local layout:

```text
output/
  playwright/
    real-flow/
      report.json
      command-center.png
      traffic-lab.png
      provider-atlas.png
      route-studio.png
      change-studio.png
  pencil/
    prism-control-plane/
      command-center-overview--latest.png
      traffic-lab-overview--latest.png
      ...
```

### Best format

- use `PNG` for UI screenshots and Pencil exports
- use `JSON` for machine-readable run summaries
- use stable semantic filenames instead of `v2`, `final`, or random hashes

Recommended naming:

- `command-center.png`
- `traffic-lab.png`
- `report.json`
- `command-center-overview--latest.png`

### Retention rule

Keep them locally for review and debugging.
Do not commit them unless there is an explicit product reason to version a reference artifact.

In normal engineering flow, screenshots should be regenerated, not versioned.

## Pencil Policy

Pencil has a clear split between source and output.

Commit:

- `.pen` workspace files
- prompts
- architecture/conventions/quality notes

Do not commit:

- exported PNG/JPEG review boards
- temporary desktop saves
- MCP/session artifacts

If only PNG exists and the `.pen` source was not saved yet, the PNG is a temporary review artifact, not a substitute for a committed design source.

## Test Asset Policy

### Keep in Git

- test source code
- stable fixtures
- deterministic helper scripts
- mock payloads that are intentionally curated

### Keep out of Git

- screenshots from test runs
- browser videos
- traces
- transient logs
- timestamped reports

If a visual regression baseline is ever needed, store it in an explicit, reviewed fixture location rather than mixing it into `output/`.

## Skill Policy

There should be one canonical place for shared skills:

- `.agents/skills/`

Tool-specific mirrors should stay thin:

- `.claude/commands/`
- `.opencode/commands/`

`.codex/` is machine-local and should not be used as the repository source of truth.

## Practical Review Checklist

Before committing, ask:

1. Is this file source-of-truth or just execution output?
2. Can another developer reproduce it from tracked code and config?
3. Does this file contain machine-local state, credentials, or transient paths?
4. Would a future diff reviewer want to inspect changes to this file?

If the file fails those checks, it probably should not be committed.
