---
description: "Run code linting (check/fix)"
---

Run code linting. Mode: $1 (default: check).

**check mode:**
1. `cargo fmt --check` — check formatting
2. `cargo clippy --workspace -- -D warnings` — check lint rules
3. Summarize: format issues + clippy warnings
4. List each issue with file location and fix suggestion

**fix mode:**
1. `cargo fmt` — auto-format
2. `cargo clippy --workspace -- -D warnings` — check lint rules
3. If warnings: `cargo clippy --fix --allow-dirty --workspace`
4. Re-run clippy to confirm all pass
5. Summarize: auto-fixed count + remaining manual-fix count
