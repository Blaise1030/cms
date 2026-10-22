use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use orso::config::Config;

#[derive(Parser)]
#[command(
    name = "orso",
    version,
    about = "Local Lovable-style site builder daemon"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the orso daemon.
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
        /// Auth token, hex only (default: random 32-byte hex).
        #[arg(long, value_parser = orso::config::parse_token)]
        token: Option<String>,
        /// Path to agents.json (default: ~/.config/orso/agents.json).
        #[arg(long)]
        agents_config: Option<PathBuf>,
        /// GitHub OAuth App client id (enables "Sign in with GitHub").
        #[arg(long, env = "ORSO_GITHUB_CLIENT_ID")]
        github_client_id: Option<String>,
        /// GitHub REST API base (tests/e2e: stub server).
        #[arg(long, env = "ORSO_GITHUB_API_URL")]
        github_api_url: Option<String>,
        /// GitHub web base for OAuth device flow (defaults to [`github_api_url`] when set).
        #[arg(long, env = "ORSO_GITHUB_WEB_URL")]
        github_web_url: Option<String>,
        /// Base for clone URLs: `{git_url}/{owner}/{repo}.git` (e2e: local bare repo root).
        #[arg(long, env = "ORSO_GITHUB_GIT_URL")]
        github_git_url: Option<String>,
        /// Base URL to print in the ready link (e.g. the Vite dev server in `just dev`).
        #[arg(long, env = "ORSO_UI_URL")]
        ui_url: Option<String>,
    },
    /// Called by coding-CLI hooks (internal).
    #[command(hide = true)]
    Hook {
        #[command(subcommand)]
        event: HookEvent,
    },
    /// App state for the agent in a orso terminal (prints JSON).
    Ctl {
        #[command(subcommand)]
        command: orso::ctl::CtlCommand,
    },
    /// Agent-only git launcher (internal): `orso git-shim -- <git argv>`.
    #[command(hide = true)]
    GitShim {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum HookEvent {
    /// A coding-CLI turn ended.
    TurnEnd {
        /// Hook payload JSON (default: read from stdin).
        payload: Option<String>,
    },
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[tokio::main]
async fn main() -> ExitCode {
    let command = Cli::parse().command;
    let Command::Serve {
        port,
        origin,
        projects_dir,
        no_auth,
        token,
        agents_config,
        github_client_id,
        github_api_url,
        github_web_url,
        github_git_url,
        ui_url,
    } = command
    else {
        return match command {
            Command::Hook {
                event: HookEvent::TurnEnd { payload },
            } => orso::hook::turn_end(payload).await,
            Command::Ctl { command } => orso::ctl::run(command).await,
            Command::GitShim { args } => orso::git_shim::run(args),
            Command::Serve { .. } => unreachable!(),
        };
    };
    let github_defaults = orso::config::GithubConfig::default();
    let api_url = github_api_url.unwrap_or_else(|| github_defaults.api_url.clone());
    let web_url = github_web_url.unwrap_or_else(|| api_url.clone());
    let git_url = github_git_url.unwrap_or_else(|| github_defaults.git_url.clone());
    let config = Config {
        port,
        origin,
        projects_dir: projects_dir.unwrap_or_else(|| home().join("orso-projects")),
        no_auth,
        token: Config::resolve_token(no_auth, token),
        agents_config: Some(
            agents_config.unwrap_or_else(|| home().join(".config/orso/agents.json")),
        ),
        github: orso::config::GithubConfig {
            client_id: github_client_id,
            api_url,
            web_url,
            git_url,
            gh_path: orso::config::resolve_gh(std::path::Path::new("gh")),
            ..github_defaults
        },
        ui_url,
        hook_exe: None,
        search_path: None,
    };
    match orso::serve(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("orso: {e}");
            ExitCode::FAILURE
        }
    }
}
