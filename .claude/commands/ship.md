端到端提交流水线。Argument $ARGUMENTS: `[--no-pr] [--merge] [commit message]`

用法:
- `/ship` — lint, test, commit, push, 创建 PR, 等 CI, 报告
- `/ship "feat: xxx"` — 用指定 commit message
- `/ship --no-pr` — 只 commit+push, 不创建 PR
- `/ship --no-pr "feat: xxx"` — 指定 message, 只 commit+push
- `/ship --merge` — CI 通过后自动 merge PR 并删除远程分支
- `/ship --merge "feat: xxx"` — 指定 message + 自动 merge

Steps:

1. **解析参数**: 从 `$ARGUMENTS` 中提取 `--no-pr`、`--merge` 标志和 commit message（如有）
2. **已有 PR 检测**: 用 `gh pr list --head <current-branch> --state open --json number,url` 检查当前分支是否已有 open PR
   - 如果有 open PR 且是 `--merge` 模式: 跳过步骤 3-10，直接跳到步骤 11d（等待 CI + merge）
   - 如果有 open PR 且非 `--merge` 模式: 报告 PR URL，提示用 `--merge` 来合并
   - 如果没有 open PR: 继续正常流程
3. **格式化 + 检查**: Run `make fmt` + `make lint` — 发现 clippy 问题则修复代码并重新检查，直到通过
4. **测试**: Run `make test` — 发现失败则修复并重新测试，直到通过
   - 如果改动涉及 `crates/server/src/handler/dashboard/`、`crates/server/tests/dashboard_tests.rs`、`web/src/` 或 `web/e2e/`，额外运行：
     - `cd web && npm run lint`
     - `cd web && npm run test`
     - `cd web && npm run build`
     - `cd web && npm run test:e2e`
   - 如果用户明确要求真实机器/远程验证，在 ship 完成前补跑 remote/live 验证
5. **文档同步检查**: 检查改动是否涉及以下文件，如有则提醒同步文档:
   - `crates/core/src/provider.rs` 或 `config.rs` 变更 → 检查 `docs/reference/types/` 是否需要更新
   - `crates/server/src/handler/` 或 `lib.rs` 路由变更 → 检查 `docs/reference/api-surface.md`
   - `crates/provider/src/` 新增 executor → 检查 `docs/playbooks/add-provider.md`
   - `crates/translator/src/` 新增翻译器 → 检查 `docs/playbooks/add-translator.md`
6. **Spec 关联检查**: 读取 `docs/specs/_index.md`，如果有关联的 Active Spec:
   - 检查改动是否完成了 Spec 的全部内容
   - 如果已完成：自动执行 `/spec advance SPEC-NNN`（Active → Completed），将目录从 `active/` 移动到 `completed/`，更新 `_index.md`
   - 将 Spec 变更一并加入本次提交
7. **分支管理**:
   - 如果当前在 `main` 分支且有**未暂存/未提交**的改动:
     - 从 commit message 推导分支名（如 `feat: add daemon support` → `feature/daemon-support`）
     - 分支名规则: `feat:` → `feature/`, `fix:` → `fix/`, `docs:` → `docs/`, `refactor:` → `refactor/`, `test:` → `test/`, `chore:` → `chore/`
     - 自动 `git checkout -b <branch-name>`
   - 如果当前在 `main` 分支且改动**已经提交**（即 main 领先 origin/main）:
     - 从最新 commit message 推导分支名
     - `git branch <branch-name>` — 在当前 commit 创建分支
     - `git checkout <branch-name>` — 切换到新分支
     - 不要重写本地 `main`；后续合并后再回到 `main` 做正常同步
8. **暂存**: `git add` 改动文件（排除 `config.yaml` / `.env` 等敏感文件）
9. **提交**: 如果参数中指定了 commit message，使用该 message；否则从分支名 + 改动推导（conventional commit 格式: `feat:`/`fix:`/`docs:`/`refactor:`/`test:`/`chore:`）。执行 `git commit`
10. **推送**: `git push -u origin HEAD`
11. **创建 PR**（除非 `--no-pr`）:
   a. 从分支名和 commit 历史推导 PR 标题（conventional commit 格式，70 字符内）
   b. 生成 PR body:
      ```
      ## Summary
      <1-3 bullet points summarizing changes>

      ## Changes
      <按 crate 分组列出主要改动>

      ## Spec & Reference Doc Impact
      <列出涉及的 Spec 和需要更新的文档，或 "None">

      ## Test Plan
      - [ ] `make lint` passes
      - [ ] `make test` passes
      - [ ] <specific test scenarios>
      ```
      - 如果本次 PR 完整解决某些 issues/epic，加入 `Closes #...`
      - 如果只是部分完成，不要提前写 closing keywords
   c. `gh pr create --title "..." --body "..."`
   d. `gh pr checks --watch` — 等待 CI 完成
   e. 如果 `--merge` 且 CI 全部通过:
      - **Stacked PR 安全检查**: 合并**前**用 `gh pr list --base <branch> --state open --json number` 检查是否有其他 PR 以当前分支为 base
      - 如果有依赖 PR:
        1. **先**逐个 `gh pr edit <dep-pr> --base main` 更新依赖 PR 的 base（**必须在 merge 前完成**，否则 GitHub 会自动关闭依赖 PR）
        2. 然后 `gh pr merge --merge --delete-branch`
      - 如果没有依赖 PR: `gh pr merge --merge --delete-branch`
   f. 报告 PR URL 和 CI 结果（及 merge 状态）
12. **本地清理** (仅 `--merge` 且合并成功后):
    - `git checkout main && git pull origin main`
    - `git branch -d <branch-name>` — 删除本地已合并的分支
13. **合并后核查** (仅 `--merge` 且 PR 使用了 closing keywords):
    - 检查 PR 是否确实是 `MERGED`
    - 检查目标 issues 是否已关闭
14. **结果报告**: 报告 commit SHA、push 结果、PR URL（如适用）、merge 状态（如适用）
