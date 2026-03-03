---
name: doc-audit
description: "Audit documentation vs code consistency. Supports: quick (default), full, types, api, agents, specs. Use --fix to auto-fix."
---

# Documentation Auditor

Audit documentation against code for consistency. The user specifies a scope (default: quick).

Supports `--fix` suffix to auto-fix discovered discrepancies (e.g., `doc-audit full --fix`).

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

### --fix mode (when arguments include `--fix`)

After completing the audit and outputting the discrepancy table, auto-fix all discovered discrepancies:

7. For each discrepancy, read the target doc file and apply the fix:
   - **Field/enum mismatch**: Update doc with actual definition from code
   - **Missing entry**: Extract definition from code and add to doc
   - **Outdated description**: Rewrite based on current code behavior
   - **Broken link**: Update to correct file path
   - **Spec status mismatch**: Update metadata Status field
8. After fixing, re-run the audit (same scope, without --fix) to verify zero discrepancies
9. Output fix summary:
   | # | File | Fix Content | Status |
   |---|------|-------------|--------|
