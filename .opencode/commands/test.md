---
description: "Run project tests (all/unit/e2e/e2e-docker/e2e-docker-full/specific)"
---

Run project tests. Mode: $1 (default: all).

Modes:
- "all" — `cargo test --workspace`
- "unit" — `cargo test --workspace --lib`
- "e2e" — `cargo test --test e2e -- --ignored`
- "e2e-docker" — `make test-e2e-docker`
- "e2e-docker-full" — `TEST_LEVEL=full make test-e2e-docker`
- Other — `cargo test --workspace $1`

Steps:
1. Run `cargo check --workspace` first (skip for e2e-docker modes)
2. Execute the test command for the specified mode
3. If failures: list each failed test with name, error, source location, and cause analysis
4. Summarize: passed / failed / ignored counts

Notes:
- e2e-docker requires `E2E_BAILIAN_API_KEY` env var
- e2e-docker supports `TEST_FILTER` env var for case filtering
