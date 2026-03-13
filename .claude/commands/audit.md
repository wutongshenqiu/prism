全量代码审查+批量修复。Argument $ARGUMENTS: `[--fix] [scope]`（scope: all/security/stability/performance/refactor，default: all）。

用法:
- `/audit` — 审查全部代码，输出问题报告
- `/audit security` — 仅审查安全相关问题
- `/audit --fix` — 审查并自动修复所有问题
- `/audit --fix stability` — 仅修复稳定性问题

Steps:

1. **解析参数**: 提取 `--fix` 标志和 scope 过滤器

2. **并行审查**: 按 crate 分派审查任务（可并行）:
   - `prism-core` — 配置、错误处理、类型安全、panic sites
   - `prism-provider` — 执行器、SSE 解析、凭证路由、网络安全
   - `prism-translator` — 格式转换、流式状态、JSON 解析
   - `prism-server` — 中间件、dispatch、handler、认证
   - 跨 crate — 架构一致性、接口契约、依赖方向

   每个审查关注:
   - **安全**: unwrap/panic、缓冲区溢出、注入、认证绕过、敏感信息泄漏
   - **稳定性**: 错误吞没、锁 poison、TOCTOU、资源泄漏
   - **性能**: O(n²) 查找、不必要分配、热路径优化
   - **重构**: 代码重复、过长函数、类型安全改进

3. **汇总分类**: 按优先级整理发现:

   ## 审查报告

   ### P0 — Critical (安全)
   | # | 文件:行号 | 问题 | 修复建议 |

   ### P1 — High (稳定性)
   | # | 文件:行号 | 问题 | 修复建议 |

   ### P2 — Medium (性能/重构)
   | # | 文件:行号 | 问题 | 修复建议 |

4. **如果 `--fix`**: 批量修复
   a. 按优先级从高到低修复每个问题
   b. 每修完一批（同 crate 的改动）运行 `cargo check`
   c. 全部修完后运行 `make lint` + `make test`
   d. 如需创建 Spec（重构类改动），自动创建并关联

5. **创建 GitHub Issues**（可选，仅无 `--fix` 时）:
   - 询问用户是否要创建 issues
   - 按问题逐个 `gh issue create`，附标签和优先级
   - 不要生成 `docs/issues/` 文档——直接用 GitHub Issues 追踪

6. **结果报告**:
   - 发现问题总数（按优先级）
   - 修复数（如 `--fix`）
   - 剩余需手动处理的问题
   - 下一步建议（如 `/ship` 提交修复）

注意:
- 审计结果直接写入 GitHub Issues，不落地到 `docs/issues/` 目录
- 如果发现已有 `docs/issues/` 中的旧文档且对应 issues 已关闭，应清理这些文件
