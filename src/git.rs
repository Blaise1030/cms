//! Git checkpoints: one commit per settled agent turn, one-click restore.
//!
//! Callers (the ACP layer, later) invoke [`checkpoint`] once a turn has settled; the
//! settle/quiet-period logic lives with them. All operations on the same project
//! directory are serialised so a restore can never interleave with a checkpoint.

use std::collections::{HashMap, HashSet};
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
    pub fn orso() -> Self {
        Self {
            name: "orso".into(),
            email: "orso@localhost".into(),
        }
    }
}

/// Make `identity` the author of future commits in `dir` (agent commits included).
pub async fn set_identity(dir: &Path, identity: &Identity) -> Result<(), ApiError> {
    let lock = lock_for(dir);
    let _guard = lock.lock().await;
    run(dir, &["config", "--local", "user.name", &identity.name]).await?;
    run(dir, &["config", "--local", "user.email", &identity.email]).await?;
    Ok(())
}

/// Whether `dir` has both a repo-local author name and email.
pub async fn has_identity(dir: &Path) -> bool {
    run(dir, &["config", "--local", "user.name"]).await.is_ok()
        && run(dir, &["config", "--local", "user.email"]).await.is_ok()
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

pub(crate) fn lock_for(dir: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<StdMutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut map = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    map.entry(dir.to_path_buf()).or_default().clone()
}

/// Run git in `dir`, returning stdout. Commit identity comes from the repo's local config.
/// Hooks are disabled: the tree is agent-authored and we never want it to execute code on our
/// behalf.
async fn run(dir: &Path, args: &[&str]) -> Result<String, ApiError> {
    run_with_index(dir, args, None).await
}

async fn run_with_index(
    dir: &Path,
    args: &[&str],
    index: Option<&Path>,
) -> Result<String, ApiError> {
    let mut command = Command::new("git");
    if let Some(index) = index {
        command
            .env("GIT_INDEX_FILE", index)
            .env("GIT_LITERAL_PATHSPECS", "1");
    }
    let out = command
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

/// Paths with local modifications (`git status --porcelain`), destination path for renames.
pub async fn changed_paths(dir: &Path) -> Result<Vec<String>, ApiError> {
    let out = run(dir, &["status", "--porcelain"]).await?;
    Ok(out
        .lines()
        .filter_map(|line| {
            if line.len() < 4 {
                return None;
            }
            let path = line[3..].trim();
            let path = path.rsplit(" -> ").next().unwrap_or(path);
            Some(path.to_string())
        })
        .collect())
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
    // A merge the agent is still resolving: committing now would seal in conflict markers.
    if run(dir, &["rev-parse", "-q", "--verify", "MERGE_HEAD"])
        .await
        .is_ok()
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

/// Commit only the selected project-relative files, preserving unrelated index entries
/// and all working-tree bytes. Paths must identify files, never directories or symlinks.
/// Like project checkpoints, this requires an existing HEAD.
pub async fn checkpoint_paths(
    dir: &Path,
    summary: &str,
    paths: &[PathBuf],
) -> Result<Option<Checkpoint>, ApiError> {
    let lock = lock_for(dir);
    let _guard = lock.lock().await;
    if paths.is_empty() {
        return Ok(None);
    }
    let root = std::fs::canonicalize(dir)?;
    let mut selected = Vec::new();
    for path in paths {
        let name = path
            .to_str()
            .ok_or_else(|| ApiError::badreq("invalid path"))?;
        if name.is_empty()
            || name.contains('\0')
            || path.components().any(|part| {
                !matches!(part, std::path::Component::Normal(_))
                    || [".git", ".hg", ".svn", ".bzr", "_darcs", ".fossil", ".jj"]
                        .iter()
                        .any(|vcs| part.as_os_str().eq_ignore_ascii_case(vcs))
            })
        {
            return Err(ApiError::sandbox(
                "checkpoint paths must be project-relative files",
            ));
        }
        let mut target = root.clone();
        for part in path.components() {
            target.push(part);
            match std::fs::symlink_metadata(&target) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(ApiError::sandbox(
                        "checkpoint paths cannot contain symlinks",
                    ));
                }
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
        }
        if target.is_dir() {
            return Err(ApiError::badreq("checkpoint paths must identify files"));
        }
        selected.push(name);
    }

    // Hold Git's real index lock throughout preparation and publication. Git processes
    // using our isolated index acquire their own separate locks.
    let index = run(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    )
    .await?;
    let index = PathBuf::from(index.trim());
    let index_lock = index.with_file_name("index.lock");
    let mut guard = ScopedIndexFiles::create(index_lock)?;
    let head = run(dir, &["rev-parse", "HEAD"]).await?.trim().to_string();
    let commit_index = guard.temporary.join("commit-index");
    run_with_index(dir, &["read-tree", &head], Some(&commit_index)).await?;
    // Literal pathspecs still match descendants. Refuse directory prefixes in
    // either HEAD or the user's index, even when the directory is gone on disk.
    for path in &selected {
        for source in [&commit_index, &index] {
            let entries =
                run_with_index(dir, &["ls-files", "-z", "--", path], Some(source)).await?;
            if entries
                .split('\0')
                .filter(|entry| !entry.is_empty())
                .any(|entry| entry != *path)
            {
                return Err(ApiError::badreq(
                    "checkpoint paths must identify exact files",
                ));
            }
        }
    }
    for path in &selected {
        // A path absent from HEAD and disk is already in the desired state.
        let tracked = run_with_index(dir, &["ls-files", "--", path], Some(&commit_index)).await?;
        if root.join(path).exists() || !tracked.is_empty() {
            run_with_index(dir, &["add", "-A", "--", path], Some(&commit_index)).await?;
        }
    }
    let tree = run_with_index(dir, &["write-tree"], Some(&commit_index)).await?;
    if tree.trim() == run(dir, &["rev-parse", "HEAD^{tree}"]).await?.trim() {
        return Ok(None);
    }
    // Prepare the new real index from the user's index, replacing only selected paths
    // with the exact tree being committed (without rereading the working tree).
    let prepared = guard.temporary.join("prepared-index");
    if index.exists() {
        std::fs::copy(&index, &prepared)?;
    } else {
        run_with_index(dir, &["read-tree", &head], Some(&prepared)).await?;
    }
    let mut reset = vec!["reset", "--quiet", tree.trim(), "--"];
    reset.extend(selected);
    run_with_index(dir, &reset, Some(&prepared)).await?;
    let summary = clean_summary(summary);
    let message = format!("{CHECKPOINT_PREFIX}{summary}");
    let sha = run_with_index(
        dir,
        &["commit-tree", tree.trim(), "-p", &head, "-m", &message],
        Some(&commit_index),
    )
    .await?
    .trim()
    .to_string();
    let meta = run(dir, &["show", "-s", "--format=%ct%x1f%an", &sha]).await?;
    let (ts, author) = meta
        .trim()
        .split_once('\u{1f}')
        .unwrap_or((meta.trim(), ""));
    // Compare-and-swap HEAD refuses an external branch update during preparation.
    // Publish the prepared index through Git's lock path, on the same filesystem.
    std::fs::copy(&prepared, &guard.lock)?;
    run(dir, &["update-ref", "-m", &message, "HEAD", &sha, &head]).await?;
    guard.publish(&index)?;
    Ok(Some(Checkpoint {
        sha,
        summary,
        author: author.to_string(),
        ts: ts.parse().unwrap_or_default(),
    }))
}

struct ScopedIndexFiles {
    owns_lock: bool,
    lock: PathBuf,
    temporary: PathBuf,
}

impl ScopedIndexFiles {
    fn publish(&mut self, index: &Path) -> Result<(), ApiError> {
        std::fs::rename(&self.lock, index)?;
        self.owns_lock = false;
        Ok(())
    }

    fn create(lock: PathBuf) -> Result<Self, ApiError> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .map_err(|err| ApiError::git(format!("cannot lock project index: {err}")))?;
        let temporary = lock
            .parent()
            .unwrap()
            .join(format!("orso-index-{:032x}", rand::random::<u128>()));
        let guard = Self {
            lock,
            temporary,
            owns_lock: true,
        };
        std::fs::create_dir(&guard.temporary)?;
        Ok(guard)
    }
}

impl Drop for ScopedIndexFiles {
    fn drop(&mut self) {
        if self.owns_lock {
            let _ = std::fs::remove_file(&self.lock);
        }
        let _ = std::fs::remove_dir_all(&self.temporary);
    }
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

/// One commit in a file's history. `path` is the file's name at that commit (it differs from
/// the requested path across renames); `published` means the commit is reachable from `main`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileVersion {
    pub sha: String,
    pub summary: String,
    pub author: String,
    pub ts: i64,
    pub path: String,
    pub published: bool,
}

/// Commits on the current branch that touched `path` (following renames), newest first.
pub async fn file_log(dir: &Path, path: &str) -> Result<Vec<FileVersion>, ApiError> {
    if run(dir, &["rev-parse", "--verify", "--quiet", "HEAD"])
        .await
        .is_err()
    {
        return Ok(Vec::new());
    }
    let out = run(
        dir,
        &[
            "log",
            "--follow",
            "--diff-filter=d",
            "--name-only",
            "--format=%x1e%H%x1f%ct%x1f%an%x1f%s",
            "--",
            path,
        ],
    )
    .await?;
    let unpublished: Option<HashSet<String>> =
        match run(dir, &["rev-parse", "--verify", "--quiet", "main"]).await {
            Ok(_) => Some(
                run(dir, &["rev-list", "HEAD", "^main"])
                    .await?
                    .lines()
                    .map(str::to_string)
                    .collect(),
            ),
            Err(_) => None,
        };
    Ok(out
        .split('\u{1e}')
        .filter_map(|record| {
            let mut lines = record.lines();
            let mut it = lines.next()?.splitn(4, '\u{1f}');
            let sha = it.next()?.to_string();
            let ts = it.next()?.parse().ok()?;
            let author = it.next()?.to_string();
            let subject = it.next().unwrap_or("");
            let summary = subject
                .strip_prefix(CHECKPOINT_PREFIX)
                .unwrap_or(subject)
                .to_string();
            let path = lines.find(|l| !l.is_empty())?.to_string();
            let published = unpublished.as_ref().is_some_and(|set| !set.contains(&sha));
            Some(FileVersion {
                sha,
                summary,
                author,
                ts,
                path,
                published,
            })
        })
        .collect())
}

/// Contents of `path` as of commit `sha`.
pub async fn file_at(dir: &Path, sha: &str, path: &str) -> Result<String, ApiError> {
    if path.starts_with('-') || path.split('/').any(|s| s == "..") {
        return Err(ApiError::badreq("invalid path"));
    }
    let full = resolve(dir, sha).await?;
    run(dir, &["show", &format!("{full}:{path}")]).await
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
    fn published_index_cleanup_preserves_the_next_owners_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let lock = tmp.path().join("index.lock");
        let mut guard = super::ScopedIndexFiles::create(lock.clone()).unwrap();
        let temporary = guard.temporary.clone();
        std::fs::write(&lock, b"our index").unwrap();
        guard.publish(&tmp.path().join("index")).unwrap();
        // Model another Git process acquiring the lock after the atomic rename.
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .unwrap();
        std::fs::write(&lock, b"another operation").unwrap();
        drop(guard);
        assert_eq!(std::fs::read(&lock).unwrap(), b"another operation");
        assert_eq!(
            std::fs::read(tmp.path().join("index")).unwrap(),
            b"our index"
        );
        assert!(!temporary.exists());
    }

    #[test]
    fn summary_is_single_line_and_bounded() {
        assert_eq!(clean_summary("  \n first line \nsecond"), "first line");
        assert_eq!(clean_summary(""), "agent turn");
        assert_eq!(clean_summary(&"x".repeat(500)).chars().count(), 100);
    }
}
