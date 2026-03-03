---
name: doc-audit
description: "Audit documentation vs code consistency. Supports: quick (default), full, types, api, agents, specs."
---

# Documentation Auditor

Audit documentation against code for consistency. The user specifies a scope (default: quick).

Scopes:
- "quick": Check reference/types/ type definitions vs Rust source types only
- "full": reference/ full check + AGENTS.md + specs/completed/ cross-check + link validity
- "types": Check docs/reference/types/ files one by one:
  - enums.md vs crates/core/src/provider.rs, config.rs, cloak.rs enum definitions
  - config.md vs crates/core/src/config.rs config types
  - provider.md vs crates/core/src/provider.rs + crates/provider/src/ types and traits
  - errors.md vs crates/core/src/error.rs ProxyError and status_code mapping
- "api": API endpoint consistency:
  - docs/reference/api-surface.md endpoint table vs crates/server/src/lib.rs route definitions
  - Each handler's actual parameters and return format
- "agents": Check AGENTS.md vs code consistency:
  - Crate Responsibilities descriptions vs actual struct/trait/field definitions
  - API Endpoints table vs crates/server/src/lib.rs route definitions
  - Provider Matrix table vs crates/provider/src/ executor implementations
- "specs": Each completed Spec's technical-design.md vs corresponding code module key declarations

Note: "full" mode automatically includes agents checks.

Steps:
1. Read target documentation files (including AGENTS.md for full/agents mode)
2. Read corresponding Rust source files
3. Compare item by item: field names, types, enum variants, method signatures, defaults, serde attributes
4. Output discrepancy table:

| Item | Doc Location | Code Location | Doc Value | Code Value | Suggested Action |
|------|-------------|---------------|-----------|------------|------------------|

5. Check documentation internal link validity (full mode only)
6. Summarize: total discrepancies, by severity (error/missing/outdated)
