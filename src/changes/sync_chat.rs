//! Verifying a finished sync chat turn: a typed [`Verdict`] for each way it can fail.

use std::path::Path;

use super::conflicts;
use crate::chat::history::SyncState;
use crate::error::ApiError;

/// What the agent's work in the sync worktree looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Clean: staged, marker-free, and no symlinks the live site didn't already have.
    Pass,
    /// Conflicts markers remain in this many of the collided files.
    Markers(usize),
    /// The agent added a symlink the live site didn't have.
    Symlink,
    /// The agent committed, or replaced the merge, instead of only editing files.
    AgentMovedGit,
    /// The draft branch moved (or vanished) while combining.
    DraftMoved,
}

/// Checks the agent's work in `worktree` against the sync recorded in `state`.
pub async fn verify(
    state: &SyncState,
    project_dir: &Path,
    worktree: &Path,
) -> Result<Verdict, ApiError> {
    if draft_moved(state, project_dir).await? {
        return Ok(Verdict::DraftMoved);
    }
    let head = conflicts::text(worktree, &["rev-parse", "HEAD"]).await?;
    if head != state.tip {
        return Ok(Verdict::AgentMovedGit);
    }
    let (status, merge_head, _) = conflicts::git(
        worktree,
        &["rev-parse", "-q", "--verify", "MERGE_HEAD"],
        &[],
        None,
    )
    .await?;
    if status != 0 || String::from_utf8_lossy(&merge_head).trim() != state.incoming {
        return Ok(Verdict::AgentMovedGit);
    }
    conflicts::checked(
        worktree,
        &["add", "-A", "--", ".", ":!node_modules"],
        &[],
        None,
    )
    .await?;
    if let Some(markers) = marker_files(worktree, &state.files).await? {
        return Ok(Verdict::Markers(markers));
    }
    if new_symlink(worktree, &state.tip, &state.incoming).await? {
        return Ok(Verdict::Symlink);
    }
    Ok(Verdict::Pass)
}

/// The draft branch must still be the one the sync started from.
async fn draft_moved(state: &SyncState, project_dir: &Path) -> Result<bool, ApiError> {
    let (status, branch, _) = conflicts::git(
        project_dir,
        &["rev-parse", &format!("refs/heads/{}", state.branch)],
        &[],
        None,
    )
    .await?;
    Ok(status != 0 || String::from_utf8_lossy(&branch).trim() != state.tip)
}

