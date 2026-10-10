//! Change branches: `content/<slug>` created from latest `main`.

use std::path::Path;

use serde::Serialize;
use tokio::process::Command;

use crate::error::ApiError;
use crate::projects;

/// An open change branch plus its display title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub branch: String,
    pub title: String,
    /// Unix seconds of the branch tip commit (set for list entries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_edited: Option<i64>,
}

/// Strip leading "New post:" / "New page:" / "New:" (case-insensitive) and trim.
pub fn display_title(raw: &str) -> Result<String, ApiError> {
    let mut s = raw.trim();
    for prefix in ["new post:", "new page:", "new:"] {
        if let Some(rest) = strip_prefix_ci(s, prefix) {
            s = rest.trim();
            break;
        }
    }
    let title = s.trim();
    if title.is_empty() {
        return Err(ApiError::badreq("change title is required"));
    }
    Ok(title.to_string())
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let s_bytes = s.as_bytes();
    let p_bytes = prefix.as_bytes();
    if s_bytes.len() < p_bytes.len() {
        return None;
    }
    if s_bytes[..p_bytes.len()]
        .iter()
        .zip(p_bytes.iter())
        .all(|(a, b)| a.to_ascii_lowercase() == *b)
    {
        Some(&s[p_bytes.len()..])
    } else {
        None
    }
}

/// `content/<slug>` from a title (after prefix stripping). Errors if the slug is empty.
pub fn branch_name(raw_title: &str) -> Result<String, ApiError> {
    let title = display_title(raw_title)?;
    let slug = projects::slugify(&title);
    if slug.is_empty() {
        return Err(ApiError::badreq(
            "change title must contain at least one letter or digit",
        ));
    }
    Ok(format!("content/{slug}"))
}

/// Pick `base`, or `base-2`, `base-3`, … until no local branch exists.
pub async fn unique_branch(dir: &Path, base: &str) -> Result<String, ApiError> {
    if !branch_exists(dir, base).await? {
        return Ok(base.to_string());
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if !branch_exists(dir, &candidate).await? {
            return Ok(candidate);
        }
    }
    Err(ApiError::conflict("too many branches with this title"))
}

