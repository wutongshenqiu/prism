---
description: "End-to-end commit pipeline: lint, test, commit, push, create PR"
---

End-to-end commit pipeline. Arguments: $1 $2 $3 (flags and commit message).

Usage:
- `ship` — lint, test, commit, push, create PR, wait CI
- `ship "feat: xxx"` — specified commit message
- `ship --no-pr` — commit+push only
- `ship --merge` — auto-merge after CI
- `ship --merge "feat: xxx"` — specified message + auto merge

Steps:

1. **Parse arguments**: Extract `--no-pr`, `--merge` flags and commit message
2. **Format + Check**: `make fmt` + `make lint` — fix issues until passing
3. **Test**: `make test` — fix failures until passing
4. **Doc sync check**: If changes touch core types/server routes/providers, check if docs need updating
5. **Spec check**: Read `docs/specs/_index.md`, if related Active Spec is fully completed, advance it
6. **Branch management**:
   - On `main` with uncommitted changes: derive branch name from commit message, create and switch
   - On `main` with committed changes ahead: create branch, reset main to origin, switch
7. **Stage**: `git add` changed files (exclude config.yaml/.env/secrets)
8. **Commit**: Use provided or derived conventional commit message
9. **Push**: `git push -u origin HEAD`
10. **Create PR** (unless `--no-pr`):
    - Derive title, generate body with Summary/Changes/Doc Impact/Test Plan
    - `gh pr create`, `gh pr checks --watch`
    - If `--merge` + CI passes: handle stacked PRs, then merge
11. **Cleanup** (merge only): checkout main, pull, delete local branch
12. **Report**: commit SHA, push result, PR URL, CI/merge status
