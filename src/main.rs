mod cli;

use clap::Parser;
use cli::{Cli, Command, RunArgs};

fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    let command = cli.command.unwrap_or(Command::Run(RunArgs::default()));

    match command {
        Command::Run(args) => prism_server::app::run(args.into()),
        #[cfg(unix)]
        Command::Stop(args) => prism_lifecycle::pid_file::cmd_stop(&args.pid_file, args.timeout),
        #[cfg(not(unix))]
        Command::Stop(_) => anyhow::bail!("The 'stop' command is only supported on Unix systems"),
        #[cfg(unix)]
        Command::Status(args) => prism_lifecycle::pid_file::cmd_status(&args.pid_file),
        #[cfg(not(unix))]
        Command::Status(_) => {
            anyhow::bail!("The 'status' command is only supported on Unix systems")
        }
        #[cfg(unix)]
        Command::Reload(args) => prism_lifecycle::pid_file::cmd_reload(&args.pid_file),
        #[cfg(not(unix))]
        Command::Reload(_) => {
            anyhow::bail!("The 'reload' command is only supported on Unix systems")
        }
        Command::HashPassword(args) => cmd_hash_password(args),
    }
}

fn cmd_hash_password(args: cli::HashPasswordArgs) -> anyhow::Result<()> {
    let password = match args.password {
        Some(p) => p,
        None => {
            eprint!("Password: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    if password.is_empty() {
        anyhow::bail!("Password cannot be empty");
    }

    let hash = bcrypt::hash(&password, args.cost)?;
    // Ensure $2y$ prefix for compatibility with prism's bcrypt verifier
    let hash = hash.replacen("$2b$", "$2y$", 1);
    println!("{hash}");
    Ok(())
}
