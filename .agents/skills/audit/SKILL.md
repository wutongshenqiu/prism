---
name: audit
description: "Full codebase review and batch fix. Supports scopes: all/security/stability/performance/refactor. Use --fix to auto-fix."
---

# Codebase Audit

Full codebase review with optional batch fix. The user specifies an optional scope (default: all) and --fix flag.

Steps:

1. **Parse arguments**: Extract `--fix` flag and scope filter (all/security/stability/performance/refactor)

2. **Parallel review**: Dispatch review tasks by crate:
   - `prism-core` — config, error handling, type safety, panic sites
   - `prism-provider` — executors, SSE parsing, credential routing, network safety
   - `prism-translator` — format conversion, streaming state, JSON parsing
   - `prism-server` — middleware, dispatch, handlers, auth
   - Cross-crate — architectural consistency, interface contracts, dependency direction

   Each review checks:
   - **Security**: unwrap/panic, buffer overflow, injection, auth bypass, info leaks
   - **Stability**: swallowed errors, lock poisoning, TOCTOU, resource leaks
   - **Performance**: O(n^2) lookups, unnecessary allocations, hot path optimization
   - **Refactoring**: code duplication, oversized functions, type safety improvements

3. **Classify findings**: Organize by priority (P0 Critical, P1 High, P2 Medium)

4. **If --fix**: Batch fix all findings, verify with `make lint` + `make test`

5. **If no --fix**: Optionally create GitHub Issues for tracking

6. **Report**: Total findings, fixes applied, remaining items
