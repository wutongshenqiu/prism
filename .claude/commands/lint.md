运行代码检查。Argument $ARGUMENTS: check/fix（default: check）。

模式:
- "check" — 仅检查，报告问题但不修改
- "fix" — 自动修复可修复的问题

Steps:

**check 模式:**
1. `cargo fmt --check` — 检查格式
2. `cargo clippy --workspace --tests -- -D warnings` — 检查 lint 规则
3. 汇总: 格式问题数 + clippy 警告数
4. 如有问题，列出每个问题的文件位置和修复建议

**fix 模式:**
1. `cargo fmt` — 自动格式化
2. `cargo clippy --workspace --tests -- -D warnings` — 检查 lint 规则
3. 如有 clippy 警告，尝试应用 `cargo clippy --fix --allow-dirty --workspace`
4. 再次运行 `cargo clippy --workspace --tests -- -D warnings` 确认全部通过
5. 汇总: 自动修复数 + 剩余需手动修复数
