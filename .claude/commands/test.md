运行测试。Argument $ARGUMENTS: all/unit/e2e/e2e-docker/e2e-docker-full/specific（default: all）。

模式:
- "all" — 运行全部测试: `cargo test --workspace`
- "unit" — 仅单元测试（不含集成测试）: `cargo test --workspace --lib`
- "e2e" — 运行 E2E 集成测试: `cargo test --test e2e -- --ignored`
- "e2e-docker" — 运行 Docker E2E 测试（quick 级别）: `make test-e2e-docker`
- "e2e-docker-full" — 运行 Docker E2E 全量测试: `TEST_LEVEL=full make test-e2e-docker`
- 其他值 — 作为测试过滤器: `cargo test --workspace $ARGUMENTS`

Steps:
1. Run `cargo check --workspace` — 先确保编译通过（e2e-docker 模式跳过此步）
2. 按模式执行测试
3. 如有失败:
   - 列出每个失败测试的名称和错误信息
   - 定位对应的源文件和测试文件
   - 分析失败原因（编译错误/断言失败/panic）
4. 汇总: 通过数 / 失败数 / 忽略数

注意:
- e2e-docker 和 e2e-docker-full 需要 `E2E_BAILIAN_API_KEY` 环境变量
- e2e-docker 支持 `TEST_FILTER` 环境变量过滤测试用例（如 `TEST_FILTER=cline`）

示例:
```
/test                    # 全部 cargo 测试
/test unit               # 仅单元测试
/test e2e-docker         # Docker E2E (quick)
/test e2e-docker-full    # Docker E2E (全量)
/test test_should_cloak  # 运行名称匹配的测试
/test cloak              # 运行 cloak 相关测试
```
