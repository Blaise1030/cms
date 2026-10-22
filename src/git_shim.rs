//! Hard-block agent `git checkout`/`switch`: only the app may switch drafts.
//!
//! Agents (chat and terminal) run with a daemon-generated `git` launcher first on their
//! `PATH`; the launcher execs this binary's `git-shim` subcommand, which refuses any
//! HEAD-moving checkout/switch unless the app set [`ALLOW_ENV`]. Allowed commands are
//! `exec`'d into the real git ([`REAL_GIT_ENV`], else the next `git` on `PATH`) so exit
//! codes, stdio and argv pass through faithfully.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use crate::config::Config;

/// Set to `1` only on the app's own checkout, permitting that single HEAD move.
pub const ALLOW_ENV: &str = "ORSO_ALLOW_CHECKOUT";
/// The real git to exec for allowed commands, resolved at spawn time.
pub const REAL_GIT_ENV: &str = "ORSO_REAL_GIT";
/// The one-line stderr the shim prints (for the agent's logs) before exiting `1`.
pub const BLOCKED_MSG: &str = "orso: switching drafts is only allowed from the app";
/// Marker near the top of the generated `git` launcher, used to skip it when falling back
/// to "the next git on PATH".
const SHIM_MARKER: &str = "orso git-shim";

/// `{projects_dir}/.orso/bin`, the daemon-owned dir holding the `git` launcher.
pub fn shim_dir(projects_dir: &Path) -> PathBuf {
    projects_dir.join(crate::projects::STATE_DIR).join("bin")
}

/// Whether a `git` invocation (args after the command, no leading `git`) moves HEAD.
///
/// Blocks only HEAD-moving forms: `checkout <branch>` (including the `-` alias for the
/// previous branch), `checkout -b/-B/--orphan/--detach`, and `switch` with any argument.
/// Path restores (`checkout -- <path>` and `checkout <tree-ish> -- <path>`) and every
/// non-checkout command pass through.
pub fn is_head_moving(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        Some("checkout") => checkout_moves_head(&args[1..]),
        Some("switch") => !args[1..].is_empty(),
        _ => false,
    }
}

fn checkout_moves_head(args: &[String]) -> bool {
    // Branch-creating/detaching forms always move HEAD.
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-b" | "-B" | "--orphan" | "--detach"))
    {
        return true;
    }
    // `checkout <tree-ish> -- <pathspec>` only overwrites the index/worktree for those
    // paths and leaves HEAD alone; any `--` makes this a path restore, not a switch.
    if args.iter().any(|a| a == "--") {
        return false;
    }
    // No `--`: a branch argument switches HEAD. `-` is an alias for the previous branch.
    args.iter().any(|a| a == "-" || !a.starts_with('-'))
}

/// Ensure the `git` launcher exists in [`shim_dir`] and return that dir. `None` when there
/// is no projects dir (tests) or the launcher cannot be written.
pub fn ensure_shim_dir(config: &Config) -> Option<PathBuf> {
    if config.projects_dir.as_os_str().is_empty() {
        return None;
    }
    let dir = shim_dir(&config.projects_dir);
    let script = dir.join("git");
    let content = shim_script(&config.hook_exe_path());
    if std::fs::create_dir_all(&dir).is_err() {
        eprintln!("orso: cannot create the git launcher dir {}", dir.display());
        return None;
    }
    let fresh = std::fs::read_to_string(&script)
        .map(|existing| existing != content)
        .unwrap_or(true);
    if fresh && write_executable(&script, &content).is_err() {
        eprintln!(
            "orso: cannot write the git launcher {}; agents may switch drafts",
            script.display()
        );
        return None;
    }
    Some(dir)
}

/// The system git the shim execs, resolved at spawn time: the first `git` on the daemon's
/// `PATH` (the same resolution as its own `git` invocations), else the bare name.
pub fn resolve_system_git() -> OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let cand = dir.join("git");
        if is_executable(&cand) {
            return cand.into_os_string();
        }
    }
    OsString::from("git")
}

/// The shim entry point: block a HEAD move, else `exec` the real git.
pub fn run(args: Vec<String>) -> ExitCode {
    if std::env::var_os(ALLOW_ENV).as_deref() != Some(OsStr::new("1")) && is_head_moving(&args) {
        eprintln!("{BLOCKED_MSG}");
        return ExitCode::from(1);
    }
    let git = real_git();
    #[cfg(unix)]
    {
        let err = std::process::Command::new(&git).args(&args).exec();
        eprintln!("orso: cannot run git ({}): {err}", git.to_string_lossy());
    }
    #[cfg(not(unix))]
    {
        let _ = (git, args);
    }
    ExitCode::from(1)
}

