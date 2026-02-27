端到端提交流水线。Argument $ARGUMENTS: `[--no-pr] [commit message]`

用法:
- `/ship` — lint, test, commit, push, 创建 PR, 等 CI, 报告
- `/ship "feat: xxx"` — 用指定 commit message
- `/ship --no-pr` — 只 commit+push, 不创建 PR
- `/ship --no-pr "feat: xxx"` — 指定 message, 只 commit+push

Steps:

1. **解析参数**: 从 `$ARGUMENTS` 中提取 `--no-pr` 标志和 commit message（如有）
2. **格式化 + 检查**: Run `make fmt` + `make lint` — 发现 clippy 问题则修复代码并重新检查，直到通过
3. **测试**: Run `make test` — 发现失败则修复并重新测试，直到通过
4. **文档同步检查**: 检查改动是否涉及以下文件，如有则提醒同步文档:
   - `crates/core/src/provider.rs` 或 `config.rs` 变更 → 检查 `docs/reference/types/` 是否需要更新
   - `crates/server/src/handler/` 或 `lib.rs` 路由变更 → 检查 `docs/reference/api-surface.md`
   - `crates/provider/src/` 新增 executor → 检查 `docs/playbooks/add-provider.md`
   - `crates/translator/src/` 新增翻译器 → 检查 `docs/playbooks/add-translator.md`
5. **Spec 关联检查**: 检查是否有关联的 Spec — 如有活跃 Spec，确认 status 是否需要更新
6. **暂存**: `git add` 改动文件（排除 `config.yaml` / `.env` 等敏感文件）
7. **提交**: 如果参数中指定了 commit message，使用该 message；否则从分支名 + 改动推导（conventional commit 格式: `feat:`/`fix:`/`docs:`/`refactor:`/`test:`/`chore:`）。执行 `git commit`
8. **推送**: `git push -u origin HEAD`
9. **创建 PR**（除非 `--no-pr`）:
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
   e. 报告 PR URL 和 CI 结果
10. **结果报告**: 报告 commit SHA、push 结果、PR URL（如适用）
