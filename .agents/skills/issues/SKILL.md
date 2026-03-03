---
name: issues
description: "Generate GitHub Issues from a Spec. Provide SPEC-NNN and optional --milestone."
---

# Issues Generator

Generate GitHub Issues from a Spec's PRD/TD. The user provides a Spec ID and optional milestone.

Usage: `issues SPEC-NNN [--milestone "name"]`

Steps:

1. **Read Spec**: Read `docs/specs/active/<SPEC>/prd.md` and `technical-design.md` (if exists)
   - Extract task list from TD's Task Breakdown
   - If no TD, derive tasks from PRD's Goals / User Stories / Requirements

2. **Check milestone**: If `--milestone` specified:
   - `gh api repos/:owner/:repo/milestones` to check if it exists
   - If not, `gh api repos/:owner/:repo/milestones -f title="..."` to create

3. **Create labels**: Ensure required labels exist
   - `gh label create spec --color 0E8A16 --force` (if not exists)
   - Create module-specific labels as needed (e.g., `backend`, `frontend`, `dashboard`)

4. **Generate Epic issue**: Create Spec-level Epic issue
   - Title: `SPEC-NNN: <Spec Title> — Epic`
   - Body: Spec overview + sub-task checklist (filled with issue numbers later)
   - Labels: `spec`, `epic`
   - Milestone: if specified

5. **Generate Sub-task issues**: Create one issue per Task Breakdown item
   - Title: `[SPEC-NNN] <task description>`
   - Body: description, tasks checklist, acceptance criteria, dependencies
   - Labels: `spec` + module labels
   - Milestone: if specified

6. **Update Epic body**: Replace checklist with actual issue numbers

7. **Report results**: Output list of created issues

Notes:
- Does not create duplicates: checks for existing issues with same title first
- Sub-tasks created in dependency order so blocked-by references exist
- Task granularity: each sub-task should be completable in one PR
