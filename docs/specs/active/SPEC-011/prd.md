# PRD: Web Dashboard - Configuration & Operations

| Field     | Value                                        |
|-----------|----------------------------------------------|
| Spec ID   | SPEC-011                                     |
| Title     | Web Dashboard - Configuration & Operations   |
| Author    | AI Proxy Team                                |
| Status    | Draft                                        |
| Created   | 2026-02-28                                   |
| Updated   | 2026-02-28                                   |

## Problem Statement

AI Proxy Gateway 的配置管理完全依赖手动编辑 YAML 文件和 CLI 命令。运维人员需要 SSH 到服务器、编辑配置文件、手动触发重载才能修改 Provider 配置或管理 API Key。缺乏直观的 Web 界面来执行配置变更和系统运维操作，增加了操作复杂度和出错风险。

## Goals

- **Provider 管理界面**: 列表展示所有 Provider，支持新增、编辑、删除 Provider 及其 API Key 配置
- **API Key 管理界面**: 列表展示客户端 API Key，支持新增、删除、启用/禁用
- **路由配置界面**: 查看和修改路由策略（round-robin / fill-first），直观展示当前路由状态
- **配置校验与预览**: 修改配置前展示 diff 预览，校验通过后一键应用
- **热重载控制**: 从 Dashboard 触发配置热重载，展示重载状态和结果
- **系统健康详情**: 展示系统运行时信息（uptime、版本、Daemon 状态、资源使用）
- **日志查看器**: 查看近期运行日志，支持按级别过滤和搜索

## Non-Goals

- 配置版本管理和回滚（初期不做 Git-like 的版本控制）
- 多实例集群管理（只管理单个 Proxy 实例）
- Provider API Key 的加密存储（复用现有配置文件机制）
- 定时任务和自动化策略
- 告警规则配置和通知

## User Stories

- As an operator, I want to add a new Provider through the web UI so that I don't need to SSH into the server and edit YAML.
- As an operator, I want to edit Provider settings (API key, base URL, model list) in the web UI so that I can quickly adjust upstream configurations.
- As an operator, I want to delete a Provider through the web UI so that I can remove deprecated services cleanly.
- As an operator, I want to manage client API keys (create, revoke) through the web UI so that I can control access without file editing.
- As an operator, I want to see a diff preview before applying configuration changes so that I can verify changes are correct.
- As an operator, I want to trigger config reload from the Dashboard so that changes take effect without CLI access.
- As an operator, I want to view system health and daemon status from the Dashboard so that I can monitor the proxy remotely.
- As an operator, I want to view and search application logs from the Dashboard so that I can troubleshoot without accessing log files directly.

## Success Metrics

- Provider CRUD 操作从 Web UI 执行到配置生效 < 3s（含校验和热重载）
- 配置校验能拦截 95% 的常见配置错误（缺少必填字段、无效 URL、格式错误等）
- Diff 预览准确反映所有即将应用的变更
- 日志查看器支持 10,000+ 行日志的流畅滚动和过滤

## Dependencies

- **SPEC-009** (Dashboard Admin API & WebSocket): 所有修改操作调用 SPEC-009 的 Admin API，API Key 脱敏规则遵循 SPEC-009 定义（前4后4）
- **SPEC-010** (Web Dashboard - Monitoring): 复用前端项目脚手架、共享组件、WebSocket 连接管理和认证流程

## Constraints

- 复用 SPEC-010 建立的前端项目结构和技术栈
- 所有修改操作调用 SPEC-009 的 `/api/dashboard/*` 端点，不直接操作后端
- 配置修改必须经过校验 → Diff 预览 → 确认 三步流程
- Provider API Key 在 UI 中脱敏显示，遵循 SPEC-009 API 返回的脱敏格式
- 日志查看器通过分页 API 加载历史日志，可选通过 `/ws/dashboard` 实时跟踪新日志

## Open Questions

- [ ] 配置修改是否需要二次确认（例如输入 Dashboard 密码确认）？
- [ ] 是否需要 "导入/导出配置" 功能（上传/下载 YAML 文件）？
- [ ] 日志查看器是实时流式还是分页加载？

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| 配置修改流程 | 直接保存, 预览后保存, 预览+确认后保存 | 预览+确认后保存 | 防止误操作，配置变更影响全局，需要谨慎 |
| API Key 显示 | 完整显示, 完全隐藏, 脱敏显示 | 脱敏显示 (前4后4) | 平衡安全性和可辨识度，方便运维人员区分不同 Key |
| 表单方案 | 原生 form, React Hook Form, Formik | React Hook Form | 性能好，TypeScript 支持好，与 UI 组件库集成方便 |
| 日志加载方式 | 全量加载, 分页, WebSocket 流式 | 分页 + 可选 WebSocket 实时跟踪 | 分页确保大量日志不卡顿，WebSocket 实时跟踪满足 tail -f 场景 |
