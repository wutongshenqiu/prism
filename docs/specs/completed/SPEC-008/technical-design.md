# Technical Design: 支持 Daemon

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-008       |
| Title     | 支持 Daemon    |
| Author    |                |
| Status    | Completed      |
| Created   | 2026-02-27     |
| Updated   | 2026-02-27     |

## Overview

核心设计理念：**子命令架构 + Lifecycle 抽象 + 组合式启动管线**。

当前 main.rs 是一个 ~200 行的单体函数，混合了 CLI 解析、日志、组件构建、服务启动、信号处理。本设计将其拆分为：

1. **子命令层**（`run` / `stop` / `status` / `reload`）— 每个命令是独立的入口
2. **Lifecycle trait** — 统一 init → ready → reload → shutdown 语义
3. **组合式管线** — PidFile、SignalHandler、LogGuard 作为独立组件组合使用

重构后 main.rs 仅做 CLI dispatch，所有逻辑下沉到 `crates/core/src/lifecycle/` 模块。

## CLI Design

```
prism <COMMAND>

Commands:
  run       Start the proxy server (default)
  stop      Stop a running daemon
  status    Check daemon status
  reload    Send SIGHUP to reload configuration

Run Options:
  -c, --config <PATH>           Config file [default: config.yaml]
      --host <HOST>             Bind host override
      --port <PORT>             Bind port override
      --log-level <LEVEL>       Log level [default: info]
      --daemon                  Fork to background
      --pid-file <PATH>         PID file path [default: ./prism.pid]
      --shutdown-timeout <SECS> Graceful shutdown timeout [default: 30]

Stop/Status/Reload Options:
      --pid-file <PATH>         PID file path [default: ./prism.pid]
```

`prism` 不带子命令时等价于 `prism run`（通过 `#[command(default_subcommand)]` 实现，保持 `cargo run -- --config config.yaml` 可用）。

## Backend Implementation

### Module Structure

```
src/
└── main.rs                           # CLI dispatch（~30 行）

crates/core/src/
├── lib.rs                            # 导出新模块
├── lifecycle/
│   ├── mod.rs                        # Lifecycle trait + re-exports
│   ├── pid_file.rs                   # PID 文件管理（flock）
│   ├── signal.rs                     # 信号处理（SIGTERM/SIGINT/SIGHUP）
│   ├── daemon.rs                     # 双 fork daemonize
│   ├── logging.rs                    # tracing-appender 文件日志
│   └── notify.rs                     # sd-notify 包装
├── app.rs                            # [NEW] Application 组装：config → providers → router → serve
└── config.rs                         # 新增 DaemonConfig

dist/
└── prism.service                  # systemd unit file
```

### Key Types

#### Lifecycle Trait

```rust
// crates/core/src/lifecycle/mod.rs

/// 进程生命周期的统一抽象。
/// daemon 模式和前台模式共享同一套语义。
pub trait Lifecycle: Send + Sync {
    /// 进程就绪（listener 已绑定，可接受连接）
    fn on_ready(&self) {}
    /// 配置重载中
    fn on_reloading(&self) {}
    /// 配置重载完成
    fn on_reloaded(&self) {}
    /// 进程正在关停
    fn on_stopping(&self) {}
}
```

两个实现：

```rust
/// 前台模式 — 仅日志
pub struct ForegroundLifecycle;

impl Lifecycle for ForegroundLifecycle {
    fn on_ready(&self) {
        tracing::info!("Server ready");
    }
    // ... 其余 hook 仅 tracing::info
}

/// systemd 模式 — sd-notify + 日志
pub struct SystemdLifecycle;

impl Lifecycle for SystemdLifecycle {
    fn on_ready(&self) {
        tracing::info!("Server ready");
        let _ = sd_notify::notify(false, &[sd_notify::NotifyState::Ready]);
    }
    fn on_reloading(&self) {
        let _ = sd_notify::notify(false, &[sd_notify::NotifyState::Reloading]);
    }
    fn on_reloaded(&self) {
        let _ = sd_notify::notify(false, &[sd_notify::NotifyState::Ready]);
    }
    fn on_stopping(&self) {
        let _ = sd_notify::notify(false, &[sd_notify::NotifyState::Stopping]);
    }
}
```

运行时自动选择：检测 `NOTIFY_SOCKET` 环境变量 → 有则 `SystemdLifecycle`，否则 `ForegroundLifecycle`。

#### PidFile（RAII）

