//! Change branches: `content/<slug>` created from latest `main`.

use std::path::Path;

use serde::Serialize;
use tokio::process::Command;

use crate::error::ApiError;
use crate::projects;

pub mod auto_resolve;
pub mod conflict_sessions;
pub mod conflicts;

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

/// Pick `base`, or `base-2`, `base-3`, … until neither a local branch nor `origin`'s copy of
/// one has that name (a cleaned-up change keeps its remote branch and its PR).
pub async fn unique_branch(dir: &Path, base: &str) -> Result<String, ApiError> {
    if !name_taken(dir, base).await? {
        return Ok(base.to_string());
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if !name_taken(dir, &candidate).await? {
            return Ok(candidate);
        }
    }
    Err(ApiError::conflict("too many branches with this title"))
}

async fn name_taken(dir: &Path, name: &str) -> Result<bool, ApiError> {
    Ok(branch_exists(dir, name).await?
        || run(
            dir,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/remotes/origin/{name}"),
            ],
        )
        .await
        .is_ok())
}

async fn branch_exists(dir: &Path, name: &str) -> Result<bool, ApiError> {
    // `rev-parse --verify` exits 0 when the ref exists.
    Ok(run(
        dir,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{name}"),
        ],
    )
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
    let on_main = current_branch(dir).await? == "main";
    // Edits made on `main` move into the new change; edits on another change must be saved.
    if !on_main {
        ensure_clean(dir, "starting a new change").await?;
    }
    let start = if has_remote(dir, "origin").await? {
        run(dir, &["fetch", "origin", "main"]).await?;
        "origin/main"
    } else {
        "main"
    };
    let branch = unique_branch(dir, &base).await?;
    if let Err(e) = run(dir, &["checkout", "-b", &branch, start]).await {
        return Err(if on_main && is_dirty(dir).await? {
            ApiError::conflict(
                "Your unsaved edits clash with the latest main — checkpoint or discard them before starting a new change",
            )
        } else {
            e
        });
    }
    run(
        dir,
        &["config", &format!("branch.{branch}.description"), &title],
    )
    .await?;
    Ok(Change {
        branch,
        title,
        last_edited: None,
    })
}

async fn current_branch(dir: &Path) -> Result<String, ApiError> {
    Ok(run(dir, &["branch", "--show-current"])
        .await?
        .trim()
        .to_string())
}

/// Tracked files have uncommitted modifications.
pub async fn is_dirty(dir: &Path) -> Result<bool, ApiError> {
    let status = run(dir, &["status", "--porcelain", "--untracked-files=no"]).await?;
    Ok(!status.trim().is_empty())
}

async fn has_remote(dir: &Path, name: &str) -> Result<bool, ApiError> {
    Ok(run(dir, &["remote", "get-url", name]).await.is_ok())
}

/// Refuse while tracked files have uncommitted modifications.
async fn ensure_clean(dir: &Path, action: &str) -> Result<(), ApiError> {
    // Ignore untracked (e.g. `node_modules` / build output from preview); those do not
    // block checkout and are not what "checkpoint first" means.
    if is_dirty(dir).await? {
        return Err(ApiError::conflict(format!(
            "Uncommitted changes — checkpoint or discard them before {action}"
        )));
    }
    Ok(())
}

/// GitHub-remote projects cannot be edited (terminal / checkpoint) on `main`.
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
    let title = match run(
        dir,
        &["config", "--get", &format!("branch.{branch}.description")],
    )
    .await
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
    // Only the app's checkout is allowed: mark this one invocation (never process-wide).
    run_env(
        dir,
        &["checkout", branch],
        &[(crate::git_shim::ALLOW_ENV.to_string(), "1".to_string())],
    )
    .await?;
    let title = branch_title(dir, branch).await;
    Ok(Change {
        branch: branch.to_string(),
        title,
        last_edited: None,
    })
}

