mod app;
mod cli;

use clap::Parser;
use cli::{Cli, Command, RunArgs};

fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    let command = cli.command.unwrap_or(Command::Run(RunArgs::default()));

    match command {
        Command::Run(args) => cmd_run(args),
        Command::Stop(args) => cmd_stop(args),
        Command::Status(args) => cmd_status(args),
        Command::Reload(args) => cmd_reload(args),
    }
}

fn cmd_run(args: RunArgs) -> anyhow::Result<()> {
    // Daemonize before creating tokio runtime (unix only)
    #[cfg(unix)]
    if args.daemon {
        prism_core::lifecycle::daemon::daemonize()?;
    }

    // Load config early for logging decisions and pre-building shared deps
    let config = prism_core::config::Config::load(&args.config).unwrap_or_default();

    // Init logging — force file logging when running as daemon
    let to_file = args.daemon || config.logging_to_file;
    let log_dir = config.log_dir.clone();

    // Create request_logs and audit backend before logging init so they can be
    // shared with both the GatewayLogLayer and the Application.
    let request_logs = std::sync::Arc::new(prism_core::request_log::RequestLogStore::new(
        config.dashboard.request_log_capacity,
    ));
    let audit: std::sync::Arc<dyn prism_core::audit::AuditBackend> = if config.audit.enabled {
        match prism_core::audit::FileAuditBackend::new(config.audit.clone()) {
            Ok(backend) => std::sync::Arc::new(backend),
            Err(e) => {
                eprintln!("Failed to initialize audit backend: {e}, audit disabled");
                std::sync::Arc::new(prism_core::audit::NoopAuditBackend)
            }
        }
    } else {
        std::sync::Arc::new(prism_core::audit::NoopAuditBackend)
    };

    let gateway_layer =
        prism_server::telemetry::GatewayLogLayer::new(request_logs.clone(), audit.clone());

    let _guard = prism_core::lifecycle::logging::init_logging_with_layer(
        &args.log_level,
        to_file,
        log_dir.as_deref(),
        Box::new(gateway_layer),
    );

    // Build and run on a multi-thread runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        // Spawn audit cleanup task inside the tokio runtime
        if config.audit.enabled {
            prism_core::audit::FileAuditBackend::spawn_cleanup_task(config.audit.clone());
        }
        let application = app::Application::build(&args, request_logs, audit)?;
        application.serve().await
    })
}

#[cfg(unix)]
fn cmd_stop(args: cli::PidArgs) -> anyhow::Result<()> {
    use prism_core::lifecycle::pid_file::PidFile;

    let pid = PidFile::read_pid(&args.pid_file)?;
    if !PidFile::is_alive(pid) {
        println!("Process {pid} is not running.");
        return Ok(());
    }

    println!("Stopping PID {pid} (timeout {}s)...", args.timeout);
    PidFile::stop(pid, std::time::Duration::from_secs(args.timeout))?;
    println!("Stopped.");
    Ok(())
}

#[cfg(not(unix))]
fn cmd_stop(_args: cli::PidArgs) -> anyhow::Result<()> {
    anyhow::bail!("The 'stop' command is only supported on Unix systems");
}

#[cfg(unix)]
fn cmd_status(args: cli::PidArgs) -> anyhow::Result<()> {
    use prism_core::lifecycle::pid_file::PidFile;

    match PidFile::read_pid(&args.pid_file) {
        Ok(pid) => {
            if PidFile::is_alive(pid) {
                println!("prism is running (PID {pid})");
            } else {
                println!("prism is NOT running (stale PID file, PID {pid})");
            }
        }
        Err(_) => {
            println!("prism is NOT running (no PID file at {})", args.pid_file);
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn cmd_status(_args: cli::PidArgs) -> anyhow::Result<()> {
    anyhow::bail!("The 'status' command is only supported on Unix systems");
}

#[cfg(unix)]
fn cmd_reload(args: cli::PidArgs) -> anyhow::Result<()> {
    use prism_core::lifecycle::pid_file::PidFile;

    let pid = PidFile::read_pid(&args.pid_file)?;
    if !PidFile::is_alive(pid) {
        anyhow::bail!("Process {pid} is not running");
    }

    PidFile::send_signal(pid, libc::SIGHUP)?;
    println!("Sent SIGHUP to PID {pid}");
    Ok(())
}

#[cfg(not(unix))]
fn cmd_reload(_args: cli::PidArgs) -> anyhow::Result<()> {
    anyhow::bail!("The 'reload' command is only supported on Unix systems");
}
