//! `GET /api/setup/status`: what this machine still needs before the app is fully usable.

use std::ffi::{OsStr, OsString};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::ApiError;
use crate::providers;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    git: bool,
    gh: bool,
    github_signed_in: bool,
    providers: Vec<ProviderStatus>,
    projects_dir: PathBuf,
}

#[derive(Serialize)]
pub struct ProviderStatus {
    id: String,
    name: String,
    available: bool,
}

/// `GET /api/setup/status` → `{ git, gh, githubSignedIn, providers: [{ id, name, available }],
/// projectsDir }`. Programs are looked up on the search path; none of them is run.
pub async fn status(State(state): State<AppState>) -> Result<Json<SetupStatus>, ApiError> {
    let search = match state.config.search_path.clone() {
        Some(path) => path,
        None => default_search_path().await,
    };
    let found = |program: &str| on_path(program, &search);
    let providers = providers::load(state.config.agents_config.as_deref())?
        .into_iter()
        .map(|p| ProviderStatus {
            available: found(&p.command[0]),
            name: providers::display_name(&p.id).to_string(),
            id: p.id,
        })
        .collect();
    let github = super::github::status(State(state.clone())).await.0;
    Ok(Json(SetupStatus {
        git: found("git") && !is_macos_git_stub(&search),
        gh: found("gh"),
        github_signed_in: github.user.is_some(),
        providers,
        projects_dir: state.config.projects_dir.clone(),
    }))
}

/// The login shell's `PATH` (where terminals find the agents; an app opened from the Finder
/// only has launchd's minimal one), then the daemon's own.
async fn default_search_path() -> OsString {
    let login = crate::terminal::login_path().await.unwrap_or_default();
    let own = std::env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&login)
        .chain(std::env::split_paths(&own))
        .collect();
    std::env::join_paths(dirs).unwrap_or(own)
}

/// macOS ships `/usr/bin/git` even without git: it only offers to install the Command Line
/// Tools. It is real once those tools or Xcode are installed.
fn is_macos_git_stub(search: &OsStr) -> bool {
    let first = std::env::split_paths(search)
        .map(|dir| dir.join("git"))
        .find(|p| is_executable(p));
    cfg!(target_os = "macos")
        && first.is_some_and(|p| p == Path::new("/usr/bin/git"))
        && ![
            "/Library/Developer/CommandLineTools/usr/bin/git",
            "/Applications/Xcode.app/Contents/Developer/usr/bin/git",
        ]
        .iter()
        .any(|p| is_executable(Path::new(p)))
}

/// Whether `program` names an executable file: as given when it contains a `/`, else in one of
/// the `PATH`-style `search` directories.
fn on_path(program: &str, search: &OsStr) -> bool {
    let program = Path::new(program);
    if program.components().count() > 1 {
        return is_executable(program);
    }
    std::env::split_paths(search).any(|dir| is_executable(&dir.join(program)))
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
