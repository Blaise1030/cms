//! Builds one revision of a project into a static site.
//!
//! The build runs in a throwaway git worktree that exists only while the build
//! runs: it is removed after success, failure, timeout and cancellation alike,
//! so previews never keep a checkout on disk. Only the built `dist` survives,
//! moved to the caller's output directory.
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::settings::{self, ProjectSettings};

#[derive(Debug)]
pub enum BuildError {
    Unavailable(String),
    Invalid(String),
}

/// Builds `revision` with `PUBLIC_BASE=base_url` and moves its `dist` to `out`.
///
/// The project's `orso.json` decides where the site builds (`root`) and whose
/// `node_modules` it uses (`installDir`); a file that fails to load means the defaults.
pub async fn build(
    repo: &Path,
    revision: &str,
    base_url: &str,
    out: &Path,
    timeout: Duration,
) -> Result<(), BuildError> {
    let unavailable = |e: std::io::Error| BuildError::Unavailable(e.to_string());
    let settings = settings::load(repo).unwrap_or_else(|_| ProjectSettings::defaults());
    if !settings.site_dir(repo).join("package.json").is_file() {
        return Err(BuildError::Unavailable(
            "This project has no preview build command".into(),
        ));
    }
    let parent = out
        .parent()
        .ok_or_else(|| BuildError::Unavailable("Preview output has no parent".into()))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(unavailable)?;
    let tmp = parent.join(format!(".build-{:016x}", rand::random::<u64>()));

    // Armed before `git worktree add`, so even a half-made worktree is removed.
    // Normally removed off the runtime once the build ends; if this future is
    // dropped mid-build instead, the guard's `Drop` removes it synchronously.
    let worktree = Worktree {
        repo: repo.to_owned(),
        dir: tmp.clone(),
    };
    let result = build_in(repo, revision, base_url, out, timeout, &settings, &tmp).await;
    worktree.remove().await;
    result
}

/// Everything that happens inside the throwaway worktree at `tmp`.
async fn build_in(
    repo: &Path,
    revision: &str,
    base_url: &str,
    out: &Path,
    timeout: Duration,
    settings: &ProjectSettings,
    tmp: &Path,
) -> Result<(), BuildError> {
    let unavailable = |e: std::io::Error| BuildError::Unavailable(e.to_string());
    let output = Command::new("git")
        .args(["worktree", "add", "--detach", "--quiet"])
        .arg(tmp)
        .arg(revision)
        .current_dir(repo)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(unavailable)?;
    if !output.status.success() {
        return Err(BuildError::Unavailable(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    link_node_modules(&settings.install_path(repo), &settings.install_path(tmp))
        .map_err(unavailable)?;

    let site = settings.site_dir(tmp);
    let mut command = Command::new("npm");
    command
        .args(["run", "build"])
        .env("PUBLIC_BASE", base_url)
        .current_dir(&site)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .process_group(0);
    let child = command.spawn().map_err(unavailable)?;
    // Dropped on every exit from here, so the build's whole process group
    // dies before its worktree is removed.
    let group = BuildGroup(child.id().map(|pid| pid as i32));
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| BuildError::Unavailable("Preview build timed out".into()))?
        .map_err(unavailable)?;
    drop(group);
    if !output.status.success() {
        return Err(classify_failure(&output.stdout, &output.stderr));
    }
    let dist = site.join("dist");
    if !dist.is_dir() {
        return Err(BuildError::Unavailable(
            "Preview build produced no dist directory".into(),
        ));
    }
    if !dist.join("index.html").is_file() {
        return Err(BuildError::Unavailable(
            "Preview build produced no home page".into(),
        ));
    }
    if out.exists() {
        tokio::fs::remove_dir_all(out).await.map_err(unavailable)?;
    }
    tokio::fs::rename(&dist, out).await.map_err(unavailable)?;
    Ok(())
}

fn classify_failure(stdout: &[u8], stderr: &[u8]) -> BuildError {
    let output_text = format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    );
    let message: String = output_text
        .chars()
        .rev()
        .take(1800)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let confirmed_invalid = [
        "SyntaxError",
        "ParseError",
        "Unexpected token",
        "Could not resolve",
        "references missing section",
        "error during build",
        "CompileError",
    ]
    .iter()
    .any(|marker| message.contains(marker));
    if confirmed_invalid {
        BuildError::Invalid(message)
    } else {
        BuildError::Unavailable(format!("Preview build could not complete: {message}"))
    }
}

/// Removes everything under `base` that is not in `keep` (left by a crash or an
/// older orso), then forgets worktrees whose directories are gone.
pub async fn sweep(repo: &Path, base: &Path, keep: &[PathBuf]) {
    if let Ok(mut entries) = tokio::fs::read_dir(base).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if keep.contains(&path) || !entry.file_type().await.is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let _ = tokio::fs::remove_dir_all(&path).await;
        }
    }
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
        .await;
}

