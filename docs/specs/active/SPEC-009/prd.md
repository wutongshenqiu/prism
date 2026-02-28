# PRD: Dashboard Admin API & WebSocket

| Field     | Value                              |
|-----------|------------------------------------|
| Spec ID   | SPEC-009                           |
| Title     | Dashboard Admin API & WebSocket    |
| Author    | AI Proxy Team                      |
| Status    | Draft                              |
| Created   | 2026-02-28                         |
| Updated   | 2026-02-28                         |

## Problem Statement

AI Proxy Gateway 目前只有面向 AI 客户端的 API 和少量只读 Admin 端点（`/admin/config`、`/admin/metrics`、`/admin/models`）。运维人员无法通过 Web 界面管理 Provider、查看请求日志、修改配置或实时监控系统状态。需要一套完整的后端 Admin API 和 WebSocket 实时推送能力，作为 Web Dashboard 的数据基础。

## Goals

- **Dashboard 认证**: 独立于客户端 API 认证的 Dashboard 登录机制（JWT），保护管理端点
- **Provider 管理 API**: 对 Provider 配置的 CRUD 操作（列表/新增/编辑/删除 Provider 及其 API Key）
- **Auth Key 管理 API**: 对客户端 API Key 的 CRUD 操作
- **路由配置 API**: 查看和修改路由策略（round-robin / fill-first）
- **请求日志 API**: 存储近期请求日志并提供分页查询、按 Provider/Model/状态码过滤
- **WebSocket 实时推送**: 实时推送 Metrics 更新和请求日志流
- **配置校验与热重载触发**: 修改配置前校验合法性，应用后触发热重载
- **系统运维 API**: 健康状态详情、Daemon 控制（reload/stop）、日志文件查看

## Non-Goals

- 前端 UI 实现（由 SPEC-010 和 SPEC-011 覆盖）
- OAuth2 / OIDC / SSO 集成
- 多用户角色权限（RBAC），初期只区分 "已登录 / 未登录"
- 请求日志的持久化存储（初期使用内存 ring buffer，不引入数据库）
- Audit log（操作审计日志）

## User Stories

- As an operator, I want to authenticate to the Dashboard API so that management endpoints are protected from unauthorized access.
- As an operator, I want to list, add, edit, and delete providers via API so that I can manage upstream AI services without editing YAML files.
- As an operator, I want to manage client API keys via API so that I can grant or revoke access dynamically.
- As an operator, I want to query recent request logs with filters so that I can troubleshoot issues and understand traffic patterns.
- As an operator, I want to receive real-time metrics updates via WebSocket so that the Dashboard can show live data without polling.
- As an operator, I want to validate configuration changes before applying them so that a bad config does not break the proxy.
- As an operator, I want to trigger config hot-reload from the API so that changes take effect immediately.
- As an operator, I want to check detailed system health and control the daemon so that I can operate the proxy remotely.

## Success Metrics

- 所有管理端点在未认证时返回 401
- Provider/Auth Key CRUD 操作正确更新运行时配置并触发热重载
- 请求日志 ring buffer 支持至少 10,000 条记录，查询延迟 < 50ms
- WebSocket 连接稳定，Metrics 推送间隔 ≤ 1s
- 配置校验能拦截常见错误（缺少必填字段、无效 URL、重复 Key）

## Dependencies

- **SPEC-004** (Configuration System & Hot-Reload): 复用 ConfigWatcher 文件监听和热重载回调机制
- **SPEC-006** (Security & Authentication): 复用现有 CORS permissive 策略，Dashboard 认证与客户端 API 认证并行独立
- **SPEC-008** (Daemon Support): Daemon 控制 API (reload/stop) 复用现有 CLI 子命令的内部实现

## Constraints

- 所有管理 API 挂载在 `/api/dashboard/` 前缀下，与现有 `/v1/` 和 `/admin/` 路径互不干扰
- WebSocket 端点挂载在 `/ws/` 前缀下
- 新建 `dashboard_auth_middleware` (JWT)，与现有 `auth_middleware` (Bearer/x-api-key) 并行，各自保护不同路由组
- 请求日志使用内存 ring buffer（`VecDeque` + `RwLock`），不引入外部存储依赖
- CORS 已有 permissive 策略（SPEC-006），无需额外处理

## Configuration Extension

Dashboard 需在 `config.yaml` 新增 `dashboard` section：

```yaml
dashboard:
  enabled: true                    # 是否启用 Dashboard API
  username: "admin"                # 管理员用户名
  password_hash: "bcrypt:$2b$..." # bcrypt 哈希后的密码
  jwt_secret: "..."               # JWT 签名密钥（或从环境变量 DASHBOARD_JWT_SECRET 读取）
  jwt_ttl_secs: 3600              # Token 有效期，默认 1 小时
  request_log_capacity: 10000     # 请求日志 ring buffer 容量，默认 10000
```