/// Outcome of [`get_latest`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    /// Local `main` moved forward to `origin/main`.
    pub main_updated: bool,
    /// Local `main` has commits that are not on `origin/main`, so it was left as it is (the
    /// change branches were still brought up to `origin/main`).
    pub main_diverged: bool,
    /// Change branches that now include latest `main` (merge commit, or fast-forward when the
    /// active change has no work of its own yet).
    pub updated: Vec<Change>,
    /// Change branches whose merge with `main` conflicts; left exactly as they were.
    pub conflicts: Vec<Change>,
    /// The checked-out branch needed the latest but was left as it is because a terminal is
    /// working in the checkout ([`get_latest_leaving_checkout`]).
    pub skipped: Option<Change>,
}

/// `git merge-tree --write-tree` (how merges are computed without a checkout) needs 2.38+.
/// `version` is `git --version` output.
pub fn check_git_version(version: &str) -> Result<(), ApiError> {
    let found = version
        .trim()
        .strip_prefix("git version ")
        .and_then(|v| v.split_whitespace().next())
        .unwrap_or("");
    let mut parts = found.split('.').map(|p| p.parse::<u32>().ok());
    match (parts.next().flatten(), parts.next().flatten()) {
        (Some(major), Some(minor)) if (major, minor) >= (2, 38) => Ok(()),
        _ => Err(ApiError::git(format!(
            "Get latest needs git 2.38 or newer (found {}) — update git and try again",
            if found.is_empty() {
                version.trim()
            } else {
                found
            }
        ))),
    }
}

/// Get latest: fetch `origin/main` (with `env`, the GitHub auth from
/// [`crate::github::GithubAuth::git_env`]), fast-forward local `main`, and merge it into every
/// open change branch — the active one plus every `content/*` branch with work not on `main`.
/// Merges are commits authored by the repo's identity (never a rebase, so shared branches
/// never need a force-push). Conflicting branches are reported and left untouched.
///
/// All or nothing: every merge is computed first (objects only, no refs); then the refs move
/// in one atomic `update-ref` transaction and, last, the checked-out branch fast-forwards.
/// Any failure before that leaves `main` and every change branch where they were.
pub async fn get_latest(dir: &Path, env: &[(String, String)]) -> Result<Update, ApiError> {
    get_latest_impl(dir, env, false).await
}

/// [`get_latest`] without touching the working tree: the checked-out branch is left as it is
/// (reported in [`Update::skipped`] when it needed the latest), everything else moves by ref.
/// For while a terminal is editing the checkout.
pub async fn get_latest_leaving_checkout(
    dir: &Path,
    env: &[(String, String)],
) -> Result<Update, ApiError> {
    get_latest_impl(dir, env, true).await
}

async fn get_latest_impl(
    dir: &Path,
    env: &[(String, String)],
    leave_checkout: bool,
) -> Result<Update, ApiError> {
    check_git_version(&run(dir, &["--version"]).await?)?;
    // Serialise with checkpoints/restore on the same repo.
    let lock = crate::git::lock_for(dir);
    let _guard = lock.lock().await;
    if !leave_checkout {
        ensure_clean(dir, "getting the latest").await?;
    }
    let current = run(dir, &["branch", "--show-current"])
        .await?
        .trim()
        .to_string();
    let main = rev_parse(dir, "refs/heads/main").await?;
    let mut update = Update::default();
    let mut plan: Vec<RefMove> = Vec::new();

    // `base`: what the change branches are brought up to.
    let base = if has_remote(dir, "origin").await? {
        run_env(dir, &["fetch", "origin", "main"], env).await?;
        let remote = rev_parse(dir, "refs/remotes/origin/main").await?;
        if remote == main || is_ancestor(dir, &remote, &main).await {
            main.clone()
        } else if is_ancestor(dir, &main, &remote).await {
            update.main_updated = true;
            plan.push(RefMove::new("main", &main, &remote));
            remote
        } else {
            update.main_diverged = true;
            remote
        }
    } else {
        main.clone()
    };

    for branch in change_branches(dir).await? {
        let tip = rev_parse(dir, &format!("refs/heads/{branch}")).await?;
        let planned = plan_merge(dir, &tip, &base, branch == current).await?;
        let change = || async {
            Change {
                title: branch_title(dir, &branch).await,
                branch: branch.clone(),
                last_edited: None,
            }
        };
        match planned {
            Planned::UpToDate => {}
            _ if leave_checkout && branch == current => update.skipped = Some(change().await),
            Planned::Conflict => update.conflicts.push(change().await),
            Planned::Advance(target) => {
                plan.push(RefMove::new(&branch, &tip, &target));
                update.updated.push(change().await);
            }
        }
    }

    if leave_checkout && plan.iter().any(|m| m.branch == current) {
        // Only `main` can still be here: the checked-out branch was skipped above.
        plan.retain(|m| m.branch != current);
        update.main_updated = false;
        update.skipped = Some(Change {
            branch: current.clone(),
            title: branch_title(dir, &current).await,
            last_edited: None,
        });
    }

    apply(dir, &current, &plan).await?;
    Ok(update)
}