/// Removes a build's worktree, and its directory, when dropped.
struct Worktree {
    repo: PathBuf,
    dir: PathBuf,
}
impl Worktree {
    /// Removes the worktree on a blocking thread, disarming the guard.
    async fn remove(mut self) {
        let repo = std::mem::take(&mut self.repo);
        let dir = std::mem::take(&mut self.dir);
        // If this await is cancelled the blocking task still runs to completion.
        let _ = tokio::task::spawn_blocking(move || remove_worktree(&repo, &dir)).await;
    }
}
impl Drop for Worktree {
    fn drop(&mut self) {
        // Still armed only when the build future was dropped (cancelled or
        // panicked) before `remove`; synchronous because nothing awaits a drop.
        if !self.dir.as_os_str().is_empty() {
            remove_worktree(&self.repo, &self.dir);
        }
    }
}
fn remove_worktree(repo: &Path, dir: &Path) {
    let _ = std::process::Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(dir)
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::process::Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Lets a checkout at `dir` build with the dependencies installed at `installed`.
pub(crate) fn link_node_modules(installed: &Path, dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    if installed.join("node_modules").exists() {
        std::os::unix::fs::symlink(installed.join("node_modules"), dir.join("node_modules"))?;
    }
    Ok(())
}

/// Kills a `process_group(0)` child's whole group when dropped.
pub(crate) struct BuildGroup(pub(crate) Option<i32>);
impl Drop for BuildGroup {
    fn drop(&mut self) {
        if let Some(group) = self.0 {
            // SAFETY: the child was spawned in its own process group.
            unsafe {
                libc::killpg(group, libc::SIGKILL);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command as StdCommand;

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = StdCommand::new("git")
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@example.com",
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// A repo whose site lives in `site_dir` (relative) and builds with `script`.
    fn fixture(repo: &Path, site_dir: &str, script: &str) -> String {
        fs::create_dir_all(repo).unwrap();
        fs::create_dir_all(repo.join(site_dir)).unwrap();
        git(repo, &["init", "-q", "-b", "main"]);
        let site = repo.join(site_dir);
        fs::write(
            site.join("package.json"),
            r#"{"scripts":{"build":"node build.js"}}"#,
        )
        .unwrap();
        fs::write(site.join("build.js"), script).unwrap();
        git(repo, &["add", "."]);
        git(repo, &["commit", "-q", "-m", "site"]);
        git(repo, &["rev-parse", "HEAD"])
    }

    const WRITES_DIST: &str = "const fs = require('fs');\n\
        fs.mkdirSync('dist', { recursive: true });\n\
        fs.writeFileSync('dist/index.html', '<h1>' + process.env.PUBLIC_BASE + '</h1>');\n";

    fn worktrees(repo: &Path) -> usize {
        git(repo, &["worktree", "list", "--porcelain"])
            .lines()
            .filter(|l| l.starts_with("worktree "))
            .count()
    }

    /// Everything in `base` other than `out`.
    fn leftovers(base: &Path, out: &Path) -> Vec<PathBuf> {
        fs::read_dir(base)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p != out)
            .collect()
    }

    #[tokio::test]
    async fn build_removes_worktree_after_success() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let rev = fixture(&repo, ".", WRITES_DIST);
        let base = temp.path().join("previews");
        let out = base.join("out");
        build(&repo, &rev, "/p/x/", &out, Duration::from_secs(60))
            .await
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(
            fs::read_to_string(out.join("index.html")).unwrap(),
            "<h1>/p/x/</h1>"
        );
        assert_eq!(worktrees(&repo), 1);
        assert!(
            leftovers(&base, &out).is_empty(),
            "{:?}",
            leftovers(&base, &out)
        );
    }

    #[tokio::test]
    async fn build_removes_worktree_after_failure() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let rev = fixture(&repo, ".", "process.exit(1);\n");
        let base = temp.path().join("previews");
        let out = base.join("out");
        let result = build(&repo, &rev, "/p/x/", &out, Duration::from_secs(60)).await;
        assert!(
            matches!(result, Err(BuildError::Unavailable(ref m)) if m.starts_with("Preview build could not complete")),
            "{result:?}"
        );
        assert!(!out.exists());
        assert_eq!(worktrees(&repo), 1);
        assert!(leftovers(&base, &out).is_empty());
    }

    #[tokio::test]
    async fn build_removes_worktree_after_timeout() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let started = temp.path().join("started");
        let marker = temp.path().join("still-running");
        // The build starts a grandchild that outlives npm unless the whole
        // process group is killed, then never finishes on its own. npm alone
        // takes about 2 s to start, hence a timeout well past that.
        let script = format!(
            "const fs = require('fs');\n\
             fs.writeFileSync({started:?}, 'x');\n\
             require('child_process').spawn('sh', ['-c', 'sleep 7; touch \"' + {marker:?} + '\"'], {{ stdio: 'ignore' }});\n\
             setInterval(() => {{}}, 1000);\n",
            started = started.to_string_lossy(),
            marker = marker.to_string_lossy(),
        );
        let rev = fixture(&repo, ".", &script);
        let base = temp.path().join("previews");
        let out = base.join("out");
        let result = build(&repo, &rev, "/p/x/", &out, Duration::from_secs(6)).await;
        assert!(
            matches!(result, Err(BuildError::Unavailable(ref m)) if m == "Preview build timed out"),
            "{result:?}"
        );
        assert!(started.exists(), "the build never reached its script");
        assert!(!out.exists());
        assert_eq!(worktrees(&repo), 1);
        assert!(leftovers(&base, &out).is_empty());
        // The grandchild would touch the marker 7 s after the script started,
        // well after the timeout killed the build.
        let began = fs::metadata(&started).unwrap().modified().unwrap();
        let due = began + Duration::from_secs(8);
        if let Ok(wait) = due.duration_since(std::time::SystemTime::now()) {
            tokio::time::sleep(wait).await;
        }
        assert!(!marker.exists(), "the build kept running after the timeout");
    }

    #[tokio::test]
    async fn build_runs_in_orso_root() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        fs::write(
            repo.join("orso.json"),
            r#"{"root":"site","installDir":"site"}"#,
        )
        .unwrap();
        // The build needs the project's installed dependencies to be linked in.
        let script = format!(
            "if (!require('fs').existsSync('node_modules/marker')) process.exit(1);\n{WRITES_DIST}"
        );
        // A root must hold an Astro config for orso.json to load.
        fs::create_dir_all(repo.join("site")).unwrap();
        fs::write(repo.join("site/astro.config.mjs"), "export default {};\n").unwrap();
        let rev = fixture(&repo, "site", &script);
        fs::create_dir_all(repo.join("site/node_modules/marker")).unwrap();
        let base = temp.path().join("previews");
        let out = base.join("out");
        build(&repo, &rev, "/p/y/", &out, Duration::from_secs(60))
            .await
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert!(out.join("index.html").is_file());
        assert_eq!(worktrees(&repo), 1);
        assert!(leftovers(&base, &out).is_empty());
    }

    #[tokio::test]
    async fn sweep_removes_orphans_and_prunes() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        fixture(&repo, ".", WRITES_DIST);
        let base = temp.path().join("previews");
        fs::create_dir_all(base.join("keep")).unwrap();
        fs::create_dir_all(base.join("orphan")).unwrap();
        let stale = base.join("stale");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "--detach",
                "-q",
                &stale.to_string_lossy(),
                "HEAD",
            ],
        );
        assert_eq!(worktrees(&repo), 2);
        sweep(&repo, &base, &[base.join("keep")]).await;
        assert!(base.join("keep").is_dir());
        assert!(!base.join("orphan").exists());
        assert!(!stale.exists());
        assert_eq!(worktrees(&repo), 1);
    }
}