```rust
// crates/core/src/lifecycle/pid_file.rs

/// RAII PID 文件。创建时获取 flock，Drop 时释放并删除。
pub struct PidFile {
    path: PathBuf,
    _file: File,  // 持有 fd 保持 flock
}

impl PidFile {
    /// 创建并锁定。失败说明有其他实例运行。
    pub fn acquire(path: impl Into<PathBuf>) -> anyhow::Result<Self>;

    /// 读取已有 PID 文件中的 pid（用于 stop/status/reload）。
    pub fn read_pid(path: impl AsRef<Path>) -> anyhow::Result<u32>;

    /// 检查 pid 是否存活。
    pub fn is_alive(pid: u32) -> bool;

    /// 发送信号。
    pub fn send_signal(pid: u32, signal: libc::c_int) -> anyhow::Result<()>;

    /// 发送 SIGTERM 并等待进程退出，超时则 SIGKILL。
    pub fn stop(pid: u32, timeout: Duration) -> anyhow::Result<()>;
}

impl Drop for PidFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
```

#### SignalHandler

```rust
// crates/core/src/lifecycle/signal.rs

use tokio::sync::watch;
use tokio::signal::unix::{signal, SignalKind};

/// 信号到语义事件的映射。
pub enum SignalEvent {
    Shutdown,
    Reload,
}

pub struct SignalHandler {
    shutdown: watch::Sender<bool>,
}

impl SignalHandler {
    pub fn new() -> (Self, watch::Receiver<bool>);

    /// 阻塞监听信号，将 SIGHUP 映射为 reload_fn 调用，
    /// SIGTERM/SIGINT 映射为 shutdown 通知。
    pub async fn run(self, reload_fn: impl Fn() + Send + 'static);
}
```

#### Application（组装器）

```rust
// crates/core/src/app.rs

/// 将 main.rs 中散落的组装逻辑封装为结构化的 Application。
pub struct Application {
    config: Arc<ArcSwap<Config>>,
    router: Arc<CredentialRouter>,
    lifecycle: Box<dyn Lifecycle>,
    pid_file: Option<PidFile>,
    _log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
    _watcher: Option<ConfigWatcher>,
}

impl Application {
    /// Builder: 从 RunArgs 构建完整的 Application。
    pub fn build(args: &RunArgs) -> anyhow::Result<Self>;

    /// 启动服务器，阻塞直到 shutdown 信号。
    pub async fn serve(self) -> anyhow::Result<()>;
}
```

### Flow

#### main.rs（重构后）

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "prism", version, about = "Prism — AI API Proxy Gateway")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the proxy server (default)
    Run(RunArgs),
    /// Stop a running daemon
    Stop(PidArgs),
    /// Check daemon status
    Status(PidArgs),
    /// Reload configuration (send SIGHUP)
    Reload(PidArgs),
}

fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Run(RunArgs::default())) {
        Command::Run(args) => cmd_run(args),
        Command::Stop(args) => cmd_stop(args),
        Command::Status(args) => cmd_status(args),
        Command::Reload(args) => cmd_reload(args),
    }
}
```

注意：`main()` 不是 `async fn`，也没有 `#[tokio::main]`。`cmd_run()` 内部手动构建 tokio Runtime，确保 daemon fork 发生在 Runtime 创建之前。

#### cmd_run 流程

```
fn cmd_run(args: RunArgs) -> anyhow::Result<()>
│
├── 1. if args.daemon → daemon::daemonize()    // 双 fork，仍在 sync 上下文
│
├── 2. logging::init(&args)                    // daemon=true 时强制 to_file
│      返回 Option<WorkerGuard>
│
├── 3. tokio::runtime::Builder::new_multi_thread()
│      .enable_all().build()?                  // 手动创建 Runtime
│
└── 4. runtime.block_on(async {
       ├── Application::build(&args)?          // config + providers + router
       │   ├── PidFile::acquire()              // 如果 daemon 模式
       │   ├── detect lifecycle (systemd/foreground)
       │   ├── Config::load()
       │   ├── build_registry / CredentialRouter / translators
       │   ├── ConfigWatcher::start()
       │   └── build_router(AppState)
       │
       └── app.serve().await                   // bind + serve + signal loop
           ├── lifecycle.on_ready()
           ├── SignalHandler::run(reload_fn)   // 阻塞直到 shutdown
           ├── lifecycle.on_stopping()
           ├── timeout(shutdown_timeout, drain)
           └── PidFile::drop()                 // RAII 清理
   })
```

#### cmd_stop / cmd_status / cmd_reload