/// Local `content/*` branch names.
async fn change_branches(dir: &Path) -> Result<Vec<String>, ApiError> {
    let out = run(
        dir,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/content/",
        ],
    )
    .await?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|b| b.starts_with("content/"))
        .map(str::to_string)
        .collect())
}

/// Move `refs/heads/{branch}` from `old` to `new`.
struct RefMove {
    branch: String,
    old: String,
    new: String,
}

impl RefMove {
    fn new(branch: &str, old: &str, new: &str) -> Self {
        Self {
            branch: branch.to_string(),
            old: old.to_string(),
            new: new.to_string(),
        }
    }
}

/// What getting `base` into a branch takes. Computing it writes objects but no refs.
enum Planned {
    UpToDate,
    Advance(String),
    Conflict,
}

/// Merge `base` into `tip` without a checkout: `merge-tree` computes the tree and
/// `commit-tree` records it with both parents. Conflicts are detected here, so Slice 9 can
/// resolve them on the same `merge-tree` result.
async fn plan_merge(dir: &Path, tip: &str, base: &str, active: bool) -> Result<Planned, ApiError> {
    if is_ancestor(dir, base, tip).await {
        return Ok(Planned::UpToDate);
    }
    if is_ancestor(dir, tip, base).await {
        // Already merged into main (not an open change) — unless it is the active change with
        // no work of its own yet, which simply catches up.
        return Ok(if active {
            Planned::Advance(base.to_string())
        } else {
            Planned::UpToDate
        });
    }
    let Some(tree) = merge_tree(dir, tip, base).await? else {
        return Ok(Planned::Conflict);
    };
    let commit = run(
        dir,
        &[
            "commit-tree",
            &tree,
            "-p",
            tip,
            "-p",
            base,
            "-m",
            "Get latest from main",
        ],
    )
    .await?;
    Ok(Planned::Advance(commit.trim().to_string()))
}

/// Apply `plan`: refuse first if the checked-out branch's update would clobber untracked
/// files; then move every other ref in one atomic transaction; then fast-forward the
/// checked-out branch (rolling the transaction back if that fails).
async fn apply(dir: &Path, current: &str, plan: &[RefMove]) -> Result<(), ApiError> {
    let (active, others): (Vec<&RefMove>, Vec<&RefMove>) =
        plan.iter().partition(|m| m.branch == current);
    if let Some(m) = active.first() {
        ensure_no_untracked_in_the_way(dir, &m.new).await?;
    }
    update_refs(
        dir,
        others.iter().map(|m| (m.branch.as_str(), &m.new, &m.old)),
    )
    .await?;
    if let Some(m) = active.first() {
        if let Err(e) = run(dir, &["merge", "--ff-only", &m.new]).await {
            let _ = update_refs(
                dir,
                others.iter().map(|m| (m.branch.as_str(), &m.old, &m.new)),
            )
            .await;
            return Err(e);
        }
    }
    Ok(())
}

