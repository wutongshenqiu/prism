批量合并多个 PR。Argument $ARGUMENTS: `[PR numbers...]`

用法:
- `/merge` — 合并当前分支对应的 PR
- `/merge 81 85` — 按顺序合并 PR #81 和 #85
- `/merge 81 85 86` — 按顺序合并 3 个 PR

Steps:

1. **解析参数**: 从 `$ARGUMENTS` 中提取 PR 编号列表
   - 如果 `$ARGUMENTS` 为空: 自动检测当前分支的 PR（`gh pr view --json number -q .number`），将其作为唯一待合并 PR
2. **预检查**: 对每个 PR 并行执行:
   - `gh pr view <N> --json state,mergeable,headRefName,baseRefName` — 确认 PR 状态
   - 标记冲突的 PR
3. **按顺序合并**: 对每个 PR 依次执行:
   a. `gh pr checks <N> --watch` — 等 CI 通过
   b. 如果有冲突:
      - `git fetch origin`
      - `git checkout <head-branch>`
      - `git rebase origin/main`
      - 如果 rebase 冲突：解决冲突，`GIT_EDITOR=true git rebase --continue`
      - `git push --force-with-lease`
      - `sleep 10` — 等待 CI 重新触发
      - `gh pr checks <N> --watch` — 再次等 CI
   c. **Stacked PR 安全检查**: `gh pr list --base <head-branch> --state open --json number`
      - 如果有依赖 PR: 先 `gh pr edit <dep-pr> --base main`
   d. `gh pr merge <N> --merge --delete-branch`
   e. `git checkout main && git pull origin main`
   f. 清理本地分支: `git branch -d <head-branch>`
4. **结果报告**: 汇总每个 PR 的合并状态

注意:
- 合并顺序很重要：先合并基础 PR，再合并依赖 PR
- 每次合并后 pull main，确保后续 rebase 基于最新代码
- 使用 `--force-with-lease`（而非 `--force`）确保安全推送