```rust
fn cmd_stop(args: PidArgs) -> anyhow::Result<()> {
    let pid = PidFile::read_pid(&args.pid_file)?;
    PidFile::stop(pid, Duration::from_secs(args.timeout.unwrap_or(30)))
}

fn cmd_status(args: PidArgs) -> anyhow::Result<()> {
    let pid = PidFile::read_pid(&args.pid_file)?;
    if PidFile::is_alive(pid) {
        println!("prism is running (pid: {pid})");
    } else {
        println!("prism is not running (stale pid file)");
    }
    Ok(())
}

fn cmd_reload(args: PidArgs) -> anyhow::Result<()> {
    let pid = PidFile::read_pid(&args.pid_file)?;
    PidFile::send_signal(pid, libc::SIGHUP)?;
    println!("Reload signal sent to pid {pid}");
    Ok(())
}
```

## Configuration Changes

### Config 新增字段

```rust
// config.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct DaemonConfig {
    /// PID 文件路径
    pub pid_file: String,
    /// 优雅关停超时（秒）
    pub shutdown_timeout: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            pid_file: "./prism.pid".to_string(),
            shutdown_timeout: 30,
        }
    }
}
```

Config 结构体新增 `pub daemon: DaemonConfig`。

### config.yaml 示例

```yaml
# Daemon 配置（可选，CLI 参数可覆盖）
daemon:
  pid-file: "./prism.pid"
  shutdown-timeout: 30
```

### systemd Unit File

```ini
# dist/prism.service
[Unit]
Description=Prism
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
ExecStart=/usr/local/bin/prism run --config /etc/prism/config.yaml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5
WatchdogSec=60

ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

## Provider Compatibility

不影响 provider 层。

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | N/A       | 无影响 |
| Claude   | N/A       | 无影响 |
| Gemini   | N/A       | 无影响 |

## Alternative Approaches

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| 保持 flag 模式 (`--daemon`/`--stop`) | 向后兼容 | 参数空间混乱，扩展困难 | 放弃 |
| Application trait 而非 struct | 更抽象 | 仅一个实现，过度设计 | 放弃 |
| 每个子命令一个 bin target | 隔离彻底 | 多 binary 分发复杂 | 放弃 |
| Lifecycle 用 enum 替代 trait | 无 vtable 开销 | 不可扩展（用户无法自定义） | 放弃 |

## Task Breakdown

- [ ] T1: 新增 `crates/core/src/lifecycle/` 模块 — `mod.rs`（Lifecycle trait + 实现）
- [ ] T2: 新增 `lifecycle/pid_file.rs` — PidFile RAII（acquire/read_pid/is_alive/send_signal/stop）
- [ ] T3: 新增 `lifecycle/daemon.rs` — daemonize() 双 fork
- [ ] T4: 新增 `lifecycle/signal.rs` — SignalHandler（SIGTERM/SIGINT/SIGHUP）
- [ ] T5: 新增 `lifecycle/logging.rs` — init_logging（console / file + non-blocking）
- [ ] T6: 新增 `lifecycle/notify.rs` — sd-notify 薄包装
- [ ] T7: 修改 `config.rs` — 新增 DaemonConfig
- [ ] T8: 新增 `app.rs` — Application struct（build + serve）
- [ ] T9: 重写 `src/main.rs` — 子命令 dispatch + cmd_run/stop/status/reload
- [ ] T10: 新增依赖：`fork`, `sd-notify`, `tracing-appender`, `libc`
- [ ] T11: 新增 `dist/prism.service`
- [ ] T12: 单元测试（PidFile、DaemonConfig 序列化、Lifecycle 实现）
- [ ] T13: 集成测试（daemon 启停、SIGHUP reload、重复启动检测）
- [ ] T14: 更新 config.example.yaml、AGENTS.md

## Test Strategy

- **Unit tests:**
  - PidFile: acquire/read/is_alive/Drop 清理、并发锁检测
  - DaemonConfig: 默认值、YAML round-trip
  - Lifecycle 实现: on_ready/on_stopping 调用验证
  - SignalHandler: 构造 + shutdown_receiver 通道

- **Integration tests:**
  - `prism run --daemon` → 后台运行 + PID 文件
  - `prism status` → 正确报告
  - `prism reload` → SIGHUP 重载日志
  - `prism stop` → 优雅关停 + PID 文件清理
  - 重复 `prism run --daemon` → flock 冲突错误
  - shutdown-timeout 超时强制退出

- **Manual verification:**
  - systemd: `systemctl start/stop/reload/status prism`
  - Docker: `prism run`（前台模式无回归）

## Rollout Plan

1. **Phase 1 — Lifecycle 基础**（T1-T6）: lifecycle 模块全部实现
2. **Phase 2 — 组装层**（T7-T9）: Application + main.rs 重写 + Config
3. **Phase 3 — 分发**（T10-T11）: 依赖 + systemd unit
4. **Phase 4 — 验证**（T12-T14）: 测试 + 文档
