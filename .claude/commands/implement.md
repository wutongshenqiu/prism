实现 Spec 的 Technical Design。Argument $ARGUMENTS: `SPEC-NNN`

用法:
- `/implement SPEC-008` — 读取 SPEC-008 的 TD，生成实现计划并执行

前置条件:
- Spec 必须处于 **Active** 状态（已有 PRD + TD）
- TD 中必须包含 Task Breakdown 或 Implementation Steps

Steps:

1. **读取 Spec**: 读取 `docs/specs/active/$ARGUMENTS/technical-design.md` 和 `docs/specs/active/$ARGUMENTS/prd.md`
   - 如果 Spec 不存在或不是 Active 状态，报错退出
   - 提取 TD 中的 Task Breakdown / Implementation Steps / 关键文件列表

2. **分析依赖**: 解析各任务之间的依赖关系
   - 哪些任务可以并行执行
   - 哪些任务有严格的先后顺序
   - 标注每个任务涉及的文件和 crate

3. **生成实现计划**: 基于 TD 的 Task Breakdown 创建 TaskList
   - 每个任务包含: subject, description（来自 TD）, activeForm
   - 设置任务依赖关系（blockedBy）
   - 展示计划给用户确认

4. **逐步实现**: 按依赖顺序执行每个任务
   - 每个任务开始前: `TaskUpdate` 标记 `in_progress`
   - 实现代码变更
   - 每个任务完成后:
     a. `cargo check --workspace` — 确保编译通过
     b. `TaskUpdate` 标记 `completed`
   - 如果遇到编译错误，立即修复再继续

5. **质量验证**: 所有任务完成后
   - `make fmt` — 格式化
   - `make lint` — 确保 clippy 通过
   - `make test` — 确保所有测试通过
   - 如果有失败，修复后重新验证

6. **结果报告**:
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
