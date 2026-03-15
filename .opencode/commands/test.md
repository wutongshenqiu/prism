---
description: "Run project tests (cargo, dashboard/web, Playwright, docker e2e, or specific filter)"
---

Run project tests. Mode: $1 (default: all).

Modes:
- "all" — `cargo test --workspace`
- "unit" — `cargo test --workspace --lib`
- "e2e" — `cargo test --test e2e -- --ignored`
- "web" — `cd web && npm run lint && npm run test && npm run build`
- "web-e2e" — `cd web && npm run test:e2e`
- "dashboard" — `make test && cd web && npm run lint && npm run test && npm run build && npm run test:e2e`
- "e2e-docker" — `make test-e2e-docker`
- "e2e-docker-full" — `TEST_LEVEL=full make test-e2e-docker`
- Other — `cargo test --workspace $1`

Steps:
1. If the change touches `crates/server/src/handler/dashboard/`, `crates/server/tests/dashboard_tests.rs`, `web/src/`, or `web/e2e/`, prefer `dashboard`
2. Run `cargo check --workspace` first (skip for `web`, `web-e2e`, and e2e-docker modes)
3. Execute the test command for the specified mode
4. If failures: list each failed test with name, error, source location, and cause analysis
5. Summarize: passed / failed / ignored counts

Notes:
- `web-e2e` / `dashboard` use the live local Prism + Vite Playwright flow
- e2e-docker requires `E2E_BAILIAN_API_KEY` env var
- e2e-docker supports `TEST_FILTER` env var for case filtering
