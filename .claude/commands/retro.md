复盘近期对话，沉淀改进到 commands/skills/workflow。Argument $ARGUMENTS: `[sessions count]`（默认 1，即当前会话）

用法:
- `/retro` — 复盘当前会话
- `/retro 3` — 复盘最近 3 个会话

Steps:

1. **收集上下文**: 并行读取以下内容
   - 所有 `.claude/commands/*.md` — 现有命令定义
   - `.claude/settings.json` — 权限和 hooks 配置
   - `.agents/skills/` — 现有 portable skills
   - `docs/playbooks/coding-agent-workflow.md` — 当前开发流程
   - `CLAUDE.md` 和 `AGENTS.md` — 项目约定

2. **收集会话记录**: 读取最近 N 个会话的 transcript
   - **当前会话 (N=1)**: 无需读取 transcript — 你已有完整上下文，直接从记忆中分析
   - **历史会话 (N>1)**: 定位路径 `ls -lt ~/.claude/projects/-Users-*/*.jsonl | head -N`（项目目录名是 cwd 路径用 `-` 替换 `/`）
   - 按修改时间倒序取最近 N 个
   - 大文件 (>5MB) 只读首尾各 2000 行；小文件全量读取
   - 提取关键动作序列（命令调用、工具使用、手动操作）

3. **分析模式**: 对比会话中的实际操作 vs 现有自动化能力，识别:
   - **摩擦点**: 需要手动执行的重复步骤（应自动化）
   - **缺失命令**: 反复出现但没有对应 command 的操作模式
   - **命令增强**: 现有命令缺少的参数或步骤
   - **流程断点**: SDD 生命周期中需要手动衔接的环节
   - **文档过时**: CLAUDE.md / AGENTS.md / playbooks 与代码实际不符

4. **生成改进报告**: 按优先级（高/中/低）输出
   ```
   ## 改进建议

   ### 高优先级
   | # | 类型 | 描述 | 涉及文件 |
   |---|------|------|---------|
   | 1 | 命令增强 | /ship 需要 --merge 选项 | .claude/commands/ship.md |

   ### 中优先级
   ...

   ### 低优先级
   ...
   ```

5. **确认范围**: 用 AskUserQuestion 让用户选择要实施的改进项（多选）

6. **实施**: 对选中的改进项:
   - 修改/新增 command 文件
   - 更新 CLAUDE.md / AGENTS.md / playbooks
   - 更新 `.claude/settings.json`（如涉及权限或 hooks）

7. **提交**: 使用 `/ship --merge "chore: retro improvements — <summary>"` 提交

注意:
- 不要改动代码文件（只改工具链和文档）
- 保持 command 文件简洁（步骤描述而非实现细节）
- 新增 command 后同步更新 CLAUDE.md 的 Slash Commands 表
- 改进应基于实际痛点，不要过度设计
