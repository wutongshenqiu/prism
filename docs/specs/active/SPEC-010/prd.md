# PRD: Web Dashboard - Monitoring

| Field     | Value                          |
|-----------|--------------------------------|
| Spec ID   | SPEC-010                       |
| Title     | Web Dashboard - Monitoring     |
| Author    | AI Proxy Team                  |
| Status    | Draft                          |
| Created   | 2026-02-28                     |
| Updated   | 2026-02-28                     |

## Problem Statement

AI Proxy Gateway 缺少可视化的监控界面。运维人员只能通过 JSON API (`/admin/metrics`) 或命令行查看系统状态，无法直观了解请求趋势、Provider 负载分布、错误率等关键指标。需要一个实时监控 Dashboard 提供直观的可视化能力。

## Goals

- **前端项目脚手架**: 建立独立的前端项目（React + TypeScript + Vite），配置开发代理、构建流程和项目结构
- **概览仪表盘**: 展示核心指标（总请求数、错误率、Token 用量、活跃 Provider 数）的实时摘要卡片
- **指标可视化**: 请求量趋势图、延迟分布直方图、Per-Provider 和 Per-Model 的请求分布饼图/柱状图
- **请求日志查看器**: 分页列表展示近期请求，支持按 Provider、Model、状态码、时间范围过滤和搜索
- **Provider 健康状态**: 展示每个 Provider 的连接状态、最近错误、平均延迟
- **WebSocket 实时更新**: 通过 WebSocket 接收实时数据，Dashboard 无需轮询即可自动更新

## Non-Goals

- 配置修改功能（由 SPEC-011 覆盖）
- 系统运维操作（由 SPEC-011 覆盖）
- 移动端适配（初期只支持桌面浏览器）
- 自定义 Dashboard 布局（固定布局）
- 历史数据持久化和长期趋势分析

## User Stories

- As an operator, I want to see a real-time overview of proxy health so that I can quickly assess system status at a glance.
- As an operator, I want to view request volume trends over time so that I can understand traffic patterns and capacity needs.
- As an operator, I want to see latency distribution so that I can identify performance bottlenecks.
- As an operator, I want to filter and search request logs so that I can troubleshoot specific issues quickly.
- As an operator, I want to see per-provider health status so that I can identify which upstream services have problems.
- As an operator, I want the dashboard to update in real-time so that I always see the latest data without manual refresh.

## Success Metrics

- Dashboard 首次加载时间 < 3s（gzip 后静态资源 < 500KB）
- WebSocket 连接断开后自动重连，恢复时间 < 5s
- 指标图表在 10,000+ 数据点下仍流畅渲染（60fps）
- 请求日志列表支持 10,000 条记录的流畅滚动和过滤

## Dependencies

- **SPEC-009** (Dashboard Admin API & WebSocket): 所有数据来源于 SPEC-009 的 REST API 和 WebSocket 端点

## Constraints

- 前端项目位于仓库根目录 `web/` 下，独立 `package.json`
- 使用 React 18+ / TypeScript 5+ / Vite 作为技术栈
- 图表库使用 Recharts 或 ECharts（轻量且与 React 集成良好）
- UI 组件库使用 Ant Design 或 shadcn/ui + Tailwind CSS
- 开发模式通过 Vite proxy 转发 `/api/dashboard/*` 和 `/ws/*` 请求到本地 Axum 服务
- 生产构建输出到 `web/dist/`，可通过 Nginx 部署或嵌入 Rust 二进制
- WebSocket 连接地址：`/ws/dashboard`，消息协议遵循 SPEC-009 定义的 JSON 信封格式
- 请求日志数据来源于 SPEC-009 的内存 ring buffer（`RequestLogEntry` schema）

## Open Questions

- [ ] 图表库选择：Recharts（React 原生）vs ECharts（功能更强大）？
- [ ] UI 组件库选择：Ant Design（开箱即用）vs shadcn/ui + Tailwind（更灵活轻量）？
- [ ] 是否需要暗色主题支持？
- [ ] 指标数据的时间窗口：最近 1 小时 / 6 小时 / 24 小时 / 自定义？

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| 前端框架 | React, Vue 3, Svelte | React | 生态最成熟，社区最大，组件库选择多 |
| 构建工具 | Vite, Webpack, Rspack | Vite | 开发体验好，HMR 快，配置简洁 |
| 项目位置 | monorepo (web/), 独立仓库, crates/内 | monorepo (web/) | 与后端同仓库方便联调，CI/CD 统一管理 |
| 状态管理 | Redux, Zustand, React Context | Zustand | 轻量，TypeScript 友好，适合中等规模应用 |
| 实时数据 | 轮询, WebSocket, SSE | WebSocket | 双向通信，延迟低，SPEC-009 已提供 WebSocket 端点 |
