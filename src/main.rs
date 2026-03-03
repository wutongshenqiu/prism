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

    // Init logging — force file logging when running as daemon
    let to_file = args.daemon || {
        // Peek at config to check logging_to_file
        prism_core::config::Config::load(&args.config)
            .map(|c| c.logging_to_file)
            .unwrap_or(false)
    };
    let log_dir = prism_core::config::Config::load(&args.config)
        .ok()
        .and_then(|c| c.log_dir.clone());
    let _guard =
        prism_core::lifecycle::logging::init_logging(&args.log_level, to_file, log_dir.as_deref());

    // Build and run on a multi-thread runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        let application = app::Application::build(&args)?;
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
