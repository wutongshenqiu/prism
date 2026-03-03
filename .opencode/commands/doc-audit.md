---
description: "Audit documentation vs code consistency (quick/full/types/api/agents/specs)"
---

Audit documentation against code for consistency. Scope: $1 (default: quick).

Scopes:
- "quick": Check reference/types/ type definitions vs Rust source types only
- "full": reference/ full check + AGENTS.md + specs/completed/ cross-check + link validity
- "types": Check docs/reference/types/ files one by one against corresponding Rust source
- "api": API endpoint consistency — docs/reference/api-surface.md vs crates/server/src/lib.rs routes
- "agents": Check AGENTS.md vs code — Crate Responsibilities, API Endpoints, Provider Matrix
- "specs": Each completed Spec's technical-design.md vs corresponding code module

Steps:
1. Read target documentation files
2. Read corresponding Rust source files
3. Compare item by item: field names, types, enum variants, method signatures, defaults, serde attributes
4. Output discrepancy table with: Item | Doc Location | Code Location | Doc Value | Code Value | Suggested Action
5. Check documentation internal link validity (full mode only)
6. Summarize: total discrepancies, by severity (error/missing/outdated)
