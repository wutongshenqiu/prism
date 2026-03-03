---
description: "Batch merge multiple PRs in sequence"
---

Batch merge PRs: $1 $2 $3 $4 $5

Steps:

1. **Parse**: Extract PR numbers from arguments
2. **Pre-check**: For each PR in parallel:
   - `gh pr view <N> --json state,mergeable,headRefName,baseRefName`
   - Flag conflicting PRs
3. **Merge sequentially**: For each PR:
   a. `gh pr checks <N> --watch` — wait for CI
   b. If conflicts:
      - `git fetch origin && git checkout <head-branch>`
      - `git rebase origin/main`
      - If rebase conflicts: resolve, `GIT_EDITOR=true git rebase --continue`
      - `git push --force-with-lease`
      - Wait for CI, then `gh pr checks <N> --watch`
   c. **Stacked PR check**: `gh pr list --base <head-branch> --state open --json number`
      - If dependent PRs: `gh pr edit <dep-pr> --base main` first
   d. `gh pr merge <N> --merge --delete-branch`
   e. `git checkout main && git pull origin main`
   f. `git branch -d <head-branch>`
4. **Report**: Merge status for each PR

Notes:
- Order matters: merge base PRs first
- Pull main after each merge for latest code
- Use `--force-with-lease` for safe push