/// How many of `files` still have conflict markers, or `None` when none do.
async fn marker_files(worktree: &Path, files: &[String]) -> Result<Option<usize>, ApiError> {
    if files.is_empty() {
        return Ok(None);
    }
    let mut args = vec![
        "--literal-pathspecs",
        "grep",
        "--cached",
        "-l",
        "-E",
        "^(<<<<<<< |=======$|>>>>>>> )",
        "--",
    ];
    args.extend(files.iter().map(String::as_str));
    let (status, stdout, stderr) = conflicts::git(worktree, &args, &[], None).await?;
    match status {
        1 => Ok(None),
        0 => Ok(Some(
            String::from_utf8_lossy(&stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
        )),
        _ => Err(ApiError::git(format!(
            "git grep failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        ))),
    }
}

/// Whether the agent added a symlink the incoming side did not already have.
async fn new_symlink(worktree: &Path, tip: &str, incoming: &str) -> Result<bool, ApiError> {
    let raw = conflicts::checked(
        worktree,
        &["diff", "--cached", "--raw", "-z", "--no-renames", tip],
        &[],
        None,
    )
    .await?;
    let raw = String::from_utf8_lossy(&raw);
    let mut fields = raw.split('\0');
    while let (Some(header), Some(path)) = (fields.next(), fields.next()) {
        if header.split_whitespace().nth(1) != Some("120000") {
            continue;
        }
        let entry = conflicts::text(
            worktree,
            &["--literal-pathspecs", "ls-tree", incoming, "--", path],
        )
        .await?;
        if !entry.starts_with("120000 ") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// "src/content/pricing-plans.md" → "Pricing plans".
pub fn label(path: &str) -> String {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .replace(['-', '_'], " ");
    let mut chars = stem.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => path.to_string(),
    }
}

/// The chat title: the collided pages, at most three, kept short for the sidebar.
pub fn title(state: &SyncState) -> String {
    let labels: Vec<String> = state.files.iter().take(3).map(|f| label(f)).collect();
    let title = format!("Combining {}", labels.join(", "));
    title.chars().take(72).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::history::SyncPhase;
    use std::process::Command;

    fn sh(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args([
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@e",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Repo mid-merge with a conflict in `page.md`; `live` adds extra files on the incoming side.
    /// Returns (dir, tip, incoming).
    fn conflicted_with(live: impl Fn(&Path)) -> (tempfile::TempDir, String, String) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        sh(dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("page.md"), "title: base\n").unwrap();
        std::fs::write(dir.join("notes.md"), "Notes\n=======\n").unwrap();
        sh(dir, &["add", "."]);
        sh(dir, &["commit", "-qm", "base"]);
        sh(dir, &["checkout", "-qb", "draft"]);
        std::fs::write(dir.join("page.md"), "title: draft\n").unwrap();
        sh(dir, &["commit", "-qam", "draft"]);
        let tip = sh(dir, &["rev-parse", "HEAD"]);
        sh(dir, &["checkout", "-q", "main"]);
        std::fs::write(dir.join("page.md"), "title: live\n").unwrap();
        live(dir);
        sh(dir, &["add", "-A"]);
        sh(dir, &["commit", "-qm", "live"]);
        let incoming = sh(dir, &["rev-parse", "HEAD"]);
        sh(dir, &["checkout", "-q", "draft"]);
        sh(dir, &["merge", "--no-commit", "--no-ff", &incoming]);
        let page = std::fs::read_to_string(dir.join("page.md")).unwrap();
        assert!(page.contains("<<<<<<< "), "fixture must conflict");
        (tmp, tip, incoming)
    }

    fn conflicted() -> (tempfile::TempDir, String, String) {
        conflicted_with(|_| {})
    }

    fn resolve(dir: &Path) {
        std::fs::write(dir.join("page.md"), "title: draft and live\n").unwrap();
    }

    fn record(branch: &str, tip: &str, incoming: &str, files: &[&str]) -> SyncState {
        SyncState {
            conflict_session_id: "session".into(),
            branch: branch.into(),
            tip: tip.into(),
            incoming: incoming.into(),
            files: files.iter().map(|f| f.to_string()).collect(),
            phase: SyncPhase::Checking,
            started_at: 0,
            ended_at: None,
            repair_attempted: false,
        }
    }

    /// A sync state whose `branch` is the fixture's current HEAD — the draft it started from.
    fn state(dir: &Path, branch: &str, tip: &str, incoming: &str, files: &[&str]) -> SyncState {
        sh(dir, &["branch", "-f", branch, "HEAD"]);
        record(branch, tip, incoming, files)
    }

    #[tokio::test]
    async fn verify_rejects_markers() {
        let (tmp, tip, incoming) = conflicted();
        let st = state(tmp.path(), "content/page", &tip, &incoming, &["page.md"]);
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::Markers(1)
        );
    }

    #[tokio::test]
    async fn verify_ignores_markers_outside_conflicted_files() {
        let (tmp, tip, incoming) = conflicted();
        let st = state(tmp.path(), "content/page", &tip, &incoming, &["page.md"]);
        resolve(tmp.path());
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::Pass
        );
    }

    #[tokio::test]
    async fn verify_rejects_agent_commit() {
        let (tmp, tip, incoming) = conflicted();
        let st = state(tmp.path(), "content/page", &tip, &incoming, &["page.md"]);
        resolve(tmp.path());
        sh(tmp.path(), &["commit", "-qam", "agent"]);
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::AgentMovedGit
        );
    }

    #[tokio::test]
    async fn verify_rejects_new_symlink() {
        let (tmp, tip, incoming) = conflicted();
        let st = state(tmp.path(), "content/page", &tip, &incoming, &["page.md"]);
        resolve(tmp.path());
        std::os::unix::fs::symlink("/etc/passwd", tmp.path().join("link")).unwrap();
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::Symlink
        );
    }

    #[tokio::test]
    async fn verify_allows_symlink_from_incoming() {
        let (tmp, tip, incoming) = conflicted_with(|dir| {
            std::os::unix::fs::symlink("page.md", dir.join("link")).unwrap();
        });
        let st = state(tmp.path(), "content/page", &tip, &incoming, &["page.md"]);
        resolve(tmp.path());
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::Pass
        );
    }

    #[tokio::test]
    async fn moved_draft_is_reported() {
        let (tmp, tip, incoming) = conflicted();
        sh(tmp.path(), &["reset", "-q", "--hard", "HEAD~0"]);
        // move the branch tip the sync was created from
        let st = state(
            tmp.path(),
            "content/page",
            &format!("{tip}0"),
            &incoming,
            &["page.md"],
        );
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::DraftMoved
        );
    }

    #[tokio::test]
    async fn marker_verdict_counts_files() {
        let (tmp, tip, incoming) = conflicted();
        let st = state(
            tmp.path(),
            "content/page",
            &tip,
            &incoming,
            &["page.md", "other.md"],
        );
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::Markers(1)
        );
    }

    #[test]
    fn labels_from_file_stems() {
        assert_eq!(label("src/content/pricing-plans.md"), "Pricing plans");
        assert_eq!(label("about_us.yaml"), "About us");
        assert_eq!(label(".md"), ".md");
    }

    #[test]
    fn title_names_up_to_three_files_within_72_chars() {
        let st = record(
            "content/page",
            "tip",
            "incoming",
            &[
                "src/content/pricing-plans.md",
                "about_us.md",
                "home.md",
                "extra.md",
            ],
        );
        assert_eq!(title(&st), "Combining Pricing plans, About us, Home");

        let long = vec![format!("{}.md", "x".repeat(60)); 2];
        let files: Vec<&str> = long.iter().map(String::as_str).collect();
        let st = record("content/page", "tip", "incoming", &files);
        let title = title(&st);
        assert_eq!(title.chars().count(), 72, "{title}");
        assert!(title.starts_with("Combining "), "{title}");
    }
}
