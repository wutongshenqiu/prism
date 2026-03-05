诊断并修复项目问题。Argument $ARGUMENTS: 问题描述。

Steps:
1. 分析问题描述，定位可能模块:
   - 路由/凭证问题 → crates/provider/src/routing.rs, crates/core/src/config.rs
   - 翻译错误 → crates/translator/src/ (对应的翻译器模块)
   - SSE/流式问题 → crates/provider/src/sse.rs, crates/server/src/streaming.rs
   - 配置/热重载 → crates/core/src/config.rs (ConfigWatcher)
   - 认证问题 → crates/server/src/auth.rs
   - Cloaking/Payload → crates/core/src/cloak.rs, crates/core/src/payload.rs
   - 请求分发/重试 → crates/server/src/dispatch/ (mod.rs, helpers.rs, streaming.rs, retry.rs)
   - Provider 执行 → crates/provider/src/ (对应的 executor)
2. 检查相关代码路径:
   - 读取可能涉及的源文件
   - 检查错误处理路径 (ProxyError 变体)
   - 检查 config.example.yaml 中的相关配置项
3. 尝试复现:
   - 检查相关测试是否覆盖该场景
   - 如有必要，构造 minimal 的测试用例
4. 定位 root cause，实施修复
5. Run `make test` 验证修复不引入新问题
6. Run `make lint` 确保代码规范
7. 汇总: 问题原因 → 修复内容 → 验证结果 → 是否需要更新文档
