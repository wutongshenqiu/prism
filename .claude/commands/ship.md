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
2. **格式化 + 检查**: Run `make fmt` + `make lint` — 发现 clippy 问题则修复代码并重新检查，直到通过
3. **测试**: Run `make test` — 发现失败则修复并重新测试，直到通过
4. **文档同步检查**: 检查改动是否涉及以下文件，如有则提醒同步文档:
   - `crates/core/src/provider.rs` 或 `config.rs` 变更 → 检查 `docs/reference/types/` 是否需要更新
   - `crates/server/src/handler/` 或 `lib.rs` 路由变更 → 检查 `docs/reference/api-surface.md`
   - `crates/provider/src/` 新增 executor → 检查 `docs/playbooks/add-provider.md`
   - `crates/translator/src/` 新增翻译器 → 检查 `docs/playbooks/add-translator.md`
5. **Spec 关联检查**: 读取 `docs/specs/_index.md`，如果有关联的 Active Spec:
   - 检查改动是否完成了 Spec 的全部内容
   - 如果已完成：自动执行 `/spec advance SPEC-NNN`（Active → Completed），将目录从 `active/` 移动到 `completed/`，更新 `_index.md`
   - 将 Spec 变更一并加入本次提交
6. **分支管理**: 如果当前在 `main` 分支:
   - 从 commit message 推导分支名（如 `feat: add daemon support` → `feature/daemon-support`）
   - 分支名规则: `feat:` → `feature/`, `fix:` → `fix/`, `docs:` → `docs/`, `refactor:` → `refactor/`, `test:` → `test/`, `chore:` → `chore/`
   - 自动 `git checkout -b <branch-name>`
7. **暂存**: `git add` 改动文件（排除 `config.yaml` / `.env` 等敏感文件）
8. **提交**: 如果参数中指定了 commit message，使用该 message；否则从分支名 + 改动推导（conventional commit 格式: `feat:`/`fix:`/`docs:`/`refactor:`/`test:`/`chore:`）。执行 `git commit`
9. **推送**: `git push -u origin HEAD`
10. **创建 PR**（除非 `--no-pr`）:
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
   c. `gh pr create --title "..." --body "..."`
   d. `gh pr checks --watch` — 等待 CI 完成
   e. 如果 `--merge` 且 CI 全部通过: `gh pr merge --merge --delete-branch`
   f. 报告 PR URL 和 CI 结果（及 merge 状态）
11. **结果报告**: 报告 commit SHA、push 结果、PR URL（如适用）、merge 状态（如适用）