/// The real git to exec: `$ORSO_REAL_GIT` (when it names a path), else the first `git` on
/// `PATH` that is not this shim.
fn real_git() -> OsString {
    if let Some(git) = std::env::var_os(REAL_GIT_ENV) {
        if Path::new(&git).components().count() > 1 {
            return git;
        }
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let cand = dir.join("git");
        if is_executable(&cand) && !is_shim(&cand) {
            return cand.into_os_string();
        }
    }
    OsString::from("git")
}

fn is_shim(candidate: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(candidate) else {
        return false;
    };
    let mut head = [0u8; 512];
    let Ok(n) = file.read(&mut head) else {
        return false;
    };
    head[..n]
        .windows(SHIM_MARKER.len())
        .any(|w| w == SHIM_MARKER.as_bytes())
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn write_executable(path: &Path, content: &str) -> std::io::Result<()> {
    std::fs::write(path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
}

/// POSIX single-quote `s` so a shell reads it back as one word.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn shim_script(orso: &Path) -> String {
    format!(
        "#!/bin/sh\n# {SHIM_MARKER}: only the app switches drafts; do not edit.\nexec {} git-shim -- \"$@\"\n",
        sh_quote(&orso.to_string_lossy())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn head_moving_matrix() {
        for blocked in [
            &["checkout", "content/other"][..],
            &["checkout", "-"][..],
            &["checkout", "-b", "content/x"][..],
            &["checkout", "-B", "content/x"][..],
            &["checkout", "--orphan", "content/x"][..],
            &["checkout", "--detach"][..],
            &["checkout", "--detach", "main"][..],
            &["switch", "content/x"][..],
            &["switch", "-c", "content/x"][..],
            &["switch", "-C", "content/x"][..],
            &["switch", "--detach"][..],
            &["switch", "-"][..],
        ] {
            assert!(is_head_moving(&v(blocked)), "{blocked:?} must be blocked");
        }
    }

    #[test]
    fn pass_through_matrix() {
        for allowed in [
            &[] as &[&str],
            &["status"][..],
            &["add", "-A"][..],
            &["commit", "-m", "x"][..],
            &["fetch", "origin", "main"][..],
            &["merge", "origin/main"][..],
            &["push"][..],
            &["checkout"][..],
            &["checkout", "--", "content/file.md"][..],
            &["checkout", "main", "--", "file.md"][..],
            &["checkout", "origin/main", "--", "file.md"][..],
            &["checkout", "HEAD", "--", "file.md"][..],
            &["checkout", "-p", "--", "content/file.md"][..],
            &["checkout", "--ours", "--", "content/file.md"][..],
            &["checkout", "-f", "--", "content/file.md"][..],
            &["switch"][..],
        ] {
            assert!(!is_head_moving(&v(allowed)), "{allowed:?} must pass through");
        }
    }

    #[test]
    fn resolve_system_git_is_a_path_or_bare_name() {
        let git = resolve_system_git();
        let s = git.to_string_lossy();
        assert!(s == "git" || s.contains('/'), "{s}");
    }

    #[test]
    fn ensure_shim_writes_an_executable_idempotent_launcher() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config {
            projects_dir: tmp.path().to_path_buf(),
            hook_exe: Some(PathBuf::from("/app/orso")),
            ..Default::default()
        };
        let dir = ensure_shim_dir(&config).unwrap();
        assert_eq!(dir, shim_dir(tmp.path()));
        let script = dir.join("git");
        let content = std::fs::read_to_string(&script).unwrap();
        assert!(content.contains("git-shim -- \"$@\""), "{content}");
        assert!(content.contains("/app/orso"), "{content}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_ne!(std::fs::metadata(&script).unwrap().permissions().mode() & 0o111, 0);
        }
        // Idempotent: a second call leaves the file untouched.
        let before = std::fs::metadata(&script).unwrap().modified().unwrap();
        ensure_shim_dir(&config).unwrap();
        let after = std::fs::metadata(&script).unwrap().modified().unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn ensure_shim_skips_an_empty_projects_dir() {
        let config = Config {
            projects_dir: PathBuf::new(),
            hook_exe: Some(PathBuf::from("/app/orso")),
            ..Default::default()
        };
        assert_eq!(ensure_shim_dir(&config), None);
    }
}
