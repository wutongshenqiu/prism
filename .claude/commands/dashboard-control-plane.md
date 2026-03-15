整理和收口 Prism dashboard/control-plane 的产品正确性问题。Argument $ARGUMENTS: 问题描述或目标（可选）。

适用场景:
- “dashboard truth”
- “config workspace 不可信/容易误导”
- “request logs / websocket 行为不对”
- “Models / Protocols / System 页面状态应该来自 runtime truth”
- “需要真实 Playwright 覆盖，不要只做 mocked 测试”
- “几张相关 issue 其实应该合并成一个产品改动”

Steps:
1. 先按产品面重组问题，而不是照搬历史 issue 切分:
   - Config mutation truth
   - Runtime truth semantics
   - Realtime logs / websocket productization
   - Browser contract coverage
2. 从 backend truth 开始:
   - `crates/server/src/handler/dashboard/`
   - `crates/server/tests/dashboard_tests.rs`
3. 再检查匹配的 frontend 面:
   - `web/src/pages/`
   - `web/src/stores/`
   - `web/src/services/`
   - `web/e2e/`
4. 做实现时遵循这些规则:
   - 配置写路径只能有一条共享事务路径
   - UI badge / 状态必须来自 backend/runtime truth，不要前端硬编码绿勾
   - live logs 插入必须尊重 filters 和 page 语义
   - websocket 连接状态、重连、token refresh 要在 UI 可见
   - 能删掉误导性的状态就不要保留“猜出来”的语义
5. 默认验证包:
   - `make lint`
   - `make test`
   - `cd web && npm run test`
   - `cd web && npm run build`
   - `cd web && npm run test:e2e`
6. 只有用户明确要求真实机器/远程验证时，才加 remote/live 验证:
   - 构建当前分支，而不是借用旧服务
   - 在远端起隔离实例
   - 本地 Playwright 通过 tunnel 打远端 backend
7. 如果几个 issue 本质上已经收敛成一个产品改动:
   - 先解产品问题，再映射回 issues/spec
   - 只有完整收口时才在 PR 里写 closing keywords
   - merge 后核查 issue 是否真的关闭
