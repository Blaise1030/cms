//! The `orso git-shim` hard block: agents may not switch drafts; only the app may.

// One PTY at a time across async tests — a std Mutex is intentional.
#![allow(clippy::await_holding_lock)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use orso::config::Config;

const BLOCKED_MSG: &str = "orso: switching drafts is only allowed from the app\n";

fn orso() -> &'static str {
    env!("CARGO_BIN_EXE_orso")
}

fn fake_git() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_git.sh")
}

/// Run `orso git-shim -- <args>` with `ORSO_REAL_GIT` pointed at the fake git, logging the
/// fake's argv to `log`.
fn run_shim(log: &Path, args: &[&str], allow: bool) -> Output {
    let mut cmd = Command::new(orso());
    cmd.arg("git-shim").arg("--").args(args);
    cmd.env("ORSO_REAL_GIT", fake_git());
    cmd.env("FAKE_GIT_LOG", log);
    if allow {
        cmd.env("ORSO_ALLOW_CHECKOUT", "1");
    }
    cmd.output().unwrap()
}

/// The fake git's recorded argv (one per line), empty when it was never invoked.
fn logged(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn checkout_is_blocked_without_the_allow_env() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("git.log");
    let out = run_shim(&log, &["checkout", "content/other"], false);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&out.stderr), BLOCKED_MSG);
    assert!(out.stdout.is_empty());
    assert!(!log.exists(), "the real git must not run");
}

#[test]
fn head_moving_forms_are_blocked() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("git.log");
    for args in [
        &["checkout", "content/other"][..],
        &["checkout", "-"][..],
        &["checkout", "-b", "content/x"][..],
        &["checkout", "-B", "content/x"][..],
        &["checkout", "--orphan", "content/x"][..],
        &["checkout", "--detach"][..],
        &["checkout", "--detach", "main"][..],
        &["switch", "content/x"][..],
        &["switch", "-c", "content/x"][..],
        &["switch", "--detach"][..],
        &["switch", "-"][..],
    ] {
        let out = run_shim(&log, args, false);
        assert_eq!(
            out.status.code(),
            Some(1),
            "args {args:?} should be blocked"
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stderr),
            BLOCKED_MSG,
            "{args:?}"
        );
    }
    assert!(!log.exists(), "no blocked form may run the real git");
}

#[test]
fn non_checkout_commands_pass_through() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("git.log");
    for (args, expected) in [
        (&["status"][..], vec!["status"]),
        (&["add", "-A"][..], vec!["add", "-A"]),
        (&["commit", "-m", "hi"][..], vec!["commit", "-m", "hi"]),
        (
            &["fetch", "origin", "main"][..],
            vec!["fetch", "origin", "main"],
        ),
        (&["merge", "origin/main"][..], vec!["merge", "origin/main"]),
        (&["push"][..], vec!["push"]),
    ] {
        std::fs::remove_file(&log).ok();
        let out = run_shim(&log, args, false);
        assert_eq!(out.status.code(), Some(0), "args {args:?}");
        assert!(out.stderr.is_empty(), "args {args:?}");
        assert_eq!(logged(&log), expected, "args {args:?}");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            format!("{}\n", expected.join("\n")),
            "stdout must pass through for {args:?}"
        );
    }
}

#[test]
fn path_only_checkout_does_not_move_head_and_passes_through() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("git.log");
    for (args, expected) in [
        (
            &["checkout", "--", "content/file.md"][..],
            vec!["checkout", "--", "content/file.md"],
        ),
        (
            &["checkout", "main", "--", "file.md"][..],
            vec!["checkout", "main", "--", "file.md"],
        ),
        (
            &["checkout", "origin/main", "--", "file.md"][..],
            vec!["checkout", "origin/main", "--", "file.md"],
        ),
        (
            &["checkout", "-p", "--", "content/file.md"][..],
            vec!["checkout", "-p", "--", "content/file.md"],
        ),
        (
            &["checkout", "--ours", "--", "content/file.md"][..],
            vec!["checkout", "--ours", "--", "content/file.md"],
        ),
    ] {
        std::fs::remove_file(&log).ok();
        let out = run_shim(&log, args, false);
        assert_eq!(out.status.code(), Some(0), "args {args:?} must be allowed");
        assert_eq!(logged(&log), expected, "args {args:?}");
    }
}

#[test]
fn checkout_is_allowed_with_the_allow_env() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("git.log");
    let out = run_shim(&log, &["checkout", "content/other"], true);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(logged(&log), vec!["checkout", "content/other"]);
}

#[test]
fn exit_code_and_stdout_pass_through_faithfully() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("git.log");
    let mut cmd = Command::new(orso());
    cmd.arg("git-shim").arg("--").args(["status"]);
    cmd.env("ORSO_REAL_GIT", fake_git());
    cmd.env("FAKE_GIT_LOG", &log);
    cmd.env("FAKE_GIT_EXIT", "3");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "status\n");
    assert_eq!(logged(&log), vec!["status"]);
}

#[test]
fn switch_without_arguments_is_not_head_moving() {
    // A bare `git switch` errors without moving HEAD; it must not be blocked.
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("git.log");
    let out = run_shim(&log, &["switch"], false);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(logged(&log), vec!["switch"]);
}

/// Chat agents get `ORSO_REAL_GIT` (an absolute path) and the daemon's `git` launcher
/// first on `PATH`.
#[test]
fn chat_agent_env_sets_real_git_and_prepends_the_shim() {
    use orso::state::AppState;

    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let config = Config {
        projects_dir: projects.clone(),
        hook_exe: Some(PathBuf::from(orso())),
        ..Default::default()
    };
    let st = AppState::with_scaffold_install(config, false);
    let env: std::collections::HashMap<String, String> =
        orso::http::chat::agent_env(&st, "p1").into_iter().collect();

    let real_git = env.get("ORSO_REAL_GIT").expect("ORSO_REAL_GIT");
    assert!(real_git.contains('/') || real_git == "git", "{real_git}");

    let path = env.get("PATH").expect("PATH");
    let shim_dir = projects.join(".orso").join("bin");
    assert!(
        path.starts_with(&format!("{}:", shim_dir.display())),
        "shim dir should lead PATH: {path}"
    );
    let launcher = shim_dir.join("git");
    assert!(
        launcher.is_file(),
        "launcher missing: {}",
        launcher.display()
    );
    assert_eq!(env.get("ORSO_PROJECT").map(String::as_str), Some("p1"));
}
