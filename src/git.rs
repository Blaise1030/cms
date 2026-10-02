//! Git checkpoints: one commit per settled agent turn, one-click restore.
//!
//! Callers (the ACP layer, later) invoke [`checkpoint`] once a turn has settled; the
//! settle/quiet-period logic lives with them. All operations on the same project
//! directory are serialised so a restore can never interleave with a checkpoint.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use serde::Serialize;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::error::ApiError;

/// Commit author/committer for a project repo, stored in its local git config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

impl Identity {
    /// Used when nobody is signed in.
    pub fn workbench() -> Self {
        Self {
            name: "Workbench".into(),
            email: "workbench@localhost".into(),
        }
    }
}

/// Make `identity` the author of future commits in `dir` (agent commits included).
pub async fn set_identity(dir: &Path, identity: &Identity) -> Result<(), ApiError> {
    run(dir, &["config", "--local", "user.name", &identity.name]).await?;
    run(dir, &["config", "--local", "user.email", &identity.email]).await?;
    Ok(())
}

/// Whether `dir` has a repo-local author name.
pub async fn has_identity(dir: &Path) -> bool {
    run(dir, &["config", "--local", "user.name"]).await.is_ok()
}

const CHECKPOINT_PREFIX: &str = "checkpoint: ";
const MAX_SUMMARY_CHARS: usize = 100;
const FALLBACK_SUMMARY: &str = "agent turn";

/// `{ sha, summary, author, ts }` — `author` is the commit author name, `ts` the commit time in
/// unix seconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Checkpoint {
    pub sha: String,
    pub summary: String,
    pub author: String,
    pub ts: i64,
}

fn lock_for(dir: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<StdMutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut map = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    map.entry(dir.to_path_buf()).or_default().clone()
}

/// Run git in `dir`, returning stdout. Commit identity comes from the repo's local config. Hooks are disabled: the tree is agent-authored and
/// we never want it to execute code on our behalf.
async fn run(dir: &Path, args: &[&str]) -> Result<String, ApiError> {
    let out = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.quotepath=off",
        ])
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| ApiError::git(format!("failed to run git: {e}")))?;
    if !out.status.success() {
        return Err(ApiError::git(format!(
            "git {} failed ({}): {}",
            args.first().copied().unwrap_or(""),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Single-line, bounded summary; falls back to a generic label when empty.
fn clean_summary(summary: &str) -> String {
    let line = summary.lines().map(str::trim).find(|l| !l.is_empty());
    match line {
        Some(l) => l.chars().take(MAX_SUMMARY_CHARS).collect(),
        None => FALLBACK_SUMMARY.to_string(),
    }
}

/// Commit everything in the working tree as `checkpoint: <summary>`.
///
/// Returns `Ok(None)` (and creates no commit) when the tree is clean.
pub async fn checkpoint(dir: &Path, summary: &str) -> Result<Option<Checkpoint>, ApiError> {
    let lock = lock_for(dir);
    let _guard = lock.lock().await;

    if run(dir, &["status", "--porcelain"])
        .await?
        .trim()
        .is_empty()
    {
        return Ok(None);
    }
    let summary = clean_summary(summary);
    run(dir, &["add", "-A"]).await?;
    // Exit 0 means the index matches HEAD: nothing to commit after all.
    if run(dir, &["diff", "--cached", "--quiet"]).await.is_ok() {
        return Ok(None);
    }
    run(
        dir,
        &[
            "commit",
            "--no-verify",
            "-m",
            &format!("{CHECKPOINT_PREFIX}{summary}"),
        ],
    )
    .await?;
    let sha = run(dir, &["rev-parse", "HEAD"]).await?.trim().to_string();
    let meta = run(dir, &["show", "-s", "--format=%ct%x1f%an", "HEAD"]).await?;
    let (ts, author) = meta
        .trim()
        .split_once('\u{1f}')
        .unwrap_or((meta.trim(), ""));
    Ok(Some(Checkpoint {
        sha,
        summary,
        author: author.to_string(),
        ts: ts.parse().unwrap_or_default(),
    }))
}

/// Commits on the current branch, newest first. Includes the scaffold's initial commit so
/// a project can be restored to its pristine state.
pub async fn log(dir: &Path) -> Result<Vec<Checkpoint>, ApiError> {
    // No commits yet (or unborn HEAD): empty log rather than an error.
    if run(dir, &["rev-parse", "--verify", "--quiet", "HEAD"])
        .await
        .is_err()
    {
        return Ok(Vec::new());
    }
    let out = run(dir, &["log", "--format=%H%x1f%ct%x1f%an%x1f%s"]).await?;
    Ok(out
        .lines()
        .filter_map(|line| {
            let mut it = line.splitn(4, '\u{1f}');
            let sha = it.next()?.to_string();
            let ts = it.next()?.parse().ok()?;
            let author = it.next()?.to_string();
            let subject = it.next().unwrap_or("");
            let summary = subject
                .strip_prefix(CHECKPOINT_PREFIX)
                .unwrap_or(subject)
                .to_string();
            Some(Checkpoint {
                sha,
                summary,
                author,
                ts,
            })
        })
        .collect())
}

/// Validate a client-supplied revision and resolve it to a full commit SHA that is part of
/// the current branch's history. Only hex is accepted, so it can never be read as an option
/// or a ref expression.
async fn resolve(dir: &Path, sha: &str) -> Result<String, ApiError> {
    if sha.len() < 4 || sha.len() > 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApiError::badreq("sha must be 4-40 hex characters"));
    }
    let spec = format!("{sha}^{{commit}}");
    let full = run(dir, &["rev-parse", "--verify", "--quiet", &spec])
        .await
        .map_err(|_| ApiError::enoent(format!("no such checkpoint: {sha}")))?
        .trim()
        .to_string();
    // Must be reachable from HEAD: `merge-base --is-ancestor` exits 0 iff so.
    run(dir, &["merge-base", "--is-ancestor", &full, "HEAD"])
        .await
        .map_err(|_| ApiError::enoent(format!("no such checkpoint: {sha}")))?;
    Ok(full)
}

/// Unified diff introduced by the given commit (for the root commit: everything added).
pub async fn diff(dir: &Path, sha: &str) -> Result<String, ApiError> {
    let full = resolve(dir, sha).await?;
    run(
        dir,
        &[
            "show",
            "--format=",
            "--patch",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            &full,
        ],
    )
    .await
}

/// Restore the working tree to `sha`: `git reset --hard <sha> && git clean -fd`.
/// Ignored files (e.g. `node_modules`) are left alone. Returns the full SHA.
pub async fn restore(dir: &Path, sha: &str) -> Result<String, ApiError> {
    let lock = lock_for(dir);
    let _guard = lock.lock().await;
    let full = resolve(dir, sha).await?;
    run(dir, &["reset", "--hard", &full]).await?;
    run(dir, &["clean", "-fd"]).await?;
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::clean_summary;

    #[test]
    fn summary_is_single_line_and_bounded() {
        assert_eq!(clean_summary("  \n first line \nsecond"), "first line");
        assert_eq!(clean_summary(""), "agent turn");
        assert_eq!(clean_summary(&"x".repeat(500)).chars().count(), 100);
    }
}
