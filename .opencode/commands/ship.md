---
description: "End-to-end commit pipeline: lint, test, commit, push, create PR. Detects existing PRs."
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
2. **Existing PR detection**: Check if current branch already has an open PR
   - If open PR + `--merge`: skip to CI wait + merge
   - If open PR + no `--merge`: report PR URL, suggest `--merge`
   - If no open PR: continue normal flow
3. **Format + Check**: `make fmt` + `make lint` — fix issues until passing
4. **Test**: `make test` — fix failures until passing
   - If changes touch `crates/server/src/handler/dashboard/`, `crates/server/tests/dashboard_tests.rs`, `web/src/`, or `web/e2e/`, also run:
     - `cd web && npm run lint`
     - `cd web && npm run test`
     - `cd web && npm run build`
     - `cd web && npm run test:e2e`
   - If the user explicitly asked for real-machine validation, run the remote/live validation bundle before ship completes
5. **Doc sync check**: If changes touch core types/server routes/providers, check if docs need updating
6. **Spec check**: Read `docs/specs/_index.md`, if related Active Spec is fully completed, advance it
7. **Branch management**:
   - On `main` with uncommitted changes: derive branch name from commit message, create and switch
   - On `main` with committed changes ahead: create a feature branch at the current `HEAD` and switch to it; do not rewrite `main`
8. **Stage**: `git add` changed files (exclude config.yaml/.env/secrets)
9. **Commit**: Use provided or derived conventional commit message
10. **Push**: `git push -u origin HEAD`
11. **Create PR** (unless `--no-pr`):
    - Derive title, generate body with Summary/Changes/Doc Impact/Test Plan
    - Include `Closes #...` only when the branch fully resolves the target issue
    - `gh pr create`, `gh pr checks --watch`
    - If `--merge` + CI passes: handle stacked PRs, then merge
12. **Cleanup** (merge only): checkout main, pull, delete local branch
13. **Post-merge verification** (merge only):
    - Verify the PR is actually `MERGED`
    - If closing keywords were used, confirm the target issues closed
14. **Report**: commit SHA, push result, PR URL, CI/merge status
