从 Spec 自动生成 GitHub Issues。Argument $ARGUMENTS: `SPEC-NNN [--milestone "name"]`

用法:
- `/issues SPEC-009` — 从 SPEC-009 的 PRD/TD 生成 GitHub issues
- `/issues SPEC-009 --milestone "Web Dashboard"` — 生成 issues 并关联到 milestone

Steps:

1. **读取 Spec**: 读取 `docs/specs/active/$SPEC/prd.md` 和 `technical-design.md`（如有）
   - 从 TD 的 Task Breakdown 提取任务列表
   - 如果没有 TD，从 PRD 的 Goals / User Stories / Requirements 推导任务

2. **检查 milestone**: 如果指定了 `--milestone`:
   - `gh api repos/:owner/:repo/milestones` 检查是否已存在
   - 如果不存在，`gh api repos/:owner/:repo/milestones -f title="..."` 创建

3. **创建 labels**: 确保所需 labels 存在
   - `gh label create spec --color 0E8A16 --force`（如不存在）
   - 根据 Spec 涉及的模块创建对应标签（如 `backend`, `frontend`, `dashboard`）

4. **生成 Epic issue**: 创建 Spec 级别的 Epic issue
   - 标题: `SPEC-NNN: <Spec 标题> — Epic`
   - Body: 包含 Spec 概述 + 子任务 checklist（稍后填充 issue 编号）
   - Labels: `spec`, `epic`
   - Milestone: 如指定

5. **生成 Sub-task issues**: 按 Task Breakdown 逐个创建
   - 标题: `[SPEC-NNN] <任务描述>`
   - Body:
     ```
     ## Description
     <任务详细描述，来自 TD/PRD>

     ## Tasks
     - [ ] <具体实现步骤>

     ## Acceptance Criteria
     <验收标准>

     ## Dependencies
     Blocked by: #<number> (如有前置依赖)
     Part of: #<epic-number>
     ```
   - Labels: `spec` + 模块标签
   - Milestone: 如指定

6. **更新 Epic body**: 用实际 issue 编号更新 Epic 的子任务 checklist

7. **结果报告**: 输出创建的 issues 列表
   ```
   Created N issues for SPEC-NNN:
   - #XX SPEC-NNN Epic
   - #XX [SPEC-NNN] Task 1
   - #XX [SPEC-NNN] Task 2
   ...
   ```

注意:
- 不会重复创建：先检查是否已有同名 issue
- Sub-task 按依赖顺序创建，确保 blocked-by 引用的 issue 已存在
- 任务粒度: 每个 sub-task 应该可以在一个 PR 中完成