## Config Write-Back Flow

配置修改通过写回 YAML 文件 + 触发 ConfigWatcher 重载实现：

1. Admin API 接收修改请求（如 `PATCH /api/dashboard/providers/{id}`）
2. 反序列化当前 config.yaml 为 Config struct
3. 应用修改并校验新配置的合法性
4. 使用 atomic write（tempfile + rename）写回 config.yaml，防止部分写入
5. ConfigWatcher（SPEC-004）检测到文件变更，通过 SHA256 去重后触发热重载回调
6. 运行时配置通过 `ArcSwap` 原子更新，立即生效
7. 并发写入采用 last-write-wins 策略（写操作受 `Mutex` 保护防止竞态）

> **注意**: 序列化回写会丢失 YAML 注释和自定义格式。

## Request Log Entry Schema

请求日志只记录代理请求（`/v1/*` 路径），不记录管理端点和健康检查：

```rust
pub struct RequestLogEntry {
    pub timestamp: i64,           // Unix timestamp (ms)
    pub request_id: String,       // 来自 RequestContext
    pub method: String,           // HTTP method
    pub path: String,             // 请求路径
    pub status: u16,              // 响应状态码
    pub latency_ms: u64,          // 请求耗时
    pub provider: Option<String>, // 路由到的 Provider
    pub model: Option<String>,    // 请求的模型
    pub input_tokens: Option<u64>,  // 输入 token 数
    pub output_tokens: Option<u64>, // 输出 token 数
    pub error: Option<String>,    // 错误信息（如有）
}
```

> **安全约束**: 不记录请求/响应 body，避免 PII 泄露。

## WebSocket Message Protocol

WebSocket 端点使用 JSON 消息信封格式，单一连接 `/ws/dashboard`：

```json
// 服务端 → 客户端：Metrics 推送（每秒）
{
  "type": "metrics",
  "data": { /* Metrics::snapshot() 输出 */ }
}

// 服务端 → 客户端：新请求日志
{
  "type": "request_log",
  "data": { /* RequestLogEntry */ }
}
```

客户端通过发送订阅消息控制接收内容：

```json
// 客户端 → 服务端：订阅控制
{
  "type": "subscribe",
  "channels": ["metrics", "request_log"]
}
```

## Error Handling

| 场景 | HTTP Status | 响应 |
|------|-------------|------|
| JWT 缺失或无效 | 401 Unauthorized | `{"error": "invalid_token", "message": "..."}` |
| JWT 过期 | 401 Unauthorized | `{"error": "token_expired", "message": "..."}` |
| 配置校验失败 | 422 Unprocessable Entity | `{"error": "validation_failed", "fields": [...]}` |
| Provider 不存在 | 404 Not Found | `{"error": "not_found", "message": "..."}` |
| 删除使用中的 Provider | 409 Conflict | `{"error": "in_use", "message": "..."}` |
| Ring buffer 已满 | N/A（自动淘汰最旧记录） | — |

## API Key Masking

Admin API 返回 Provider API Key 时一律脱敏：只返回前 4 位和后 4 位，中间用 `****` 替代。完整 Key 仅在创建时返回一次。客户端 API Key 同理。

## Open Questions

- [ ] Dashboard 认证是否需要支持多用户，还是单一管理员账号即可？
- [ ] 是否需要支持从环境变量覆盖 dashboard 配置项（如 `DASHBOARD_JWT_SECRET`）？

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| 认证方案 | Session Cookie, JWT, Basic Auth | JWT | 无状态，适合独立前端 SPA，不需要服务端 Session 存储 |
| 请求日志存储 | 内存 ring buffer, SQLite, 文件 | 内存 ring buffer | 零外部依赖，启动即用，适合初期；未来可扩展到持久化 |
| 配置修改方式 | 直接修改内存, 写回文件+重载 | 写回文件+重载 | 复用现有 ConfigWatcher 机制，保证配置持久化和一致性 |
| 配置写入策略 | 直接覆盖, atomic write (tempfile+rename) | atomic write | 防止部分写入导致配置损坏，ConfigWatcher SHA256 去重防止重复重载 |
| WebSocket 库 | axum 内置 (tokio-tungstenite), warp, actix-ws | axum 内置 | 与现有 Axum 框架一致，无需引入新的 Web 框架 |
| WebSocket 端点 | 多端点 (/ws/metrics, /ws/logs), 单端点+订阅 | 单端点+订阅 | 减少连接数，客户端按需订阅，扩展性更好 |
| API 前缀 | `/admin/`, `/api/`, `/api/dashboard/` | `/api/dashboard/` | 避免与现有 `/admin/` 只读端点冲突，语义清晰 |
| API Key 展示 | 完整返回, 完全隐藏, 脱敏返回 | 脱敏返回 (前4后4) | 平衡安全性和可辨识度 |
