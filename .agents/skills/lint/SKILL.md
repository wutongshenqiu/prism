---
name: lint
description: "Run code linting. Supports modes: check (report only) or fix (auto-fix)."
---

# Lint Runner

Run code linting checks. The user specifies a mode (default: check).

Modes:
- "check" — Report issues without modifying files
- "fix" — Auto-fix where possible

## check mode

1. `cargo fmt --check` — Check formatting
2. `cargo clippy --workspace --tests -- -D warnings` — Check lint rules
3. Summarize: format issue count + clippy warning count
4. If issues found, list each with file location and fix suggestion

## fix mode

1. `cargo fmt` — Auto-format
2. `cargo clippy --workspace --tests -- -D warnings` — Check lint rules
3. If clippy warnings found, try `cargo clippy --fix --allow-dirty --workspace`
4. Re-run `cargo clippy --workspace --tests -- -D warnings` to confirm all pass
5. Summarize: auto-fixed count + remaining manual-fix count