async fn branch_exists(dir: &Path, name: &str) -> Result<bool, ApiError> {
    // `rev-parse --verify` exits 0 when the ref exists.
    Ok(run(dir, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{name}")])
        .await
        .is_ok())
}

/// Fetch `origin/main` when a remote exists, create a unique `content/<slug>` branch
/// from latest main, check it out, and store `title` in the branch description.
///
/// Local-only projects (no `origin`) branch from local `main` — scaffolded projects
/// never get a remote until cloned from GitHub.
pub async fn create(dir: &Path, raw_title: &str) -> Result<Change, ApiError> {
    let title = display_title(raw_title)?;
    let base = branch_name(raw_title)?;
    ensure_clean(dir, "starting a new change").await?;
    let start = if has_remote(dir, "origin").await? {
        run(dir, &["fetch", "origin", "main"]).await?;
        "origin/main"
    } else {
        "main"
    };
    let branch = unique_branch(dir, &base).await?;
    run(dir, &["checkout", "-b", &branch, start]).await?;
    run(
        dir,
        &[
            "config",
            &format!("branch.{branch}.description"),
            &title,
        ],
    )
    .await?;
    Ok(Change {
        branch,
        title,
        last_edited: None,
    })
}

async fn has_remote(dir: &Path, name: &str) -> Result<bool, ApiError> {
    Ok(run(dir, &["remote", "get-url", name]).await.is_ok())
}

/// Refuse while tracked files have uncommitted modifications.
async fn ensure_clean(dir: &Path, action: &str) -> Result<(), ApiError> {
    // Ignore untracked (e.g. `node_modules` / build output from preview); those do not
    // block checkout and are not what "checkpoint first" means.
    let status = run(dir, &["status", "--porcelain", "--untracked-files=no"]).await?;
    if !status.trim().is_empty() {
        return Err(ApiError::conflict(format!(
            "Uncommitted changes — checkpoint or discard them before {action}"
        )));
    }
    Ok(())
}

/// GitHub-remote projects cannot be edited (agent prompt / checkpoint) on `main`.
/// Local-only projects (`github_repo == None`) may edit `main`.
pub async fn require_change(dir: &Path, github_repo: Option<&str>) -> Result<(), ApiError> {
    if github_repo.is_none() {
        return Ok(());
    }
    let branch = run(dir, &["branch", "--show-current"])
        .await?
        .trim()
        .to_string();
    if branch == "main" {
        return Err(ApiError::change_required(
            "Start a change first — GitHub projects cannot be edited on main",
        ));
    }
    Ok(())
}

/// Push `branch` to `origin` (authenticated via `env` from [`crate::github::GithubAuth::git_env`]).
pub async fn push(dir: &Path, branch: &str, env: &[(String, String)]) -> Result<(), ApiError> {
    if !branch.starts_with("content/") {
        return Err(ApiError::badreq("not a change branch"));
    }
    if !branch_exists(dir, branch).await? {
        return Err(ApiError::enoent(format!("no such branch: {branch}")));
    }
    run_env(dir, &["push", "-u", "origin", branch], env).await?;
    Ok(())
}

/// Load a change branch's display title (from branch description).
pub async fn get(dir: &Path, branch: &str) -> Result<Change, ApiError> {
    if !branch.starts_with("content/") {
        return Err(ApiError::badreq("not a change branch"));
    }
    if !branch_exists(dir, branch).await? {
        return Err(ApiError::enoent(format!("no such branch: {branch}")));
    }
    let title = match run(dir, &["config", "--get", &format!("branch.{branch}.description")]).await
    {
        Ok(t) => {
            let t = t.trim();
            if t.is_empty() {
                branch
                    .strip_prefix("content/")
                    .unwrap_or(branch)
                    .replace('-', " ")
            } else {
                t.to_string()
            }
        }
        Err(_) => branch
            .strip_prefix("content/")
            .unwrap_or(branch)
            .replace('-', " "),
    };
    Ok(Change {
        branch: branch.to_string(),
        title,
        last_edited: None,
    })
}

/// Active change = current `content/*` branch (with its description title), else `None`.
pub async fn active(dir: &Path) -> Result<Option<Change>, ApiError> {
    let branch = run(dir, &["branch", "--show-current"])
        .await?
        .trim()
        .to_string();
    if !branch.starts_with("content/") {
        return Ok(None);
    }
    let title = branch_title(dir, &branch).await;
    Ok(Some(Change {
        branch,
        title,
        last_edited: None,
    }))
}

/// Local `content/*` branches whose tip is not an ancestor of `main`, newest tip first.
pub async fn list(dir: &Path) -> Result<Vec<Change>, ApiError> {
    let out = run(
        dir,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname:short)\t%(committerdate:unix)",
            "refs/heads/content/",
        ],
    )
    .await
    .unwrap_or_default();

    let mut changes = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((branch, ts)) = line.split_once('\t') else {
            continue;
        };
        if !branch.starts_with("content/") {
            continue;
        }
        // Tip reachable from main ⇒ already merged (or never diverged).
        if run(dir, &["merge-base", "--is-ancestor", branch, "main"])
            .await
            .is_ok()
        {
            continue;
        }
        let last_edited = ts.parse::<i64>().ok();
        let title = branch_title(dir, branch).await;
        changes.push(Change {
            branch: branch.to_string(),
            title,
            last_edited,
        });
    }
    Ok(changes)
}

/// Check out `branch` when the working tree is clean.
pub async fn switch(dir: &Path, branch: &str) -> Result<Change, ApiError> {
    if !branch.starts_with("content/") {
        return Err(ApiError::badreq("branch must be a content/* change"));
    }
    if !branch_exists(dir, branch).await? {
        return Err(ApiError::enoent(format!("no such change: {branch}")));
    }
    ensure_clean(dir, "switching changes").await?;
    run(dir, &["checkout", branch]).await?;
    let title = branch_title(dir, branch).await;
    Ok(Change {
        branch: branch.to_string(),
        title,
        last_edited: None,
    })
}

