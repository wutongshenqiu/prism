---
name: test
description: "Run project tests. Supports modes: all, unit, e2e, e2e-docker, e2e-docker-full, or specific test name."
---

# Test Runner

Run project tests. The user specifies a test mode (default: all).

Modes:
- "all" — Run all tests: `cargo test --workspace`
- "unit" — Unit tests only (no integration tests): `cargo test --workspace --lib`
- "e2e" — E2E integration tests: `cargo test --test e2e -- --ignored`
- "e2e-docker" — Docker E2E tests (quick level): `make test-e2e-docker`
- "e2e-docker-full" — Docker E2E full tests: `TEST_LEVEL=full make test-e2e-docker`
- Any other value — Use as test filter: `cargo test --workspace <value>`

Steps:
1. Run `cargo check --workspace` — ensure compilation passes first (skip for e2e-docker modes)
2. Execute the test command for the specified mode
3. If failures occur:
   - List each failed test name and error message
   - Locate the corresponding source and test files
   - Analyze failure cause (compile error / assertion failure / panic)
4. Summarize: passed / failed / ignored counts

Notes:
- e2e-docker and e2e-docker-full require `E2E_BAILIAN_API_KEY` environment variable
- e2e-docker supports `TEST_FILTER` environment variable to filter test cases (e.g., `TEST_FILTER=cline`)

Examples:
```
test                    # All cargo tests
test unit               # Unit tests only
test e2e-docker         # Docker E2E (quick)
test e2e-docker-full    # Docker E2E (full)
test test_should_cloak  # Run matching test
test cloak              # Run cloak-related tests
```
