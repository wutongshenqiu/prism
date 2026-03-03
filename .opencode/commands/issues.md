---
description: "Generate GitHub Issues from a Spec (SPEC-NNN [--milestone name])"
---

Generate GitHub Issues from Spec: $1 $2

Steps:

1. **Read Spec**: Read `docs/specs/active/$1/prd.md` and `technical-design.md`
   - Extract task list from TD's Task Breakdown
   - If no TD, derive tasks from PRD's Goals / User Stories / Requirements

2. **Check milestone**: If `--milestone` specified ($2):
   - Check if milestone exists via `gh api repos/:owner/:repo/milestones`
   - Create if needed

3. **Create labels**: Ensure `spec` and `epic` labels exist, plus module labels as needed

4. **Generate Epic issue**:
   - Title: `$1: <Spec Title> — Epic`
   - Body: Spec overview + sub-task checklist
   - Labels: `spec`, `epic`

5. **Generate Sub-task issues**: One per Task Breakdown item
   - Title: `[$1] <task description>`
   - Body: description, tasks, acceptance criteria, dependencies
   - Labels: `spec` + module labels

6. **Update Epic body**: Replace checklist with actual issue numbers

7. **Report**: List all created issues with numbers and titles

Notes:
- Checks for existing issues to avoid duplicates
- Sub-tasks created in dependency order
- Each sub-task should be completable in one PR
