审查文档与代码的一致性。Argument $ARGUMENTS: full/quick/types/api/specs（default: quick）。

支持 `--fix` 后缀自动修复发现的差异（例如 `/doc-audit full --fix`）。

范围说明:
- "quick": 仅检查 reference/types/ 类型定义 vs Rust 源码中的类型
- "full": reference/ 全量 + AGENTS.md + specs/completed/ 交叉检查 + 链接有效性
- "types": 逐一检查 docs/reference/types/ 下每个文件:
  - enums.md vs crates/core/src/provider.rs, config.rs, cloak.rs 中的枚举定义
  - config.md vs crates/core/src/config.rs 中的配置类型
  - provider.md vs crates/core/src/provider.rs + crates/provider/src/ 中的类型和 trait
  - errors.md vs crates/core/src/error.rs 中的 ProxyError 及 status_code 映射
- "api": API 端点一致性:
  - docs/reference/api-surface.md 端点表 vs crates/server/src/lib.rs 路由定义
  - 每个 handler 的实际参数、返回格式
- "agents": 检查 AGENTS.md 与代码的一致性:
  - Crate Responsibilities 描述 vs 实际 struct/trait/field 定义
  - API Endpoints 表 vs crates/server/src/lib.rs 路由定义
  - Provider Matrix 表 vs crates/provider/src/ executor 实现
- "specs": 每个 completed Spec 的 technical-design.md 与对应代码模块的关键声明对比

注意: "full" 模式会自动包含 agents 检查。

Steps:
1. 读取目标文档文件（含 AGENTS.md，如 full/agents 模式）
2. 读取对应的 Rust 源码文件
3. 逐项对比: 字段名、类型、枚举变体、方法签名、默认值、serde 属性
4. 输出差异表:

| 差异项 | 文档位置 | 代码位置 | 文档值 | 代码值 | 操作建议 |
|--------|----------|----------|--------|--------|----------|

5. 检查文档内链接有效性（仅 full 模式）
6. 汇总: 总差异数、按严重度分类（错误/遗漏/过时）

### --fix 模式（当 $ARGUMENTS 包含 `--fix` 时）

在完成上述审查并输出差异表后，自动修复所有发现的差异:

7. 对每个差异项，读取目标文档文件并应用修复:
   - **字段/枚举不匹配**: 用代码中的实际定义更新文档
   - **缺失条目**: 从代码中提取定义并补充到文档
   - **过时描述**: 根据当前代码行为重写
   - **断链**: 更新为正确的文件路径
   - **Spec 状态不匹配**: 更新 metadata 中的 Status 字段
8. 修复完成后，重新运行审查（相同范围，不含 --fix）以验证零差异
9. 输出修复摘要:
   | # | 文件 | 修复内容 | 状态 |
   |---|------|----------|------|
