---
description: "Manage Spec lifecycle (create/list/status/advance/td)"
---

Manage the Spec lifecycle. Subcommand: $1, argument: $2.

## create "$2"
- Read docs/specs/_index.md for next SPEC-NNN number
- Create docs/specs/active/SPEC-NNN/ directory
- Copy docs/specs/_templates/prd.md, fill number and title
- Register in _index.md as Draft

## list $2
- Read docs/specs/_index.md, filter by status ($2: active/completed/all, default: active)
- Display: Spec ID | Title | Status | Location

## status $2
- Read SPEC-NNN's prd.md and technical-design.md (if exists)
- Output full status info, summary, related code paths

## advance $2
- Read current status, advance: Draft → Active → Completed
- Update _index.md
- Active → Completed: move from active/ to completed/

## td $2
- Copy docs/specs/_templates/technical-design.md to Spec directory
- Fill number and title
- Pre-fill Overview from PRD if content exists
