use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use workbench::config::Config;

#[derive(Parser)]
#[command(
    name = "workbench",
    version,
    about = "Local Lovable-style site builder daemon"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the workbench daemon.
    Serve {
        /// Port to bind on 127.0.0.1.
        #[arg(long, default_value_t = 7800)]
        port: u16,
        /// Hosted PWA origin allowed via CORS and the Host allowlist.
        #[arg(long)]
        origin: Option<String>,
        /// Directory holding projects.
        #[arg(long)]
        projects_dir: Option<PathBuf>,
        /// Disable auth (localhost-only use).
        #[arg(long)]
        no_auth: bool,
        /// Auth token (default: random 32-byte hex).
        #[arg(long)]
        token: Option<String>,
        /// Path to agents.json (default: ~/.config/workbench/agents.json).
        #[arg(long)]
        agents_config: Option<PathBuf>,
    },
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[tokio::main]
async fn main() -> ExitCode {
    let Command::Serve {
        port,
        origin,
        projects_dir,
        no_auth,
        token,
        agents_config,
    } = Cli::parse().command;
    let config = Config {
        port,
        origin,
        projects_dir: projects_dir.unwrap_or_else(|| home().join("workbench-projects")),
        no_auth,
        token: Config::resolve_token(no_auth, token),
        agents_config: Some(
            agents_config.unwrap_or_else(|| home().join(".config/workbench/agents.json")),
        ),
    };
    match workbench::serve(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("workbench: {e}");
            ExitCode::FAILURE
        }
    }
}
