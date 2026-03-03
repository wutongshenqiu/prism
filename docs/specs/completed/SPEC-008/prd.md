# PRD: 支持 Daemon

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-008       |
| Title     | 支持 Daemon    |
| Author    |                |
| Status    | Active         |
| Created   | 2026-02-27     |
| Updated   | 2026-02-27     |

## Problem Statement

Prism 目前只能在前台运行，缺少生产环境裸机/VM 部署所需的 daemon 能力。main.rs 承担了 CLI 解析、日志初始化、组件构建、服务器启动、信号处理等所有职责，代码难以扩展。

本 spec 借鉴 Caddy（子命令驱动）和 Nginx（daemon + 信号）的设计理念，将 main.rs 重构为 **子命令架构 + Lifecycle trait 抽象**，将 daemon 作为其中一个自然的运行模式。

## Goals

- **G1**: 采用 clap 子命令架构（`run` / `stop` / `status` / `reload`），取代当前平铺的 flag 设计
- **G2**: 抽象 `Lifecycle` trait 统一进程生命周期管理（init → ready → reload → shutdown），daemon 和前台模式共享同一抽象
- **G3**: `run --daemon` 支持双 fork 后台运行，PID 文件带 flock 排他锁
- **G4**: 统一信号处理：SIGTERM/SIGINT → 优雅关停，SIGHUP → 配置重载
- **G5**: 文件日志（`tracing-appender` 滚动日志），daemon 模式自动启用，前台模式可选
- **G6**: systemd 集成：`sd-notify` 就绪/重载/停止通知 + ship unit file
- **G7**: 可配置优雅关停超时（默认 30s）

## Non-Goals

- master-worker 多进程模型
- Windows 服务 / macOS launchd 集成
- 进程内自动重启（由 systemd Restart=on-failure 处理）

## User Stories

- As a **用户**，I want `prism run` as the default command so that the current foreground behavior has a clear home.
- As a **用户**，I want `prism run --daemon` so that the proxy forks to background.
- As a **用户**，I want `prism stop` to gracefully stop a running daemon.
- As a **用户**，I want `prism status` to check if the daemon is alive.
- As a **用户**，I want `prism reload` to trigger a config reload via SIGHUP.
- As a **运维**，I want `systemctl start/stop/reload/status prism` to just work.
- As a **开发者**，I want the Lifecycle trait so I can add new lifecycle hooks without touching main.rs.

## Success Metrics

- 子命令 `run`/`stop`/`status`/`reload` 全部正常工作
- daemon 模式 PID 文件正确创建、锁定、清理
- SIGHUP 触发配置重载（与现有 file-watch 互补）
- daemon 模式日志写入文件并按天轮转
- systemd `Type=notify` 下 `systemctl start` 阻塞至就绪
- 关停超时后强制退出
- main.rs 职责清晰，<50 行

## Constraints

- **C1**: 双 fork 必须在 tokio runtime 之前（多线程 fork = UB）
- **C2**: `sd-notify` 在非 systemd 环境下为 no-op
- **C3**: PID 文件用 flock 排他锁，非文件存在判断
- **C4**: daemon 功能 `#[cfg(unix)]`，保持跨平台编译

## Open Questions

- [ ] PID 文件默认路径：`/var/run/prism.pid` vs `./prism.pid`
- [ ] 日志轮转策略：按天 vs 按大小
- [ ] 是否绑定 SIGUSR1/SIGUSR2

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| CLI 结构 | flags (`--daemon`/`--stop`) / subcommands (`run`/`stop`) | 子命令 | 更清晰、可扩展，每个命令独立参数空间 |
| 进程管理抽象 | 无抽象 / Lifecycle trait | Lifecycle trait | 统一 init/ready/reload/shutdown 语义，daemon 和前台复用 |
| Daemon 实现 | `daemonize` / `fork` / `nix` | `fork` | 最小依赖，API 简洁 |
| systemd | `libsystemd-rs` / `sd-notify` | `sd-notify` | 纯 Rust 零依赖 |
| PID 文件 | `pidfile-rs` / 手动 | 手动（libc flock） | 逻辑简单，避免多余依赖 |
| 文件日志 | `flexi_logger` / `tracing-appender` | `tracing-appender` | tracing 生态原生 |
