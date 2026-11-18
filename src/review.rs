//! Reviewing teammates' changes without touching the checkout: a change's head is fetched
//! into a private `refs/orso/review/<n>` ref (by pull-request number, so forks work) and
//! diffed straight from the object database. No working-tree change, no branch switch.

use std::path::Path;

use crate::error::ApiError;
use crate::git;

const GONE: &str = "This change is no longer waiting";

fn review_ref(number: u64) -> String {
    format!("refs/orso/review/{number}")
}

/// Fetch change `number`'s head (and refresh `origin/main`), returning the head's full SHA.
pub async fn fetch_head(
    dir: &Path,
    number: u64,
    env: &[(String, String)],
) -> Result<String, ApiError> {
    let lock = git::lock_for(dir);
    let _guard = lock.lock().await;
    let ours = review_ref(number);
    let head_spec = format!("+refs/pull/{number}/head:{ours}");
    git::run_env(
        dir,
        &[
            "fetch",
            "origin",
            &head_spec,
            "+refs/heads/main:refs/remotes/origin/main",
        ],
        env,
    )
    .await
    .map_err(|_| ApiError::enoent(GONE))?;
    let sha = git::run_env(dir, &["rev-parse", "--verify", &ours], env)
        .await
        .map_err(|_| ApiError::enoent(GONE))?;
    Ok(sha.trim().to_string())
}

/// What the change at `head_sha` does relative to where it branched from `origin/main`,
/// scoped to the site root.
pub async fn diff(dir: &Path, head_sha: &str) -> Result<String, ApiError> {
    let base = git::run_env(dir, &["merge-base", "origin/main", head_sha], &[]).await?;
    git::range_diff(
        dir,
        base.trim(),
        head_sha,
        &crate::settings::site_prefix(dir),
    )
    .await
}

/// Drop review refs for changes that are no longer open. Errors are ignored.
pub async fn prune_refs(dir: &Path, open: &[u64]) {
    let Ok(out) = git::run_env(
        dir,
        &["for-each-ref", "--format=%(refname)", "refs/orso/review/"],
        &[],
    )
    .await
    else {
        return;
    };
    let keep: Vec<String> = open.iter().map(|n| review_ref(*n)).collect();
    for name in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !keep.iter().any(|k| k == name) {
            let _ = git::run_env(dir, &["update-ref", "-d", name], &[]).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::TempDir;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A clone on `main` with `orso.json` rooted at `site`, plus a bare `origin` that holds
    /// `refs/pull/7/head`: a commit editing `site/a.md` and adding binary `site/logo.png`.
    /// Origin's `main` then gains an unrelated commit.
    fn fixture() -> (TempDir, PathBuf, String) {
        let tmp = TempDir::new().unwrap();
        let origin = tmp.path().join("origin.git");
        let work = tmp.path().join("work");
        std::fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "--bare", "-b", "main"]);
        std::fs::create_dir_all(work.join("site")).unwrap();
        git(&work, &["init", "-b", "main"]);
        git(
            &work,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        std::fs::write(work.join("orso.json"), r#"{"root":"site"}"#).unwrap();
        std::fs::write(work.join("site/a.md"), "one\n").unwrap();
        git(&work, &["add", "."]);
        git(&work, &["commit", "-m", "base"]);
        git(&work, &["push", "origin", "main"]);

        git(&work, &["checkout", "-b", "pr"]);
        std::fs::write(work.join("site/a.md"), "one\ntwo\n").unwrap();
        std::fs::write(work.join("site/logo.png"), [0u8, 1, 2, 3, 0, 255]).unwrap();
        git(&work, &["add", "."]);
        git(&work, &["commit", "-m", "pr"]);
        let sha = git(&work, &["rev-parse", "HEAD"]);
        git(&work, &["push", "origin", "pr:refs/pull/7/head"]);

        git(&work, &["checkout", "main"]);
        git(&work, &["branch", "-D", "pr"]);
        std::fs::write(work.join("unrelated.txt"), "later\n").unwrap();
        git(&work, &["add", "."]);
        git(&work, &["commit", "-m", "unrelated"]);
        git(&work, &["push", "origin", "main"]);
        // Reviewer's checkout is behind origin/main, as in real life.
        git(&work, &["reset", "--hard", "HEAD~1"]);
        (tmp, work, sha)
    }

    fn refs(dir: &Path, pattern: &str) -> String {
        git(dir, &["for-each-ref", "--format=%(refname)", pattern])
    }

    #[tokio::test]
    async fn fetch_head_stores_ref_without_touching_checkout() {
        let (_tmp, work, sha) = fixture();
        let heads_before = refs(&work, "refs/heads/");
        let status_before = git(&work, &["status", "--porcelain"]);
        let branch_before = git(&work, &["branch", "--show-current"]);

        let got = fetch_head(&work, 7, &[]).await.unwrap();

        assert_eq!(got, sha);
        assert_eq!(refs(&work, "refs/orso/review/"), "refs/orso/review/7");
        assert_eq!(git(&work, &["status", "--porcelain"]), status_before);
        assert_eq!(git(&work, &["branch", "--show-current"]), branch_before);
        assert_eq!(refs(&work, "refs/heads/"), heads_before);
    }

    #[tokio::test]
    async fn diff_is_head_against_merge_base_scoped_to_site_root() {
        let (_tmp, work, _sha) = fixture();
        let sha = fetch_head(&work, 7, &[]).await.unwrap();
        let out = diff(&work, &sha).await.unwrap();
        assert!(out.contains("site/a.md"), "{out}");
        assert!(out.contains("+two"), "{out}");
        assert!(out.contains("Binary files"), "{out}");
        assert!(out.contains("site/logo.png"), "{out}");
        assert!(!out.contains("unrelated.txt"), "{out}");
    }

    #[tokio::test]
    async fn fetch_head_of_missing_pr_is_no_longer_waiting() {
        let (_tmp, work, _sha) = fixture();
        let err = fetch_head(&work, 99, &[]).await.unwrap_err();
        assert_eq!(err.message, "This change is no longer waiting");
    }

    #[tokio::test]
    async fn prune_refs_drops_closed_reviews() {
        let (_tmp, work, sha) = fixture();
        git(&work, &["update-ref", "refs/orso/review/7", &sha]);
        git(&work, &["update-ref", "refs/orso/review/8", &sha]);
        prune_refs(&work, &[8]).await;
        assert_eq!(refs(&work, "refs/orso/review/"), "refs/orso/review/8");
    }
}
