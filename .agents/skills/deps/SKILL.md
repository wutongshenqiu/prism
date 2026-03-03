---
name: deps
description: "Dependency management. Supports: list (default), merge, fix, update."
---

# Dependency Manager

Manage project dependencies. The user specifies a subcommand (default: list status).

## No arguments — list status

1. `gh pr list --label dependencies --json number,title,url,statusCheckRollup,mergeable --jq '.[]'`
2. Group by CI status: passing / failing / pending / unmergeable
3. Report totals and per-group counts

## merge

1. Fetch all Dependabot PRs: `gh pr list --label dependencies --json number,title,url,statusCheckRollup,mergeable`
2. Filter for CI-passing and mergeable PRs
3. For each qualifying PR:
   a. `gh pr merge {number} --squash --delete-branch`
   b. Report merge result
4. Summarize: merged N, skipped M (with reasons)

## fix

1. Fetch CI-failing Dependabot PRs
2. For each failing PR:
   a. `gh pr checkout {number}`
   b. `make lint` — fix clippy/fmt issues
   c. `make test` — fix compile errors or test failures
   d. `git add` + `git commit -m "fix: resolve build issues after dependency update"`
   e. `git push`
   f. `gh pr checks --watch` — wait for CI
   g. If CI passes: `gh pr merge {number} --squash --delete-branch`
   h. If CI still fails: report error, skip this PR
3. Switch back to original branch: `git checkout -`
4. Summarize results

## update

1. `cargo update`
2. `make lint` — fix issues
3. `make test` — fix failures
4. If lint+test pass:
   a. `git add Cargo.lock`
   b. `git commit -m "chore: update dependencies"`
   c. Report which dependencies were updated
5. If lint or test fail: report issues, do not commit