/// Clean up a merged change: bring local `main` up to date, check it out, then delete the
/// local `branch` (the remote branch is left alone).
///
/// `branch` counts as merged when `pr_merged` (its pull request says so, which covers squash
/// merges) or when its tip is already in the latest `main`. Anything else is refused, and
/// nothing is deleted unless `main` fast-forwarded first.
pub async fn cleanup(
    dir: &Path,
    branch: &str,
    pr_merged: bool,
    env: &[(String, String)],
) -> Result<(), ApiError> {
    if !branch.starts_with("content/") {
        return Err(ApiError::badreq("branch must be a content/* change"));
    }
    if !branch_exists(dir, branch).await? {
        return Err(ApiError::enoent(format!("no such change: {branch}")));
    }
    ensure_clean(dir, "cleaning up").await?;
    let latest = if has_remote(dir, "origin").await? {
        run_env(dir, &["fetch", "origin", "main"], env).await?;
        "origin/main"
    } else {
        "main"
    };
    let in_main = run(dir, &["merge-base", "--is-ancestor", branch, latest])
        .await
        .is_ok();
    if !pr_merged && !in_main {
        return Err(ApiError::conflict(
            "This change has not been merged yet — merge its pull request first",
        ));
    }
    run(dir, &["checkout", "main"]).await?;
    if latest != "main" {
        run(dir, &["merge", "--ff-only", latest]).await?;
    }
    // `-D`: a squash-merged tip is never an ancestor of `main`.
    run(dir, &["branch", "-D", branch]).await?;
    Ok(())
}

async fn branch_title(dir: &Path, branch: &str) -> String {
    match run(dir, &["config", "--get", &format!("branch.{branch}.description")]).await {
        Ok(t) => {
            let t = t.trim();
            if t.is_empty() {
                branch
                    .strip_prefix("content/")
                    .unwrap_or(branch)
                    .replace('-', " ")
            } else {
                t.to_string()
            }
        }
        Err(_) => branch
            .strip_prefix("content/")
            .unwrap_or(branch)
            .replace('-', " "),
    }
}

async fn run(dir: &Path, args: &[&str]) -> Result<String, ApiError> {
    run_env(dir, args, &[]).await
}

async fn run_env(dir: &Path, args: &[&str], env: &[(String, String)]) -> Result<String, ApiError> {
    let mut cmd = Command::new("git");
    cmd.args([
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "core.quotepath=off",
    ])
    .args(args)
    .current_dir(dir)
    .stdin(std::process::Stdio::null());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .await
        .map_err(|e| ApiError::git(format!("failed to run git: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let lower = stderr.to_ascii_lowercase();
        if lower.contains("permission") || lower.contains("denied") || lower.contains("403") {
            return Err(ApiError::sandbox("You don't have push access"));
        }
        if lower.contains("authentication") || lower.contains("401") || lower.contains("credentials")
        {
            return Err(ApiError::unauth(
                "your GitHub session expired — sign in again",
            ));
        }
        return Err(ApiError::git(format!(
            "git {} failed ({}): {}",
            args.first().copied().unwrap_or(""),
            out.status,
            stderr.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_strips_punctuation_and_prefixes() {
        assert_eq!(
            branch_name("New post: Launch week").unwrap(),
            "content/launch-week"
        );
        assert_eq!(
            branch_name("New page: Hello, World!").unwrap(),
            "content/hello-world"
        );
        assert_eq!(branch_name("NEW: Ship it.").unwrap(), "content/ship-it");
        assert_eq!(
            display_title("New post: Launch week").unwrap(),
            "Launch week"
        );
    }

    #[test]
    fn slugify_unicode_and_empty() {
        assert_eq!(
            branch_name("Launch week 🚀").unwrap(),
            "content/launch-week"
        );
        assert!(display_title("").is_err());
        assert!(display_title("   ").is_err());
        assert!(display_title("New post:").is_err());
        assert!(branch_name("!!!").is_err());
        assert!(branch_name("日本語").is_err());
    }
}
