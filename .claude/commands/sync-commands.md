同步三套命令定义。Argument $ARGUMENTS: `[command-name | all]`（default: all）。

用法:
- `/sync-commands` — 全量同步所有命令到 skills 和 opencode
- `/sync-commands test` — 只同步 test 命令

背景:
项目有三套命令定义，服务不同的 agent 工具:
1. `.claude/commands/*.md` — Claude Code 原生命令（SSOT，权威来源）
2. `.agents/skills/*/SKILL.md` — 可移植 skills（Codex/OpenCode 发现）
3. `.opencode/commands/*.md` — OpenCode 原生命令（支持 RUN 指令）

Claude Code 命令是**权威来源**，其他两套从它派生。

Steps:

1. **确定范围**: 解析 `$ARGUMENTS`
   - `all`: 同步所有命令（排除 `implement.md` 和 `retro.md`，它们是 Claude Code 专有）
   - 指定名称: 只同步该命令

2. **对比差异**: 对每个待同步命令:
   - 读取 `.claude/commands/<name>.md`（权威来源）
   - 读取 `.agents/skills/<name>/SKILL.md`（如存在）
   - 读取 `.opencode/commands/<name>.md`（如存在）
   - 对比核心逻辑是否一致（忽略格式差异）

3. **生成更新**: 对有差异的命令:
   - **SKILL.md 格式**: 添加 YAML front matter (`name`, `description`)，将 `$ARGUMENTS` 替换为描述性文本，翻译为英文
   - **OpenCode 格式**: 添加 YAML front matter (`description`)，将 `$ARGUMENTS` 替换为 `$1`/`$2` 位置参数，对 review/deps 添加 `RUN` 预加载指令

4. **写入文件**: 更新有差异的 skill 和 opencode 命令文件

5. **报告**: 输出同步结果表
   | 命令 | SKILL.md | OpenCode | 操作 |
   |------|----------|----------|------|

不同步的命令（Claude Code 专有）:
- `implement` — 依赖 TaskCreate/TaskUpdate
- `retro` — 依赖 session transcripts
- `sync-commands` — 本命令自身
