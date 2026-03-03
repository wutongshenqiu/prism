---
name: merge
description: "Batch merge multiple PRs. Provide PR numbers separated by spaces."
---

# Batch PR Merger

Merge multiple PRs in sequence. The user provides PR numbers.

Usage: `merge <PR numbers...>` (e.g., `merge 81 85 86`)

Steps:

1. **Parse arguments**: Extract PR number list
2. **Pre-check**: For each PR in parallel:
   - `gh pr view <N> --json state,mergeable,headRefName,baseRefName` — confirm PR status
   - Flag conflicting PRs
3. **Merge sequentially**: For each PR:
   a. `gh pr checks <N> --watch` — wait for CI to pass
   b. If conflicts:
      - `git fetch origin`
      - `git checkout <head-branch>`
      - `git rebase origin/main`
      - If rebase conflicts: resolve, `GIT_EDITOR=true git rebase --continue`
      - `git push --force-with-lease`
      - Wait for CI to re-trigger, then `gh pr checks <N> --watch`
   c. **Stacked PR safety check**: `gh pr list --base <head-branch> --state open --json number`
      - If dependent PRs exist: `gh pr edit <dep-pr> --base main` first
   d. `gh pr merge <N> --merge --delete-branch`
   e. `git checkout main && git pull origin main`
   f. Clean up local branch: `git branch -d <head-branch>`
4. **Report**: Summarize merge status for each PR

Notes:
- Merge order matters: merge base PRs first, then dependent PRs
- Pull main after each merge to ensure subsequent rebases use latest code
- Use `--force-with-lease` (not `--force`) for safe push
