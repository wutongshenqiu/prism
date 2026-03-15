---
name: dashboard-control-plane
description: "Use when hardening Prism's dashboard/control plane for correctness, runtime-truth UX, config transaction safety, realtime log behavior, or live Playwright verification."
---

# Dashboard Control Plane

Use this skill when the task is about dashboard/backend correctness rather than a single isolated bug.

Typical triggers:
- "dashboard truth"
- "config workspace is misleading"
- "websocket/request logs behavior is wrong"
- "Protocols / Models / System page should reflect runtime truth"
- "need real Playwright coverage, not just mocked tests"
- "reorganize related dashboard issues and close them with one PR"

## Working model

Do not keep the historical issue split if it no longer matches the code. Collapse the work into a few product-level buckets:

1. Config mutation truth
2. Runtime truth semantics
3. Realtime logs / websocket productization
4. Browser contract coverage

Treat those as the real units of work, then map them back to specs/issues later.

## Backend-first workflow

1. Start from backend truth:
   - `crates/server/src/handler/dashboard/`
   - `crates/server/tests/dashboard_tests.rs`
2. Then read the matching frontend surface:
   - `web/src/pages/`
   - `web/src/stores/`
   - `web/src/services/`
   - `web/e2e/`
3. Prefer deleting misleading UI states over preserving guessed semantics.

## Design rules

### Config workspace

- There should be one shared dashboard config write path.
- Preserve raw secret references like `env://` and `file://`.
- Make conflict / validation / internal failures explicit.
- Do not let frontend-invented section models drift away from `/api/dashboard/config/current`.

### Runtime truth pages

- System, Protocols, Models, Replay, and Config should derive semantics from backend/runtime data.
- Do not hardcode streaming, tools, health, or "configured/default" badges if backend truth exists.
- Unknown is better than a false green check.

### Realtime request logs

- Live insertion must respect active filters.
- Page semantics matter: if the user is not on page 1, do not silently mutate the visible slice.
- Surface websocket connection state in the UI.
- Token refresh / reconnect behavior should be visible, not console-only.

### Issue handling

- If several issues are actually one product change, solve the product change first.
- Use PR closing keywords only when the merged result truly closes the issue.
- After merge, verify the issues actually closed.

## Verification bundle

For dashboard/control-plane changes, the default bundle is:

```bash
make lint
make test
cd web && npm run test
cd web && npm run build
cd web && npm run test:e2e
```

Do not stop at cargo tests if the change touches:
- `crates/server/src/handler/dashboard/`
- `crates/server/tests/dashboard_tests.rs`
- `web/src/`
- `web/e2e/`

## Remote/live validation

Use remote validation only when the user explicitly asks for a real machine or deployed environment.

Preferred pattern:

1. Build the current branch, not an old shared instance.
2. Start an isolated backend on the remote host.
3. Tunnel local Playwright to the remote backend.
4. Keep fixture data deterministic.
5. Treat the remote run as an extra confidence layer, not a replacement for local live Playwright.

Avoid using an old shared remote service as the truth source when the task is about current code correctness.
