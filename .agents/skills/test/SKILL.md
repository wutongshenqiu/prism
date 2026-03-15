---
name: test
description: "Run project tests. Supports cargo, dashboard/web, Playwright, docker e2e, or a specific test name."
---

# Test Runner

Run project tests. The user specifies a test mode (default: all).

Modes:
- "all" — Run all tests: `cargo test --workspace`
- "unit" — Unit tests only (no integration tests): `cargo test --workspace --lib`
- "e2e" — E2E integration tests: `cargo test --test e2e -- --ignored`
- "web" — Frontend validation bundle: `cd web && npm run lint && npm run test && npm run build`
- "web-e2e" — Real browser dashboard tests: `cd web && npm run test:e2e`
- "dashboard" — Dashboard/control-plane bundle: `make test && cd web && npm run lint && npm run test && npm run build && npm run test:e2e`
- "e2e-docker" — Docker E2E tests (quick level): `make test-e2e-docker`
- "e2e-docker-full" — Docker E2E full tests: `TEST_LEVEL=full make test-e2e-docker`
- Any other value — Use as test filter: `cargo test --workspace <value>`

Steps:
1. Choose the narrowest bundle that still validates the touched surface.
   - If the change touches `crates/server/src/handler/dashboard/`, `crates/server/tests/dashboard_tests.rs`, `web/src/`, or `web/e2e/`, prefer `dashboard` over plain cargo-only modes.
2. Run `cargo check --workspace` first for cargo-backed modes (skip for `web`, `web-e2e`, and e2e-docker modes)
3. Execute the test command for the specified mode
4. If failures occur:
   - List each failed test name and error message
   - Locate the corresponding source and test files
   - Analyze failure cause (compile error / assertion failure / panic)
5. Summarize: passed / failed / ignored counts

Notes:
- `web-e2e` / `dashboard` use the live local Prism + Vite Playwright flow; this is not a mocked-only path
- e2e-docker and e2e-docker-full require `E2E_BAILIAN_API_KEY` environment variable
- e2e-docker supports `TEST_FILTER` environment variable to filter test cases (e.g., `TEST_FILTER=cline`)

Examples:
```
test                    # All cargo tests
test unit               # Unit tests only
test web                # Frontend lint + unit + build
test web-e2e            # Real browser dashboard tests
test dashboard          # Dashboard/control-plane full bundle
test e2e-docker         # Docker E2E (quick)
test e2e-docker-full    # Docker E2E (full)
test test_should_cloak  # Run matching test
test cloak              # Run cloak-related tests
```
