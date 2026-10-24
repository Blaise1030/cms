//! Login-shell helpers shared by the agents the daemon launches.
//!
//! Chat agents are spawned directly with an ACP argv, but model listing and setup
//! health checks still have to see the `PATH` a Finder-launched daemon inherits
//! from the user's login shell, and the git shim has to lead it.

use std::path::Path;
use std::time::Duration;

/// `argv` run through the user's login shell (for its `PATH`). `shim_dir`, when
/// given, is prepended to that `PATH` so agents find the daemon's `git` launcher
/// first.
pub fn login_command(
    argv: &[String],
    cwd: &Path,
    shim_dir: Option<&Path>,
) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(user_shell());
    let line = match shim_dir {
        Some(dir) => exec_line_with_shim(argv, dir),
        None => exec_line(argv),
    };
    cmd.arg("-lc").arg(line).current_dir(cwd);
    cmd
}

/// The `PATH` the login shell reports, or `None` when the shell fails or takes
/// over 5 s.
pub async fn login_path() -> Option<std::ffi::OsString> {
    use std::os::unix::ffi::OsStringExt;
    let argv = ["/usr/bin/printenv".to_string(), "PATH".to_string()];
    let mut cmd = login_command(&argv, Path::new("/"), None);
    cmd.stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(5), cmd.output())
        .await
        .ok()?
        .ok()
        .filter(|o| o.status.success())?;
    let mut path = out.stdout;
    while path.last().is_some_and(|b| b.is_ascii_whitespace()) {
        path.pop();
    }
    Some(std::ffi::OsString::from_vec(path))
}

/// `exec` followed by each argument single-quoted for `sh`.
fn exec_line(argv: &[String]) -> String {
    let mut line = String::from("exec");
    for arg in argv {
        line.push(' ');
        line.push_str(&sh_quote(arg));
    }
    line
}

/// `exec <argv>` with `shim_dir` first on `PATH`. The login shell sets `PATH`
/// itself, so the prepend must be part of the `-c` command, not an env var.
pub fn exec_line_with_shim(argv: &[String], shim_dir: &Path) -> String {
    format!(
        "export PATH={}:\"$PATH\"; {}",
        sh_quote(&shim_dir.to_string_lossy()),
        exec_line(argv)
    )
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// `$SHELL`, else the account's login shell (a launchd service has no `$SHELL`),
/// else `/bin/sh`.
fn user_shell() -> String {
    match std::env::var("SHELL") {
        Ok(shell) if !shell.is_empty() => shell,
        _ => login_shell().unwrap_or_else(|| "/bin/sh".into()),
    }
}

fn login_shell() -> Option<String> {
    let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
    let mut buf = vec![0 as libc::c_char; 4096];
    let mut found = std::ptr::null_mut();
    let rc = unsafe {
        libc::getpwuid_r(
            libc::getuid(),
            &mut pw,
            buf.as_mut_ptr(),
            buf.len(),
            &mut found,
        )
    };
    if rc != 0 || found.is_null() || pw.pw_shell.is_null() {
        return None;
    }
    let shell = unsafe { std::ffi::CStr::from_ptr(pw.pw_shell) }
        .to_str()
        .ok()?;
    (!shell.is_empty()).then(|| shell.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn login_path_is_the_login_shells_path() {
        let path = login_path().await.expect("login shell PATH");
        assert!(
            std::env::split_paths(&path).any(|d| d == Path::new("/usr/bin")),
            "{path:?}"
        );
    }

    #[test]
    fn exec_line_quotes_every_argument() {
        let argv = ["claude", "it's a test", "$HOME", "a b"].map(String::from);
        assert_eq!(
            exec_line(&argv),
            r#"exec 'claude' 'it'\''s a test' '$HOME' 'a b'"#
        );
    }

    #[test]
    fn shim_dir_leads_the_login_path() {
        let argv = ["claude".to_string()];
        assert_eq!(
            exec_line_with_shim(&argv, Path::new("/x/.orso/bin")),
            r#"export PATH='/x/.orso/bin':"$PATH"; exec 'claude'"#
        );
    }
}
