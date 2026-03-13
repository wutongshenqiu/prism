//! CLI argument parsing with subcommand architecture.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "prism", version, about = "Prism — AI API Proxy Gateway")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run the proxy server (default when no subcommand is given)
    Run(RunArgs),
    /// Stop a running daemon
    Stop(PidArgs),
    /// Check status of a running daemon
    Status(PidArgs),
    /// Send SIGHUP to reload configuration
    Reload(PidArgs),
    /// Generate a bcrypt password hash for dashboard config
    HashPassword(HashPasswordArgs),
}

#[derive(Parser, Debug)]
pub struct HashPasswordArgs {
    /// Password to hash (reads from stdin if not provided)
    #[arg(long)]
    pub password: Option<String>,

    /// bcrypt cost factor (default: 12)
    #[arg(long, default_value = "12")]
    pub cost: u32,
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Path to config file
    #[arg(short, long, default_value = "config.yaml", env = "PRISM_CONFIG")]
    pub config: String,

    /// Listen host
    #[arg(long, env = "PRISM_HOST")]
    pub host: Option<String>,

    /// Listen port
    #[arg(long, env = "PRISM_PORT")]
    pub port: Option<u16>,

    /// Log level
    #[arg(long, default_value = "info", env = "PRISM_LOG_LEVEL")]
    pub log_level: String,

    /// Run as a background daemon (unix only)
    #[arg(long)]
    pub daemon: bool,

    /// Path to PID file (overrides config)
    #[arg(long)]
    pub pid_file: Option<String>,

    /// Graceful shutdown timeout in seconds (overrides config)
    #[arg(long)]
    pub shutdown_timeout: Option<u64>,
}

impl Default for RunArgs {
    fn default() -> Self {
        Self {
            config: "config.yaml".to_string(),
            host: None,
            port: None,
            log_level: "info".to_string(),
            daemon: false,
            pid_file: None,
            shutdown_timeout: None,
        }
    }
}

impl From<RunArgs> for prism_server::app::RunConfig {
    fn from(args: RunArgs) -> Self {
        Self {
            config_path: args.config,
            host: args.host,
            port: args.port,
            log_level: args.log_level,
            daemon: args.daemon,
            pid_file: args.pid_file,
            shutdown_timeout: args.shutdown_timeout,
        }
    }
}

#[derive(Parser, Debug)]
pub struct PidArgs {
    /// Path to PID file
    #[arg(long, default_value = "./prism.pid")]
    pub pid_file: String,

    /// Timeout in seconds for stop operation
    #[arg(long, default_value = "30")]
    pub timeout: u64,
}
