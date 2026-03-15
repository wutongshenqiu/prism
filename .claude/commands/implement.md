实现 Spec。Argument $ARGUMENTS: `SPEC-NNN`

用法:
- `/implement SPEC-008` — 读取 SPEC-008 的 TD（或 PRD），生成实现计划并执行

前置条件:
- Spec 必须处于 **Active** 或 **Draft** 状态
- 优先使用 TD；如果没有 TD，自动生成 TD 后再实现

Steps:

1. **读取 Spec**: 读取 `docs/specs/active/$ARGUMENTS/` 下的文件
   - 如果 Spec 目录不存在，报错退出
   - 优先读取 `technical-design.md`，提取 Task Breakdown / Implementation Steps
   - **如果 TD 不存在**: 读取 `prd.md`，基于 Goals / User Stories 生成 `technical-design.md` 并写入 Spec 目录，然后继续
   - **检查 `implementation-handoff.md`**: 如果存在，读取其中定义的 Issue 序列和实现顺序
     - handoff 文件定义了 ordered issue list（`#NNN: title` 格式）和每个 issue 的 scope/acceptance criteria
     - 按 handoff 中的顺序逐个实现每个 issue，每完成一个 issue 运行 `make lint && make test`
     - 每个 issue 完成后用 `gh issue close NNN` 关闭对应 issue
   - 提取关键文件列表和涉及的 crate

2. **创建分支**: 基于 Spec ID 创建 feature 分支
   - `git checkout -b feature/$ARGUMENTS` (如 `feature/spec-012`)
   - 如果分支已存在，切换到该分支

3. **生成 GitHub Issues** (如尚未创建):
   - 检查是否已有 `SPEC-NNN` 相关 issues: `gh issue list --search "SPEC-NNN"`
   - 如果没有，按 `/issues` 命令的逻辑自动创建 Epic + Sub-task issues

4. **分析依赖**: 解析各任务之间的依赖关系
   - 哪些任务可以并行执行
   - 哪些任务有严格的先后顺序
   - 标注每个任务涉及的文件和 crate

5. **生成实现计划**: 基于 TD 的 Task Breakdown 创建 TaskList
   - 每个任务包含: subject, description（来自 TD）, activeForm
   - 设置任务依赖关系（blockedBy）
   - 展示计划给用户确认

6. **逐步实现**: 按依赖顺序执行每个任务
   - 每个任务开始前: `TaskUpdate` 标记 `in_progress`
   - 实现代码变更
   - 每个任务完成后:
     a. `cargo check --workspace` — 确保编译通过
     b. `cargo fmt` — 格式化当前文件
     c. `TaskUpdate` 标记 `completed`
   - 如果遇到编译错误，立即修复再继续

7. **质量验证**: 所有任务完成后
   - `make fmt` — 格式化
   - `make lint` — 确保 clippy 通过
   - `make test` — 确保所有测试通过
   - 如果有失败，修复后重新验证

8. **结果报告**:
   - 已完成的任务列表
   - 新增/修改的文件列表
   - 测试结果摘要
   - 下一步提示: 使用 `/ship` 提交

注意:
- 不要超出 TD 定义的范围
- 遵循项目的代码风格（见 CLAUDE.md）
- 新增的 struct/trait 需要派生 `Serialize`/`Deserialize`（如果是公开类型）
- 错误处理: library crate 用 `thiserror`，application crate 用 `anyhow`
- 如果 TD 中有模糊之处，优先参考已完成 Spec 的实现模式
