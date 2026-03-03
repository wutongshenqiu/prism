---
name: ship
description: "End-to-end commit pipeline: lint, test, commit, push, create PR, wait for CI. Detects existing PRs."
---

# Ship Pipeline

End-to-end commit pipeline. The user can provide flags and a commit message.

Usage: `ship [--no-pr] [--merge] [commit message]`

- `ship` — lint, test, commit, push, create PR, wait for CI, report
- `ship "feat: xxx"` — use specified commit message
- `ship --no-pr` — commit+push only, no PR
- `ship --merge` — auto-merge PR after CI passes
- `ship --merge "feat: xxx"` — specified message + auto merge

Steps:

1. **Parse arguments**: Extract `--no-pr`, `--merge` flags and commit message
2. **Existing PR detection**: Check if current branch already has an open PR via `gh pr list --head <branch>`
   - If open PR exists + `--merge` mode: skip to CI wait + merge step
   - If open PR exists + no `--merge`: report PR URL, suggest using `--merge`
   - If no open PR: continue normal flow
3. **Format + Check**: Run `make fmt` + `make lint` — fix clippy issues and re-check until passing
4. **Test**: Run `make test` — fix failures and re-test until passing
5. **Doc sync check**: Check if changes affect files that require doc updates:
   - `crates/core/src/provider.rs` or `config.rs` → check `docs/reference/types/`
   - `crates/server/src/handler/` or `lib.rs` routes → check `docs/reference/api-surface.md`
   - `crates/provider/src/` new executor → check `docs/playbooks/add-provider.md`
   - `crates/translator/src/` new translator → check `docs/playbooks/add-translator.md`
6. **Spec association check**: Read `docs/specs/_index.md`, if related Active Spec:
   - Check if changes complete the Spec fully
   - If complete: advance Spec (Active → Completed), move directory, update `_index.md`
   - Include Spec changes in this commit
7. **Branch management**:
   - If on `main` with uncommitted changes: derive branch name from commit message, create and switch
   - If on `main` with committed changes ahead of origin: create branch, reset main to origin, switch
8. **Stage**: `git add` changed files (exclude `config.yaml` / `.env` / secrets)
9. **Commit**: Use provided or derived commit message (conventional commit format)
10. **Push**: `git push -u origin HEAD`
11. **Create PR** (unless `--no-pr`):
    a. Derive PR title from branch name and commits
    b. Generate PR body with Summary, Changes, Spec & Doc Impact, Test Plan
    c. `gh pr create --title "..." --body "..."`
    d. `gh pr checks --watch` — wait for CI
    e. If `--merge` and CI passes: merge PR, handle stacked PRs if any
12. **Local cleanup** (merge mode only): checkout main, pull, delete local branch
13. **Report**: commit SHA, push result, PR URL, CI/merge status