/// Untracked (non-ignored) files the fast-forward to `target` would overwrite — `ensure_clean`
/// ignores untracked files, but `merge --ff-only` refuses to clobber them.
async fn ensure_no_untracked_in_the_way(dir: &Path, target: &str) -> Result<(), ApiError> {
    let untracked = run(dir, &["ls-files", "--others", "--exclude-standard"]).await?;
    if untracked.trim().is_empty() {
        return Ok(());
    }
    let incoming = run(
        dir,
        &["diff", "--name-only", "--diff-filter=A", "HEAD", target],
    )
    .await?;
    let incoming: std::collections::HashSet<&str> = incoming.lines().map(str::trim).collect();
    let blocking: Vec<&str> = untracked
        .lines()
        .map(str::trim)
        .filter(|f| incoming.contains(f))
        .collect();
    if blocking.is_empty() {
        return Ok(());
    }
    Err(ApiError::conflict(format!(
        "The latest main adds files you already have uncommitted copies of: {} — move or delete them, then try again",
        blocking.join(", ")
    )))
}

/// Move `(branch, new, old)` refs in one atomic `git update-ref --stdin` transaction.
async fn update_refs<'a>(
    dir: &Path,
    moves: impl Iterator<Item = (&'a str, &'a String, &'a String)>,
) -> Result<(), ApiError> {
    use tokio::io::AsyncWriteExt;
    let mut input = String::from("start\n");
    let mut any = false;
    for (branch, new, old) in moves {
        input.push_str(&format!("update refs/heads/{branch} {new} {old}\n"));
        any = true;
    }
    if !any {
        return Ok(());
    }
    input.push_str("prepare\ncommit\n");
    let mut child = Command::new("git")
        .args(["update-ref", "--stdin"])
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| ApiError::git(format!("failed to run git: {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|e| ApiError::git(format!("failed to write to git: {e}")))?;
    }
    let out = child
        .wait_with_output()
        .await
        .map_err(|e| ApiError::git(format!("failed to run git: {e}")))?;
    if !out.status.success() {
        return Err(ApiError::git(format!(
            "could not update the change branches (nothing was changed): {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

/// Merged tree of `ours` and `theirs`, or `None` when they conflict.
async fn merge_tree(dir: &Path, ours: &str, theirs: &str) -> Result<Option<String>, ApiError> {
    let out = Command::new("git")
        .args(["merge-tree", "--write-tree", ours, theirs])
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| ApiError::git(format!("failed to run git: {e}")))?;
    match out.status.code() {
        Some(0) => Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .map(|l| l.trim().to_string())),
        Some(1) => Ok(None),
        _ => Err(ApiError::git(format!(
            "git merge-tree failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ))),
    }
}

async fn rev_parse(dir: &Path, rev: &str) -> Result<String, ApiError> {
    Ok(run(dir, &["rev-parse", "--verify", rev])
        .await?
        .trim()
        .to_string())
}

/// What GitHub says about a change's pull request, for [`cleanup`].
#[derive(Debug)]
pub enum PrMerge {
    /// The PR is merged; `head_sha` is the commit it merged.
    Merged { head_sha: String },
    /// No PR, or one that isn't merged.
    NotMerged,
    /// GitHub couldn't be asked; the error is returned if git alone can't decide.
    Unknown(ApiError),
}

/// Result of a [`cleanup`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cleaned {
    /// `false` when `origin` couldn't be reached, so `main` is as it was locally (always
    /// `true` for a local-only project, whose `main` is already the latest).
    pub main_updated: bool,
}

/// The cheap local checks of [`cleanup`]: a `content/*` branch that exists, and a clean tree.
pub async fn check_cleanup(dir: &Path, branch: &str) -> Result<(), ApiError> {
    if !branch.starts_with("content/") {
        return Err(ApiError::badreq("branch must be a content/* change"));
    }
    if !branch_exists(dir, branch).await? {
        return Err(ApiError::enoent(format!("no such change: {branch}")));
    }
    ensure_clean(dir, "cleaning up").await
}

/// Whether a pull request whose head is `head_sha` is about `branch`'s work: the head is the
/// branch tip or one of its ancestors. A PR for an earlier change that had the same name is not.
pub async fn pr_matches(dir: &Path, branch: &str, head_sha: &str) -> bool {
    !head_sha.is_empty() && is_ancestor(dir, head_sha, &format!("refs/heads/{branch}")).await
}

async fn is_ancestor(dir: &Path, ancestor: &str, descendant: &str) -> bool {
    run(dir, &["merge-base", "--is-ancestor", ancestor, descendant])
        .await
        .is_ok()
}

/// Clean up a merged change: bring local `main` up to date, check it out, then delete the
/// local `branch` (the remote branch is left alone).
///
/// The branch counts as merged when its tip is exactly the head its merged PR merged (covers
/// squash merges), or when it has commits of its own and the latest `main` contains them.
/// Anything else is refused — including edits checkpointed after the merge — and nothing is
/// deleted unless `main` fast-forwarded first. If `origin` can't be reached, cleanup still
/// goes ahead when the PR proves the merge, leaving `main` as it was ([`Cleaned::main_updated`]).
pub async fn cleanup(
    dir: &Path,
    branch: &str,
    pr: PrMerge,
    env: &[(String, String)],
) -> Result<Cleaned, ApiError> {
    check_cleanup(dir, branch).await?;
    let tip = run(dir, &["rev-parse", &format!("refs/heads/{branch}")])
        .await?
        .trim()
        .to_string();
    let merged_head = match &pr {
        PrMerge::Merged { head_sha } => Some(head_sha.as_str()),
        _ => None,
    };
    let merged_by_pr = merged_head == Some(tip.as_str());

    let remote = has_remote(dir, "origin").await?;
    let fetched = if remote {
        match run_env(dir, &["fetch", "origin", "main"], env).await {
            Ok(_) => true,
            Err(_) if merged_by_pr => false,
            Err(e) => {
                return Err(ApiError::new(
                    e.code,
                    format!(
                        "Couldn't get the latest main to check this change: {}",
                        e.message
                    ),
                ))
            }
        }
    } else {
        false
    };
    let latest = if fetched { "origin/main" } else { "main" };

    if !merged_by_pr && !merged_into(dir, &tip, latest).await? {
        return Err(match pr {
            PrMerge::Unknown(e) => e,
            PrMerge::Merged { head_sha } if pr_matches(dir, branch, &head_sha).await => {
                ApiError::conflict(
                    "This change has edits made after its pull request was merged — \
                     share them before cleaning up",
                )
            }
            _ => ApiError::conflict(
                "This change has not been merged yet — merge its pull request first",
            ),
        });
    }

    // Make sure main can fast-forward before touching the working tree.
    if fetched && !is_ancestor(dir, "refs/heads/main", latest).await {
        return Err(ApiError::conflict(
            "Local main has commits that aren't on GitHub, so it can't be updated",
        ));
    }
    let original = run(dir, &["branch", "--show-current"])
        .await?
        .trim()
        .to_string();
    run(dir, &["checkout", "main"]).await?;
    if fetched {
        if let Err(e) = run(dir, &["merge", "--ff-only", latest]).await {
            if !original.is_empty() && original != "main" {
                let _ = run(dir, &["checkout", &original]).await;
            }
            return Err(e);
        }
    }
    // `-D`: a squash-merged tip is never an ancestor of `main`.
    run(dir, &["branch", "-D", branch]).await?;
    Ok(Cleaned {
        main_updated: fetched || !remote,
    })
}

/// `tip` is in `main_ref` and is not on its first-parent line — i.e. the branch had commits of
/// its own that were merged, rather than being a branch that never moved off `main`.
async fn merged_into(dir: &Path, tip: &str, main_ref: &str) -> Result<bool, ApiError> {
    if !is_ancestor(dir, tip, main_ref).await {
        return Ok(false);
    }
    let line = run(dir, &["rev-list", "--first-parent", main_ref]).await?;
    Ok(!line.lines().any(|c| c.trim() == tip))
}

async fn branch_title(dir: &Path, branch: &str) -> String {
    match run(
        dir,
        &["config", "--get", &format!("branch.{branch}.description")],
    )
    .await
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
        if lower.contains("authentication")
            || lower.contains("401")
            || lower.contains("credentials")
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
