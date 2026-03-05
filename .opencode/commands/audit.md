---
description: "Full codebase review and batch fix"
---

Full codebase audit. Args: $1 (scope: all/security/stability/performance/refactor, default: all). Add --fix to auto-fix.

Steps:
1. Parse arguments: extract --fix flag and scope filter
2. Review each crate in parallel:
   - prism-core: config, errors, type safety, panic sites
   - prism-provider: executors, SSE, credential routing, network safety
   - prism-translator: format conversion, streaming state, JSON parsing
   - prism-server: middleware, dispatch, handlers, auth
   - Cross-crate: architecture, interface contracts
3. Check for: security (unwrap/panic, injection, auth bypass), stability (swallowed errors, lock poison, TOCTOU), performance (O(n^2), allocations), refactoring (duplication, type safety)
4. Classify findings by priority (P0/P1/P2)
5. If --fix: batch fix, then `make lint` + `make test`
6. If no --fix: optionally create GitHub Issues
7. Report: total findings, fixes applied, remaining items
