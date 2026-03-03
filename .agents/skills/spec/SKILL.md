---
name: spec
description: "Manage Spec lifecycle. Subcommands: create, list, status, advance, td."
---

# Spec Manager

Manage the Spec-Driven Development lifecycle. The user provides a subcommand.

## create "<title>"

Create a new Spec:
- Read docs/specs/_index.md to determine the next number (SPEC-NNN)
- Create SPEC-NNN/ directory under docs/specs/active/
- Copy template from docs/specs/_templates/prd.md, fill in number and title
- Register in docs/specs/_index.md table, Status = `Draft`
- Output creation result and next step guidance

## list [active|completed|all]

List Specs:
- Read docs/specs/_index.md
- Filter by status (default: `active`)
- Display: Spec ID | Title | Status | Location

## status SPEC-NNN

View Spec details:
- Read the corresponding Spec directory's prd.md and technical-design.md (if exists)
- Output full status info, summary, related code paths

## advance SPEC-NNN

Advance Spec to next stage:
- Read current status and advance to next stage
- Update docs/specs/_index.md
- Status flow: Draft → Active → Completed
- Active → Completed: move directory from active/ to completed/

## td SPEC-NNN

Create Technical Design:
- Copy template from docs/specs/_templates/technical-design.md to Spec directory
- Fill in number and title
- If PRD already has content, pre-fill TD Overview based on PRD's Goals and User Stories

Examples:
```
spec create "WebSocket Support"
spec list active
spec list all
spec status SPEC-008
spec advance SPEC-008
spec td SPEC-008
```
