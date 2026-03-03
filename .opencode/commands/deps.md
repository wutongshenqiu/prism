---
description: "Dependency management (list/merge/fix/update)"
---
RUN gh pr list --label dependencies --json number,title,url,statusCheckRollup,mergeable

Dependency management. Subcommand: $1 (default: list status).

## No arguments — list status
Group the Dependabot PRs above by CI status: passing / failing / pending / unmergeable. Report totals.

## merge
Filter for CI-passing and mergeable PRs. For each: `gh pr merge {number} --squash --delete-branch`. Summarize merged vs skipped.

## fix
For each CI-failing PR:
1. `gh pr checkout {number}`
2. `make lint` — fix issues
3. `make test` — fix failures
4. Commit fixes, push, wait for CI
5. If CI passes: merge. If not: report and skip.
6. Switch back to original branch.

## update
1. `cargo update`
2. `make lint` + `make test`
3. If pass: `git add Cargo.lock && git commit -m "chore: update dependencies"`
4. If fail: report issues, do not commit
