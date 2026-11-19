//! GitHub sign-in, the signed-in account, and repository access.
//!
//! Two sources, in order: a device-flow session saved by this daemon (`oauth`), else the
//! machine's GitHub CLI login (`cli`). The CLI token is never read: the daemon only runs
//! `gh api ...` and lets git call `gh auth git-credential`.
//!
//! The session (token + profile) lives in `{projects-dir}/.orso/github.json`, mode 0600.
//! The token is sent to the GitHub API as a bearer token and to git per command through
//! `GIT_CONFIG_*` environment variables, so it never appears in argv or in a repo's config.

use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use base64::Engine;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::config::GithubConfig;
use crate::error::ApiError;
use crate::git::Identity;

const SESSION_FILE: &str = "github.json";
/// `repo`: clone and push (private repos included). `read:user`: profile name for authorship.
const SCOPES: &str = "repo read:user";
const FORM: &str = "application/x-www-form-urlencoded";
const GITHUB_JSON: &str = "application/vnd.github+json";
/// How long a successful `gh api user` is reused.
const CLI_TTL: Duration = Duration::from_secs(60);
/// Logged-out or failed results are re-checked soon: the user may be about to run `gh auth login`.
const CLI_RETRY: Duration = Duration::from_secs(5);
/// Retry delay while serving the last good login through a transient `gh` failure.
const CLI_STALE_RETRY: Duration = Duration::from_secs(10);
/// GitHub's largest page size for list endpoints.
const PAGE_SIZE: usize = 100;
/// Stop paginating after this many pages (10k items).
const MAX_PAGES: usize = 100;
const REPOS_PATH: &str = "/user/repos?per_page=100&sort=pushed";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubUser {
    pub login: String,
    pub id: u64,
    pub name: Option<String>,
    pub avatar_url: String,
}

impl GithubUser {
    /// Commit identity: profile name (or login) and GitHub's noreply address.
    pub fn identity(&self) -> Identity {
        let name = self
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .unwrap_or(&self.login)
            .to_string();
        Identity {
            name,
            email: format!("{}+{}@users.noreply.github.com", self.id, self.login),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub configured: bool,
    /// A client id is configured, so in-app (device flow) sign-in is offered.
    pub device_flow: bool,
    /// `"oauth"` (device-flow session) or `"cli"` (GitHub CLI login).
    pub source: Option<&'static str>,
    pub user: Option<GithubUser>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStart {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// One poll of the device flow. Serialized as `{ "status": "pending" | "slowDown" | … }`.
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum PollOutcome {
    Pending,
    SlowDown { interval: u64 },
    SignedIn { user: GithubUser },
    Expired,
    Denied,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub full_name: String,
    pub private: bool,
    pub default_branch: String,
    pub can_push: bool,
    /// ISO 8601 timestamp of the last push, when GitHub reports one.
    pub pushed_at: Option<String>,
}

/// A pull request returned by [`GithubAuth::find_pr`] / [`GithubAuth::create_draft_pr`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub html_url: String,
    pub title: String,
    pub draft: bool,
    /// GraphQL node id, for mutations; never sent to the web UI.
    #[serde(skip)]
    pub node_id: String,
}

/// How long Share preview waits for the branch protection lookup before giving up.
const PROTECTION_TIMEOUT: Duration = Duration::from_secs(3);

/// Whether `main` has a branch protection rule, from [`GithubAuth::main_protection`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchProtection {
    Protected,
    Unprotected,
    /// The check failed or we lack admin rights to read it (403): say nothing.
    Unknown,
}

/// Draft PR title is the change's display title.
pub fn pr_title(change_title: &str) -> String {
    change_title.trim().to_string()
}

/// Draft PR body: the writer's description, or a line naming the change when there is none.
pub fn pr_body(change_title: &str, description: Option<&str>) -> String {
    match description.map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => d.to_string(),
        None => format!("Preview of change: {}", change_title.trim()),
    }
}

impl From<ApiRepo> for RepoInfo {
    fn from(r: ApiRepo) -> Self {
        RepoInfo {
            full_name: r.full_name,
            private: r.private,
            default_branch: r.default_branch,
            can_push: r.permissions.is_some_and(|p| p.push),
            pushed_at: r.pushed_at,
        }
    }
}

#[derive(Deserialize)]
struct ApiUser {
    login: String,
    id: u64,
    name: Option<String>,
    avatar_url: String,
}

#[derive(Deserialize)]
struct ApiRepo {
    full_name: String,
    private: bool,
    default_branch: String,
    #[serde(default)]
    permissions: Option<ApiPermissions>,
    #[serde(default)]
    pushed_at: Option<String>,
}

#[derive(Deserialize)]
struct ApiPermissions {
    #[serde(default)]
    push: bool,
}

#[derive(Deserialize)]
struct ApiPull {
    number: u64,
    /// GraphQL node id (for mutations such as `markPullRequestReadyForReview`).
    #[serde(default)]
    node_id: String,
    html_url: String,
    title: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    merged_at: Option<String>,
    /// GitHub's verdict on merging: null until it has computed one.
    #[serde(default)]
    mergeable: Option<bool>,
    /// `clean`, `dirty`, `blocked`, `behind`, `unstable`, `draft`, …
    #[serde(default)]
    mergeable_state: Option<String>,
    #[serde(default)]
    head: Option<ApiPullHead>,
    #[serde(default)]
    labels: Vec<ApiLabel>,
    /// Null when the author's account was deleted.
    #[serde(default)]
    user: Option<ApiLogin>,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    closed_at: Option<String>,
    #[serde(default)]
    created_at: String,
    /// Reviewers asked for and not yet heard from.
    #[serde(default)]
    requested_reviewers: Vec<ApiLogin>,
}

#[derive(Deserialize)]
struct ApiLabel {
    name: String,
    /// Hex colour without `#`.
    #[serde(default)]
    color: String,
}

#[derive(Deserialize)]
struct ApiPullHead {
    #[serde(default)]
    sha: String,
}

/// Response of `PUT /repos/{repo}/pulls/{number}/merge`.
#[derive(Deserialize)]
struct ApiMerge {
    #[serde(default)]
    merged: bool,
    /// The commit the merge produced on the base branch (the one that gets deployed).
    #[serde(default)]
    sha: Option<String>,
}

/// Where a pull request stands, as shown in the preview browser bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PrState {
    Draft,
    Open,
    /// A reviewer's latest decisive review asks for changes.
    #[serde(rename = "changes_requested")]
    ChangesRequested,
    Approved,
    Merged,
    Closed,
}

/// Check runs on the PR head, aggregated: any failing → failing, else any pending → pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChecksState {
    /// No check runs reported.
    None,
    Pending,
    Passing,
    Failing,
}

/// Whether the deploy that runs on a published commit has shipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeployState {
    /// No deploy check on the commit (yet): a site without CI, or a run GitHub hasn't queued.
    Unknown,
    Building,
    Live,
    Failed,
}

/// `GET /api/changes/{branch}/status` payload for a branch with a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrStatus {
    pub number: u64,
    pub url: String,
    pub state: PrState,
    pub checks: ChecksState,
    /// `environment_url` of the latest successful deployment of the head SHA.
    pub preview_url: Option<String>,
    /// Whether GitHub can merge the PR as it stands; null until it has computed that.
    pub mergeable: Option<bool>,
    /// GitHub's finer-grained merge state (`clean`, `dirty`, `blocked`, `behind`, …).
    pub mergeable_state: Option<String>,
    /// The local branch tip is the PR head: nothing new to push.
    pub up_to_date: bool,
    /// Why a reviewer sent the change back; set while the state is `changes_requested`.
    pub sent_back: Option<SentBack>,
    /// The PR's head commit (ties the PR to the local branch's commits; not sent to the UI).
    #[serde(skip)]
    pub head_sha: String,
}

/// A reviewer's request for changes: who, and what they wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SentBack {
    pub reviewer: String,
    pub comment: String,
}

/// What a reviewer decides about a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Greenlight,
    SendBack,
}

/// A pull request's [`PrState`] and head commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrRef {
    pub state: PrState,
    pub head_sha: String,
}

/// Result of [`GithubAuth::merge_pr`] (the Publish action): the change's pull request, now
/// merged into the live site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    pub number: u64,
    pub url: String,
    pub state: PrState,
    /// The commit the merge produced on the base branch; the deploy that ships the change
    /// runs on it. `None` when GitHub's answer didn't include one.
    pub merge_sha: Option<String>,
}

/// The parts of a pull request that decide its [`PrState`] (besides reviews).
struct PullFacts {
    draft: bool,
    closed: bool,
    merged: bool,
}

#[derive(Deserialize)]
struct ApiLogin {
    login: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct ApiSha {
    sha: String,
}

#[derive(Deserialize)]
struct ApiRef {
    object: ApiSha,
}

#[derive(Deserialize)]
struct ApiGitCommit {
    tree: ApiSha,
}

#[derive(Deserialize)]
struct ApiTree {
    #[serde(default)]
    tree: Vec<ApiTreeEntry>,
}

#[derive(Deserialize)]
struct ApiTreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
}

/// What [`GithubAuth::clean_review_assets`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetsCleanup {
    Nothing,
    /// These pull requests' images were removed in one commit.
    Removed(Vec<u64>),
    /// No kept pull request had images left: the branch is gone.
    DeletedBranch,
}

#[derive(Deserialize)]
struct ApiReview {
    #[serde(default)]
    user: Option<ApiLogin>,
    /// The review on GitHub.
    #[serde(default)]
    html_url: Option<String>,
    state: String,
    #[serde(default)]
    commit_id: String,
    /// The review comment.
    #[serde(default)]
    body: Option<String>,
    /// Unset while the review is pending.
    #[serde(default)]
    submitted_at: Option<String>,
}

/// `GET /repos/{o}/{r}/issues` item: just its conversation's comment count.
#[derive(Deserialize)]
struct ApiIssueCount {
    number: u64,
    #[serde(default)]
    comments: u64,
}

/// `items` with their comment counts filled in from `counts`.
fn with_comments(items: Vec<ReviewItem>, counts: &HashMap<u64, u64>) -> Vec<ReviewItem> {
    items
        .into_iter()
        .map(|mut i| {
            i.comments = counts.get(&i.number).copied().unwrap_or(0);
            i
        })
        .collect()
}

/// `GET /repos/{o}/{r}/issues/{n}/comments` item.
#[derive(Deserialize)]
struct ApiIssueComment {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    user: Option<ApiLogin>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: String,
}

/// `GET /repos/{o}/{r}/pulls/{n}/commits` item.
#[derive(Deserialize)]
struct ApiPullCommit {
    sha: String,
    commit: ApiCommitInfo,
    /// The GitHub account, when the commit's email maps to one.
    #[serde(default)]
    author: Option<ApiLogin>,
}

#[derive(Deserialize)]
struct ApiCommitInfo {
    #[serde(default)]
    message: String,
    #[serde(default)]
    author: Option<ApiCommitPerson>,
    #[serde(default)]
    committer: Option<ApiCommitPerson>,
}

#[derive(Deserialize)]
struct ApiCommitPerson {
    #[serde(default)]
    name: String,
    #[serde(default)]
    date: String,
}

/// The signed-in reviewer's standing on a pull request's current head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MyReviewState {
    New,
    SentBack,
    Greenlit,
}

/// One change in the review inbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub url: String,
    pub updated_at: String,
    pub head_sha: String,
    pub checks: ChecksState,
    /// Check runs that passed (success, neutral or skipped), of `checks_total`.
    pub checks_passed: u64,
    pub checks_total: u64,
    pub my_state: MyReviewState,
    /// `me` opened it: shown for viewing, but GitHub will not take a decision from its author.
    pub mine: bool,
    /// Labels as GitHub shows them.
    pub labels: Vec<ReviewLabel>,
    pub created_at: String,
    /// Conversation comments (best effort: 0 when GitHub couldn't say).
    pub comments: u64,
    /// `open`, `closed` or `merged`.
    pub state: String,
    pub draft: bool,
}

/// Labelled closed or merged changes, most recently updated first; `more` when GitHub had
/// a full page, so there may be older ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClosedReviews {
    pub reviews: Vec<ReviewItem>,
    pub more: bool,
}

/// Closed changes listed at most (one page).
pub const CLOSED_PAGE: usize = 30;

/// A single change opened for review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDetail {
    #[serde(flatten)]
    pub item: ReviewItem,
    pub description: Option<String>,
    pub open: bool,
    pub draft: bool,
    pub merged: bool,
    /// The pull request's label names (server-side gating; the UI reads `item.labels`).
    #[serde(skip)]
    pub labels: Vec<String>,
}

/// A label on a pull request, with its GitHub colour (hex, no `#`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewLabel {
    pub name: String,
    pub color: String,
}

/// How a review reads in a change's conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Greenlit,
    SentBack,
    Commented,
}

/// One pushed commit: short sha and the first line of its message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PushedCommit {
    pub sha: String,
    pub message: String,
    pub author: String,
}

/// An entry of a change's conversation, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TimelineItem {
    Comment {
        id: u64,
        author: String,
        /// The author's GitHub avatar.
        #[serde(rename = "avatarUrl", skip_serializing_if = "Option::is_none")]
        avatar_url: Option<String>,
        at: String,
        body: String,
        url: String,
    },
    Review {
        author: String,
        #[serde(rename = "avatarUrl", skip_serializing_if = "Option::is_none")]
        avatar_url: Option<String>,
        at: String,
        verdict: ReviewVerdict,
        body: Option<String>,
        /// The review on GitHub.
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
    /// Consecutive commits with nothing else between them.
    Push {
        at: String,
        commits: Vec<PushedCommit>,
    },
    /// The pull request was closed or merged (`state`).
    State { at: String, state: String },
}

impl TimelineItem {
    fn at(&self) -> &str {
        match self {
            Self::Comment { at, .. }
            | Self::Review { at, .. }
            | Self::Push { at, .. }
            | Self::State { at, .. } => at,
        }
    }
}

/// One check run on the change's head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckItem {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub url: Option<String>,
}

/// Where a reviewer stands: their latest decisive review, else `commented`, else `requested`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reviewer {
    pub login: String,
    pub state: ReviewerState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerState {
    Requested,
    Commented,
    Greenlit,
    SentBack,
    Dismissed,
}

/// `GET /api/projects/{id}/reviews/{n}/conversation`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conversation {
    pub items: Vec<TimelineItem>,
    pub checks: Vec<CheckItem>,
    pub reviewers: Vec<Reviewer>,
}

/// Comments, reviews, pushes and the close/merge, oldest first, with consecutive commits
/// grouped into one push dated by its latest commit. Pending reviews and empty comment-only
/// reviews are left out.
fn timeline(
    comments: Vec<ApiIssueComment>,
    reviews: &[ApiReview],
    commits: Vec<ApiPullCommit>,
    closed: Option<(String, bool)>,
) -> Vec<TimelineItem> {
    let login = |u: Option<&ApiLogin>| u.map_or_else(|| "ghost".to_string(), |u| u.login.clone());
    let avatar = |u: Option<&ApiLogin>| {
        u.and_then(|u| u.avatar_url.clone())
            .filter(|a| is_http_url(a))
    };
    let mut raw: Vec<TimelineItem> = Vec::new();
    for c in comments {
        raw.push(TimelineItem::Comment {
            id: c.id,
            author: login(c.user.as_ref()),
            avatar_url: avatar(c.user.as_ref()),
            at: c.created_at,
            body: c.body.unwrap_or_default(),
            url: c.html_url,
        });
    }
    for r in reviews {
        let Some(at) = r.submitted_at.clone() else {
            continue;
        };
        let body = r.body.clone().filter(|b| !b.trim().is_empty());
        let verdict = match r.state.as_str() {
            "APPROVED" => ReviewVerdict::Greenlit,
            "CHANGES_REQUESTED" => ReviewVerdict::SentBack,
            "COMMENTED" if body.is_some() => ReviewVerdict::Commented,
            _ => continue,
        };
        raw.push(TimelineItem::Review {
            author: login(r.user.as_ref()),
            avatar_url: avatar(r.user.as_ref()),
            at,
            verdict,
            body,
            url: r.html_url.clone().filter(|u| is_http_url(u)),
        });
    }
    for c in commits {
        let person = c.commit.committer.as_ref().or(c.commit.author.as_ref());
        let at = person.map(|p| p.date.clone()).unwrap_or_default();
        // The GitHub account, else the name git recorded.
        let author = c.author.map(|a| a.login).unwrap_or_else(|| {
            c.commit
                .author
                .as_ref()
                .or(c.commit.committer.as_ref())
                .map(|p| p.name.clone())
                .unwrap_or_default()
        });
        raw.push(TimelineItem::Push {
            at,
            commits: vec![PushedCommit {
                sha: c.sha.chars().take(7).collect(),
                message: c.commit.message.lines().next().unwrap_or("").to_string(),
                author,
            }],
        });
    }
    if let Some((at, merged)) = closed {
        let state = if merged { "merged" } else { "closed" };
        raw.push(TimelineItem::State {
            at,
            state: state.to_string(),
        });
    }
    // ISO 8601 UTC timestamps sort as strings; the sort is stable, so ties keep their order.
    raw.sort_by(|a, b| a.at().cmp(b.at()));
    let mut items: Vec<TimelineItem> = Vec::new();
    for item in raw {
        match (items.last_mut(), item) {
            (
                Some(TimelineItem::Push { commits, at }),
                TimelineItem::Push {
                    commits: more,
                    at: later,
                },
            ) => {
                // Dated by its latest commit, so the group sorts where it last moved.
                commits.extend(more);
                *at = later;
            }
            (_, item) => items.push(item),
        }
    }
    items
}

/// Each reviewer's latest standing; requested reviewers who haven't answered come last.
fn reviewers(reviews: &[ApiReview], requested: &[ApiLogin]) -> Vec<Reviewer> {
    let mut out: Vec<Reviewer> = Vec::new();
    for r in reviews.iter().filter(|r| r.submitted_at.is_some()) {
        let Some(user) = r.user.as_ref() else {
            continue;
        };
        let state = match r.state.as_str() {
            "APPROVED" => ReviewerState::Greenlit,
            "CHANGES_REQUESTED" => ReviewerState::SentBack,
            "DISMISSED" => ReviewerState::Dismissed,
            "COMMENTED" => ReviewerState::Commented,
            _ => continue,
        };
        match out
            .iter_mut()
            .find(|o| o.login.eq_ignore_ascii_case(&user.login))
        {
            // A comment doesn't undo an earlier decision.
            Some(_) if state == ReviewerState::Commented => {}
            Some(o) => o.state = state,
            None => out.push(Reviewer {
                login: user.login.clone(),
                state,
            }),
        }
    }
    for r in requested {
        if !out.iter().any(|o| o.login.eq_ignore_ascii_case(&r.login)) {
            out.push(Reviewer {
                login: r.login.clone(),
                state: ReviewerState::Requested,
            });
        }
    }
    out
}

/// The review inbox plus every open pull request's number, labelled or not (what prunes
/// cached refs and previews, so a change opened by direct URL keeps its preview).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewInbox {
    pub reviews: Vec<ReviewItem>,
    pub open: Vec<u64>,
}

/// GitHub requests in flight at once while building the review inbox.
const INBOX_CONCURRENCY: usize = 4;

/// `me`'s latest decisive review (approved / changes requested / dismissed) on `head_sha`:
/// approved -> greenlit, changes requested -> sent back, anything else -> new.
fn my_review_state(reviews: &[ApiReview], me: &str, head_sha: &str) -> MyReviewState {
    let latest = reviews.iter().rfind(|r| {
        r.user
            .as_ref()
            .is_some_and(|u| u.login.eq_ignore_ascii_case(me))
            && r.commit_id == head_sha
            && matches!(
                r.state.as_str(),
                "APPROVED" | "CHANGES_REQUESTED" | "DISMISSED"
            )
    });
    match latest.map(|r| r.state.as_str()) {
        Some("APPROVED") => MyReviewState::Greenlit,
        Some("CHANGES_REQUESTED") => MyReviewState::SentBack,
        _ => MyReviewState::New,
    }
}

#[derive(Deserialize)]
struct ApiCheckRuns {
    #[serde(default)]
    check_runs: Vec<ApiCheckRun>,
}

#[derive(Deserialize)]
struct ApiCheckRun {
    #[serde(default)]
    name: String,
    status: String,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Deserialize)]
struct ApiDeployment {
    id: u64,
    #[serde(default)]
    sha: String,
}

#[derive(Deserialize)]
struct ApiDeploymentStatus {
    state: String,
    #[serde(default)]
    environment_url: Option<String>,
}

#[derive(Deserialize)]
struct ApiBranch {
    commit: ApiBranchCommit,
}

#[derive(Deserialize)]
struct ApiBranchCommit {
    sha: String,
}

/// Merged / closed win; then draft; then approved when every reviewer's latest decisive review
/// (approve, request changes, dismissed) leaves at least one approval and no change request.
fn pr_state(pull: &PullFacts, reviews: &[ApiReview]) -> PrState {
    if pull.merged {
        return PrState::Merged;
    }
    if pull.closed {
        return PrState::Closed;
    }
    if pull.draft {
        return PrState::Draft;
    }
    let latest = latest_decisions(reviews);
    if latest.iter().any(|r| r.state == "CHANGES_REQUESTED") {
        PrState::ChangesRequested
    } else if latest.iter().any(|r| r.state == "APPROVED") {
        PrState::Approved
    } else {
        PrState::Open
    }
}

/// Each reviewer's latest decisive review (approve, request changes, dismissed).
fn latest_decisions(reviews: &[ApiReview]) -> Vec<&ApiReview> {
    // Reviews arrive oldest first; a later one replaces the reviewer's earlier one.
    let mut latest: Vec<&ApiReview> = Vec::new();
    for r in reviews {
        if !matches!(
            r.state.as_str(),
            "APPROVED" | "CHANGES_REQUESTED" | "DISMISSED"
        ) {
            continue;
        }
        let who = r.user.as_ref().map_or("", |u| u.login.as_str());
        match latest
            .iter_mut()
            .find(|e| e.user.as_ref().map_or("", |u| u.login.as_str()) == who)
        {
            Some(entry) => *entry = r,
            None => latest.push(r),
        }
    }
    latest
}

/// The newest outstanding change request, with its comment.
fn sent_back(reviews: &[ApiReview]) -> Option<SentBack> {
    let outstanding: Vec<&ApiReview> = latest_decisions(reviews)
        .into_iter()
        .filter(|r| r.state == "CHANGES_REQUESTED")
        .collect();
    // `reviews` is oldest first: the outstanding request that appears last is the newest.
    let newest = reviews
        .iter()
        .rfind(|r| outstanding.iter().any(|o| std::ptr::eq(*o, *r)))?;
    Some(SentBack {
        reviewer: newest
            .user
            .as_ref()
            .map_or_else(String::new, |u| u.login.clone()),
        comment: newest.body.clone().unwrap_or_default(),
    })
}

/// Only `http://` / `https://` URLs may become links (a deployment's `environment_url` is
/// whatever the workflow wrote, e.g. `javascript:`).
fn is_http_url(u: &str) -> bool {
    let lower = u.get(..8).unwrap_or(u).to_ascii_lowercase();
    (lower.starts_with("http://") && u.len() > 7) || (lower.starts_with("https://") && u.len() > 8)
}

/// Cloudflare Workers preview hosts are `{alias}-{worker}.{account}.workers.dev`. Strip the
/// alias using the known worker name to recover the production host.
fn production_url_from_preview(preview: &str, worker: &str) -> Option<String> {
    if worker.is_empty() || !is_http_url(preview) {
        return None;
    }
    let (scheme, rest) = preview.split_once("://")?;
    let host = rest.split('/').next().unwrap_or(rest);
    let needle = format!("-{worker}.");
    let idx = host.rfind(&needle)?;
    let prod_host = &host[idx + 1..];
    if !prod_host.starts_with(&format!("{worker}.")) {
        return None;
    }
    Some(format!("{scheme}://{prod_host}"))
}

/// `name = "..."` from a project's `wrangler.toml`, when present.
pub fn worker_name_from_wrangler(dir: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("wrangler.toml")).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("name") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim();
        let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let inner = rest.trim_start_matches(quote).split(quote).next()?;
        if !inner.is_empty() {
            return Some(inner.to_string());
        }
    }
    None
}

fn aggregate_checks(runs: &[ApiCheckRun]) -> ChecksState {
    if runs.is_empty() {
        return ChecksState::None;
    }
    let failing = runs.iter().any(|r| {
        r.status == "completed"
            && !matches!(
                r.conclusion.as_deref(),
                Some("success" | "neutral" | "skipped")
            )
    });
    if failing {
        ChecksState::Failing
    } else if runs.iter().any(|r| r.status != "completed") {
        ChecksState::Pending
    } else {
        ChecksState::Passing
    }
}

/// A check run that deploys: GitHub Pages names its own "pages build and deployment", and
/// deploy workflows are named for it ("deploy", "Deploy Cloudflare Workers", …).
fn is_deploy_check(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["deploy", "pages", "publish", "release"]
        .iter()
        .any(|word| name.contains(word))
}

/// [`DeployState`] from a commit's check runs. A failing deploy wins over one still running;
/// checks that aren't deploys (tests, lint) are ignored, so their failures never read as a
/// failed deploy. No deploy check at all stays [`DeployState::Unknown`].
fn deploy_state_of(runs: &[ApiCheckRun]) -> DeployState {
    let deploys: Vec<&ApiCheckRun> = runs.iter().filter(|r| is_deploy_check(&r.name)).collect();
    if deploys.is_empty() {
        return DeployState::Unknown;
    }
    let failed = deploys.iter().any(|r| {
        r.status == "completed"
            && !matches!(
                r.conclusion.as_deref(),
                Some("success" | "neutral" | "skipped")
            )
    });
    if failed {
        return DeployState::Failed;
    }
    if deploys.iter().any(|r| r.status != "completed") {
        return DeployState::Building;
    }
    DeployState::Live
}

impl From<ApiPull> for PullRequest {
    fn from(p: ApiPull) -> Self {
        Self {
            number: p.number,
            html_url: p.html_url,
            title: p.title,
            draft: p.draft,
            node_id: p.node_id,
        }
    }
}

/// A GraphQL response: `errors` may come with HTTP 200.
#[derive(Deserialize)]
struct ApiGraphql {
    #[serde(default)]
    data: Option<ApiMarkReadyData>,
    #[serde(default)]
    errors: Vec<ApiGraphqlError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiMarkReadyData {
    #[serde(default)]
    mark_pull_request_ready_for_review: Option<ApiMarkReadyPayload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiMarkReadyPayload {
    #[serde(default)]
    pull_request: Option<ApiDraftFlag>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiDraftFlag {
    is_draft: bool,
}

/// GraphQL error types that mean the token may not do this (vs. a GitHub-side failure).
const PERMISSION_ERRORS: &[&str] = &["FORBIDDEN", "INSUFFICIENT_SCOPES", "UNAUTHORIZED"];

#[derive(Deserialize)]
struct ApiGraphqlError {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: String,
}

const MARK_READY_MUTATION: &str = "mutation($id: ID!) { \
markPullRequestReadyForReview(input: { pullRequestId: $id }) { pullRequest { isDraft } } }";

/// GraphQL endpoint for a REST API base: `https://api.github.com` → `/graphql`;
/// GitHub Enterprise `https://host/api/v3` → `https://host/api/graphql`.
fn graphql_url(api: &str) -> String {
    match api.strip_suffix("/api/v3") {
        Some(host) => format!("{host}/api/graphql"),
        None => format!("{api}/graphql"),
    }
}

#[derive(Deserialize)]
struct ApiDeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct ApiToken {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
    interval: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    token: String,
    user: GithubUser,
}

struct Pending {
    device_code: String,
    expires_at: Instant,
    /// Seconds between polls, as GitHub asked (raised on `slow_down`).
    interval: u64,
}

#[derive(Default)]
struct Inner {
    session: Option<Session>,
    pending: Option<Pending>,
}

/// What `gh` told us, cached for [`CLI_TTL`].
#[derive(Clone)]
struct CliState {
    /// When this result must be refreshed.
    expires: Instant,
    user: Option<GithubUser>,
    /// The machine's global git identity, if both name and email are set.
    git_identity: Option<Identity>,
}

enum CliUser {
    LoggedIn(GithubUser),
    /// `gh` is missing or says it is not logged in.
    LoggedOut,
    /// Timeout, network error, 5xx, unparsable output: says nothing about the login.
    Failed,
}

struct GhOutput {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

impl GhOutput {
    /// gh's own "not logged in": exit code 4, or its `gh auth login` hint.
    fn logged_out(&self) -> bool {
        self.code == Some(4)
            || self.stderr.contains("auth login")
            || self.stderr.contains("not logged in")
    }
}

struct GhFail {
    /// The binary does not exist.
    missing: bool,
    err: ApiError,
}

pub struct GithubAuth {
    cfg: GithubConfig,
    path: PathBuf,
    http: reqwest::Client,
    inner: Mutex<Inner>,
    cli: Mutex<Option<CliState>>,
    /// The identity last written to the projects, to notice changes (see `identity_change`).
    applied: Mutex<Option<Identity>>,
    /// Token-made REST GETs, reused briefly and revalidated with ETags (see `github_cache`).
    get_cache: crate::github_cache::GetCache,
    /// Review images by `repo@sha:path`: a commit's file never changes, so they're kept as is.
    raw_files: std::sync::Mutex<std::collections::VecDeque<(String, bytes::Bytes)>>,
}

impl GithubAuth {
    /// Load a saved session from `state_dir` if there is one.
    pub fn open(cfg: GithubConfig, state_dir: &Path) -> Self {
        let path = state_dir.join(SESSION_FILE);
        let session = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Session>(&b).ok());
        let http = reqwest::Client::builder()
            .user_agent("orso")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("HTTP client");
        let cfg_ttl = cfg.get_cache_ttl;
        Self {
            cfg,
            path,
            http,
            inner: Mutex::new(Inner {
                session,
                pending: None,
            }),
            cli: Mutex::new(None),
            applied: Mutex::new(None),
            get_cache: crate::github_cache::GetCache::new(cfg_ttl),
            raw_files: std::sync::Mutex::default(),
        }
    }

    pub async fn status(&self) -> Status {
        let device_flow = self.cfg.client_id.is_some();
        if let Some(s) = self.inner.lock().await.session.as_ref() {
            return Status {
                configured: true,
                device_flow,
                source: Some("oauth"),
                user: Some(s.user.clone()),
            };
        }
        let user = self.cli_state().await.user;
        Status {
            configured: device_flow || user.is_some(),
            device_flow,
            source: user.as_ref().map(|_| "cli"),
            user,
        }
    }

    /// The `gh` login (cached for a minute). Runs `gh api user` when the cache is stale.
    async fn cli_state(&self) -> CliState {
        let mut cache = self.cli.lock().await;
        if let Some(c) = cache.as_ref().filter(|c| Instant::now() < c.expires) {
            return c.clone();
        }
        let state = match self.fetch_cli_user().await {
            CliUser::LoggedIn(user) => CliState {
                expires: Instant::now() + CLI_TTL,
                git_identity: global_git_identity(self.cfg.git_global_config.as_deref()).await,
                user: Some(user),
            },
            CliUser::LoggedOut => CliState {
                expires: Instant::now() + CLI_RETRY,
                user: None,
                git_identity: None,
            },
            // A transient failure is not a logout: keep serving the last good login.
            CliUser::Failed => match cache.as_ref().filter(|c| c.user.is_some()) {
                Some(prev) => CliState {
                    expires: Instant::now() + CLI_STALE_RETRY,
                    ..prev.clone()
                },
                None => CliState {
                    expires: Instant::now() + CLI_RETRY,
                    user: None,
                    git_identity: None,
                },
            },
        };
        *cache = Some(state.clone());
        state
    }

    async fn fetch_cli_user(&self) -> CliUser {
        let out = match self.run_gh(&["api", "user"], 10).await {
            Ok(o) => o,
            Err(f) if f.missing => return CliUser::LoggedOut,
            Err(_) => return CliUser::Failed,
        };
        if out.code == Some(0) {
            return match serde_json::from_slice::<ApiUser>(&out.stdout) {
                Ok(u) => CliUser::LoggedIn(GithubUser {
                    login: u.login,
                    id: u.id,
                    name: u.name,
                    avatar_url: u.avatar_url,
                }),
                Err(_) => CliUser::Failed,
            };
        }
        if out.logged_out() {
            CliUser::LoggedOut
        } else {
            CliUser::Failed
        }
    }

    /// Run `gh` without a terminal, with a deadline. Only exit status, stdout and stderr
    /// come back; callers must not log stdout.
    #[allow(clippy::result_large_err)] // GhFail carries ApiError; boxing every call site is noisier.
    async fn run_gh(&self, args: &[&str], secs: u64) -> Result<GhOutput, GhFail> {
        self.run_gh_input(args, secs, None).await
    }

    /// [`Self::run_gh`] with `input` written to gh's stdin.
    #[allow(clippy::result_large_err)]
    async fn run_gh_input(
        &self,
        args: &[&str],
        secs: u64,
        input: Option<&[u8]>,
    ) -> Result<GhOutput, GhFail> {
        let fail = |missing: bool, msg: String| GhFail {
            missing,
            err: ApiError::github(msg),
        };
        if self.cfg.gh_path.as_os_str().is_empty() {
            return Err(fail(true, "GitHub CLI is not available".into()));
        }
        let mut cmd = tokio::process::Command::new(&self.cfg.gh_path);
        cmd.args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let run = async {
            let mut child = cmd.spawn()?;
            if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(bytes).await?;
            }
            child.wait_with_output().await
        };
        let out = match tokio::time::timeout(Duration::from_secs(secs), run).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => {
                let missing = e.kind() == std::io::ErrorKind::NotFound;
                return Err(fail(missing, format!("GitHub CLI: cannot run gh: {e}")));
            }
            Err(_) => return Err(fail(false, "GitHub CLI: gh timed out".into())),
        };
        Ok(GhOutput {
            code: out.status.code(),
            stdout: out.stdout,
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        })
    }

    pub async fn token(&self) -> Option<String> {
        self.inner
            .lock()
            .await
            .session
            .as_ref()
            .map(|s| s.token.clone())
    }

    /// The signed-in person's commit identity, or [`Identity::orso`]. With the GitHub
    /// CLI it is the machine's global git identity when set, else the GitHub profile.
    pub async fn identity(&self) -> Identity {
        if let Some(s) = self.inner.lock().await.session.as_ref() {
            return s.user.identity();
        }
        let cli = self.cli_state().await;
        match (cli.git_identity, cli.user) {
            (Some(id), _) => id,
            (None, Some(u)) => u.identity(),
            (None, None) => Identity::orso(),
        }
    }

    /// Record that the projects now carry `identity` (daemon start).
    pub async fn note_applied(&self, identity: &Identity) {
        *self.applied.lock().await = Some(identity.clone());
    }

    /// The identity last written to the projects. Hold the guard while re-reading
    /// [`GithubAuth::identity`] and writing the projects so concurrent updates serialize.
    pub async fn applied_guard(&self) -> tokio::sync::MutexGuard<'_, Option<Identity>> {
        self.applied.lock().await
    }

    /// `{git_url}/{repo}.git` for a validated `owner/name`.
    pub fn clone_url(&self, repo: &str) -> String {
        format!("{}/{repo}.git", self.cfg.git_url.trim_end_matches('/'))
    }

    /// Environment that authenticates git to `git_url` with the session token.
    pub async fn git_env(&self) -> Result<Vec<(String, String)>, ApiError> {
        if let Some(token) = self.token().await {
            return Ok(git_auth_env(&self.cfg.git_url, &token));
        }
        if self.cli_state().await.user.is_some() {
            return Ok(git_cli_env(&self.cfg.git_url, &self.cfg.gh_path));
        }
        Err(not_signed_in())
    }

    pub async fn start_device(&self) -> Result<DeviceStart, ApiError> {
        let client_id = self.client_id()?;
        let res = self
            .http
            .post(format!("{}/login/device/code", self.web()))
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, FORM)
            .body(form(&[("client_id", client_id), ("scope", SCOPES)]))
            .send()
            .await
            .map_err(unreachable)?;
        let code: ApiDeviceCode = parse(res).await?;
        self.inner.lock().await.pending = Some(Pending {
            device_code: code.device_code,
            expires_at: Instant::now() + Duration::from_secs(code.expires_in),
            interval: code.interval,
        });
        Ok(DeviceStart {
            user_code: code.user_code,
            verification_uri: code.verification_uri,
            expires_in: code.expires_in,
            interval: code.interval,
        })
    }

    /// Ask GitHub once whether the user approved the code. The client calls this every
    /// `interval` seconds (longer after `slowDown`).
    pub async fn poll_device(&self) -> Result<PollOutcome, ApiError> {
        let client_id = self.client_id()?;
        let device_code = {
            let mut inner = self.inner.lock().await;
            match &inner.pending {
                None => return Err(ApiError::badreq("no GitHub sign-in in progress")),
                Some(p) if Instant::now() >= p.expires_at => {
                    inner.pending = None;
                    return Ok(PollOutcome::Expired);
                }
                Some(p) => p.device_code.clone(),
            }
        };
        let res = self
            .http
            .post(format!("{}/login/oauth/access_token", self.web()))
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, FORM)
            .body(form(&[
                ("client_id", client_id),
                ("device_code", &device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ]))
            .send()
            .await
            .map_err(unreachable)?;
        let tok: ApiToken = parse(res).await?;
        match (tok.access_token, tok.error.as_deref()) {
            (Some(token), _) => {
                let user = match self.fetch_user(&token).await {
                    Ok(u) => u,
                    Err(e) => {
                        self.clear_pending(&device_code).await;
                        return Err(e);
                    }
                };
                let session = Session {
                    token,
                    user: user.clone(),
                };
                match self.commit_session(&device_code, session).await {
                    Ok(true) => Ok(PollOutcome::SignedIn { user }),
                    Ok(false) => Err(ApiError::github("sign-in was cancelled").with_status(409)),
                    Err(e) => {
                        self.clear_pending(&device_code).await;
                        Err(e)
                    }
                }
            }
            (None, Some("authorization_pending")) => Ok(PollOutcome::Pending),
            (None, Some("slow_down")) => {
                let mut inner = self.inner.lock().await;
                let current = match &inner.pending {
                    Some(p) if p.device_code == device_code => p.interval,
                    _ => 5,
                };
                let interval = tok.interval.unwrap_or(current + 5);
                if let Some(p) = inner
                    .pending
                    .as_mut()
                    .filter(|p| p.device_code == device_code)
                {
                    p.interval = interval;
                }
                Ok(PollOutcome::SlowDown { interval })
            }
            (None, Some("expired_token")) => {
                self.clear_pending(&device_code).await;
                Ok(PollOutcome::Expired)
            }
            (None, Some("access_denied")) => {
                self.clear_pending(&device_code).await;
                Ok(PollOutcome::Denied)
            }
            (None, other) => {
                // Terminal: incorrect_device_code, device_flow_disabled, bad credentials, ...
                self.clear_pending(&device_code).await;
                let msg = if other == Some("device_flow_disabled") {
                    "Device Flow is not enabled for this GitHub OAuth App: tick \"Enable Device Flow\" in its settings".to_string()
                } else {
                    format!(
                        "GitHub sign-in failed: {}",
                        tok.error_description
                            .or(other.map(str::to_string))
                            .unwrap_or_else(|| "no token in the response".into())
                    )
                };
                Err(ApiError::github(msg))
            }
        }
    }

    /// Drop the pending device flow if it is still the one for `device_code`.
    async fn clear_pending(&self, device_code: &str) {
        let mut inner = self.inner.lock().await;
        if inner
            .pending
            .as_ref()
            .is_some_and(|p| p.device_code == device_code)
        {
            inner.pending = None;
        }
    }

    pub async fn sign_out(&self) {
        let mut inner = self.inner.lock().await;
        inner.session = None;
        inner.pending = None;
        let _ = std::fs::remove_file(&self.path);
    }

    /// Clear the session (and its file) only if it holds `token`. Returns whether it did.
    pub async fn sign_out_if_token(&self, token: &str) -> bool {
        let mut inner = self.inner.lock().await;
        if inner.session.as_ref().is_none_or(|s| s.token != token) {
            return false;
        }
        inner.session = None;
        inner.pending = None;
        let _ = std::fs::remove_file(&self.path);
        true
    }

    /// Repositories the user can access, most recently pushed first (first 100).
    pub async fn repos(&self) -> Result<Vec<RepoInfo>, ApiError> {
        self.repos_checked().await.0
    }

    /// Like [`GithubAuth::repos`], also reporting whether this call cleared the session
    /// because GitHub rejected its token.
    pub async fn repos_checked(&self) -> (Result<Vec<RepoInfo>, ApiError>, bool) {
        let Some(token) = self.token().await else {
            return self.cli_repos().await;
        };
        let mut cleared = false;
        let r = self.list_repos(&token, &mut cleared).await;
        (r, cleared)
    }

    /// Repositories through `gh api`. The flag says the CLI turned out to be logged out.
    async fn cli_repos(&self) -> (Result<Vec<RepoInfo>, ApiError>, bool) {
        if self.cli_state().await.user.is_none() {
            return (Err(not_signed_in()), false);
        }
        let out = match self.run_gh(&["api", REPOS_PATH], 30).await {
            Ok(o) => o,
            Err(f) => return (Err(f.err.with_status(502)), false),
        };
        if out.code != Some(0) {
            if out.logged_out() {
                *self.cli.lock().await = None;
                return (
                    Err(
                        ApiError::github("GitHub CLI is not logged in — run `gh auth login`")
                            .with_status(400),
                    ),
                    true,
                );
            }
            let tail: String = out
                .stderr
                .chars()
                .rev()
                .take(300)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            return (
                Err(ApiError::github(format!("GitHub CLI: {tail}")).with_status(502)),
                false,
            );
        }
        let repos: Vec<ApiRepo> = match serde_json::from_slice(&out.stdout) {
            Ok(r) => r,
            Err(_) => {
                return (
                    Err(ApiError::github("unexpected response from the GitHub CLI")
                        .with_status(502)),
                    false,
                )
            }
        };
        (Ok(repos.into_iter().map(RepoInfo::from).collect()), false)
    }

    async fn list_repos(&self, token: &str, cleared: &mut bool) -> Result<Vec<RepoInfo>, ApiError> {
        let res = self
            .http
            .get(format!(
                "{}/user/repos?per_page=100&sort=pushed",
                self.api()
            ))
            .bearer_auth(token)
            .header(ACCEPT, GITHUB_JSON)
            .send()
            .await
            .map_err(unreachable)?;
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            // Revoked or expired token: forget the session so the UI offers sign-in.
            // Only if it is still the rejected token: a slow 401 must not wipe a newer session.
            *cleared = self.sign_out_if_token(token).await;
            return Err(
                ApiError::github("your GitHub session expired — sign in again").with_status(400),
            );
        }
        let repos: Vec<ApiRepo> = parse(res).await?;
        Ok(repos.into_iter().map(RepoInfo::from).collect())
    }

    async fn fetch_user(&self, token: &str) -> Result<GithubUser, ApiError> {
        let res = self
            .http
            .get(format!("{}/user", self.api()))
            .bearer_auth(token)
            .header(ACCEPT, GITHUB_JSON)
            .send()
            .await
            .map_err(unreachable)?;
        let u: ApiUser = parse(res).await?;
        Ok(GithubUser {
            login: u.login,
            id: u.id,
            name: u.name,
            avatar_url: u.avatar_url,
        })
    }

    /// Persist and activate `session`, but only if the flow for `device_code` is still the
    /// pending one (not signed out or restarted meanwhile). Returns whether it was committed.
    /// The check and the write happen under one lock so a concurrent sign-out cannot be undone.
    async fn commit_session(&self, device_code: &str, session: Session) -> Result<bool, ApiError> {
        let bytes = serde_json::to_vec_pretty(&session)
            .map_err(|e| ApiError::io(format!("serialize GitHub session: {e}")))?;
        let mut inner = self.inner.lock().await;
        if inner
            .pending
            .as_ref()
            .is_none_or(|p| p.device_code != device_code)
        {
            return Ok(false);
        }
        write_private(&self.path, &bytes)?;
        inner.session = Some(session);
        inner.pending = None;
        Ok(true)
    }

    /// Open draft PR for `head_branch` on `owner/name`, or the existing open one for that head.
    pub async fn find_pr(
        &self,
        repo: &str,
        head_branch: &str,
    ) -> Result<Option<PullRequest>, ApiError> {
        let owner = repo
            .split_once('/')
            .map(|(o, _)| o)
            .ok_or_else(|| ApiError::badreq(format!("not a GitHub repository: {repo:?}")))?;
        let pulls: Vec<ApiPull> = self
            .get_json(&format!(
                "/repos/{repo}/pulls?head={}:{}&state=open",
                urlencode(owner),
                urlencode(head_branch)
            ))
            .await?;
        Ok(pulls.into_iter().next().map(PullRequest::from))
    }

    /// Create a draft pull request from `head_branch` into `main`, tagged with `labels`.
    pub async fn create_draft_pr(
        &self,
        repo: &str,
        head_branch: &str,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<PullRequest, ApiError> {
        let pull: ApiPull = self
            .api_json(
                reqwest::Method::POST,
                &format!("/repos/{repo}/pulls"),
                Some(&serde_json::json!({
                    "title": title,
                    "head": head_branch,
                    "base": "main",
                    "draft": true,
                    "body": body,
                })),
            )
            .await?;
        // Tag it so reviewers can filter orso's PRs (e.g. `label:orso`). GitHub creates a label
        // on first use. Best effort: a refused or failed label must not fail sharing the change.
        if !labels.is_empty() {
            let _ = self
                .api_raw(
                    reqwest::Method::POST,
                    &format!("/repos/{repo}/issues/{}/labels", pull.number),
                    Some(&serde_json::json!({ "labels": labels })),
                )
                .await;
        }
        Ok(PullRequest::from(pull))
    }

    /// Mark `head_branch`'s open draft pull request ready for review (GraphQL
    /// `markPullRequestReadyForReview` by node id; a PR that is already ready is left as is),
    /// then its status. The mutation is authoritative: a status re-read that fails, or that
    /// still says draft (REST can lag a GraphQL write), still yields an open PR.
    pub async fn mark_ready_for_review(
        &self,
        repo: &str,
        head_branch: &str,
    ) -> Result<PrStatus, ApiError> {
        let pull = self.find_pr(repo, head_branch).await?.ok_or_else(|| {
            ApiError::enoent("This change hasn't been shared yet — share it first")
        })?;
        if pull.draft {
            self.mark_ready_mutation(&pull.node_id).await?;
        }
        let mut status = match self.pr_status(repo, head_branch).await {
            Ok(Some(s)) if s.number == pull.number => s,
            _ => PrStatus {
                number: pull.number,
                url: pull.html_url,
                state: PrState::Open,
                checks: ChecksState::None,
                preview_url: None,
                mergeable: None,
                mergeable_state: None,
                up_to_date: false,
                sent_back: None,
                // Unknown without the re-read; only used server-side (cleanup), never sent.
                head_sha: String::new(),
            },
        };
        if status.state == PrState::Draft {
            status.state = PrState::Open;
        }
        Ok(status)
    }

    async fn mark_ready_mutation(&self, node_id: &str) -> Result<(), ApiError> {
        const WHAT: &str = "GitHub couldn't send this change for a greenlight";
        if node_id.is_empty() {
            return Err(ApiError::github(format!("{WHAT}: no GitHub id for it")));
        }
        let payload = serde_json::json!({
            "query": MARK_READY_MUTATION,
            "variables": { "id": node_id },
        });
        let (status, body) = match self.token().await {
            Some(token) => {
                let res = self
                    .http
                    .post(graphql_url(self.api()))
                    .bearer_auth(&token)
                    .header(CONTENT_TYPE, "application/json")
                    .json(&payload)
                    .send()
                    .await
                    .map_err(unreachable)?;
                let status = res.status();
                let body = res.bytes().await.map_err(unreachable)?.to_vec();
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    let _ = self.sign_out_if_token(&token).await;
                    return Err(ApiError::unauth(
                        "your GitHub session expired — sign in again",
                    ));
                }
                (status, body)
            }
            None => {
                let out = self.gh_api("POST", "graphql", Some(&payload)).await?;
                let graphql_errors = || {
                    serde_json::from_slice::<ApiGraphql>(&out.stdout)
                        .is_ok_and(|b| !b.errors.is_empty())
                };
                match (out.code, gh_http_status(&out.stderr)) {
                    (Some(0), _) => (reqwest::StatusCode::OK, out.stdout),
                    _ if out.logged_out() => return Err(self.gh_api_error(out).await),
                    (_, Some(status)) => (status, out.stdout),
                    // GraphQL errors: gh exits 1 with the response on stdout.
                    (_, None) if graphql_errors() => (reqwest::StatusCode::OK, out.stdout),
                    _ => return Err(self.gh_api_error(out).await),
                }
            }
        };
        // GraphQL names no repository path: drop every cached GET.
        self.get_cache.clear();
        let excerpt =
            |n: usize| -> String { String::from_utf8_lossy(&body).chars().take(n).collect() };
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(ApiError::sandbox(
                "You don't have permission to send this change for a greenlight",
            ));
        }
        if !status.is_success() {
            return Err(ApiError::github(format!(
                "{WHAT}: GitHub returned {status}: {}",
                excerpt(300)
            )));
        }
        let body: ApiGraphql = serde_json::from_slice(&body)
            .map_err(|_| ApiError::github(format!("{WHAT}: unexpected response from GitHub")))?;
        if let Some(e) = body.errors.into_iter().next() {
            let msg: String = e.message.chars().take(300).collect();
            return Err(
                if e.kind
                    .as_deref()
                    .is_some_and(|k| PERMISSION_ERRORS.contains(&k))
                {
                    ApiError::sandbox(format!(
                        "You don't have permission to send this change for a greenlight: {msg}"
                    ))
                } else {
                    ApiError::github(format!("{WHAT}: {msg}"))
                },
            );
        }
        let out_of_draft = body
            .data
            .and_then(|d| d.mark_pull_request_ready_for_review)
            .and_then(|p| p.pull_request)
            .is_some_and(|p| !p.is_draft);
        if !out_of_draft {
            return Err(ApiError::github(format!("{WHAT}: it is still a draft")));
        }
        Ok(())
    }

    /// Merge `head_branch`'s open pull request into `main` (the Publish action), or say why
    /// it could not be published in words a writer can act on:
    /// - no permission on the repo → ask an owner to add them;
    /// - not mergeable (including protection blocking the merge) → get latest first.
    ///
    /// The merge method is `merge`: this codebase has no squash convention.
    pub async fn merge_pr(&self, repo: &str, head_branch: &str) -> Result<PublishResult, ApiError> {
        let pull = self.find_pr(repo, head_branch).await?.ok_or_else(|| {
            ApiError::enoent("Couldn't publish. Share the draft first, then try again.")
        })?;
        // A reviewer's send-back holds the change even when GitHub would merge it (no branch
        // protection). Drafts have no reviews that count; publish readies them first.
        if !pull.draft {
            let state = self.pull_and_state(repo, head_branch).await?;
            if matches!(state, Some((_, PrState::ChangesRequested, _))) {
                return Err(ApiError::conflict("A reviewer sent this change back."));
            }
        }
        let (status, body) = self
            .api_raw(
                reqwest::Method::PUT,
                &format!("/repos/{repo}/pulls/{}/merge", pull.number),
                Some(&serde_json::json!({ "merge_method": "merge" })),
            )
            .await?;
        if !status.is_success() {
            return Err(publish_refused(status));
        }
        let merged = serde_json::from_slice::<ApiMerge>(&body).unwrap_or(ApiMerge {
            merged: false,
            sha: None,
        });
        if !merged.merged {
            return Err(publish_failed());
        }
        Ok(PublishResult {
            number: pull.number,
            url: pull.html_url,
            state: PrState::Merged,
            merge_sha: merged.sha,
        })
    }

    /// PR state, checks and preview URL for `head_branch`; `None` when it has no pull request.
    pub async fn pr_status(
        &self,
        repo: &str,
        head_branch: &str,
    ) -> Result<Option<PrStatus>, ApiError> {
        let Some((pull, state, sent_back)) = self.pull_and_state(repo, head_branch).await? else {
            return Ok(None);
        };
        let sha = pull.head.map(|h| h.sha).unwrap_or_default();
        // Best effort past the PR itself: a failing checks or deployments call only blanks
        // that part of the status.
        let (checks, preview_url) = if sha.is_empty() {
            (ChecksState::None, None)
        } else {
            let runs_path = format!("/repos/{repo}/commits/{sha}/check-runs");
            let (runs, preview) = tokio::join!(
                self.get_all::<ApiCheckRuns, ApiCheckRun>(&runs_path, |p| p.check_runs),
                self.preview_url(repo, &sha),
            );
            (
                runs.map_or(ChecksState::None, |r| aggregate_checks(&r)),
                preview.unwrap_or(None),
            )
        };
        Ok(Some(PrStatus {
            number: pull.number,
            url: pull.html_url,
            state,
            checks,
            preview_url,
            mergeable: pull.mergeable,
            mergeable_state: pull.mergeable_state,
            up_to_date: false,
            sent_back,
            head_sha: sha,
        }))
    }

    /// Login of the signed-in user.
    pub async fn signed_in_login(&self) -> Result<String, ApiError> {
        self.status()
            .await
            .user
            .map(|u| u.login)
            .ok_or_else(not_signed_in)
    }

    /// Whether the signed-in user can push to `repo`.
    pub async fn can_push(&self, repo: &str) -> Result<bool, ApiError> {
        let r: ApiRepo = self.get_json(&format!("/repos/{repo}")).await?;
        Ok(r.permissions.is_some_and(|p| p.push))
    }

    /// Open, non-draft pull requests carrying any of `labels` (case-insensitive), `me`'s own
    /// included (marked `mine`), each with `me`'s review standing and the head's checks.
    pub async fn review_inbox(
        &self,
        repo: &str,
        labels: &[String],
        me: &str,
    ) -> Result<ReviewInbox, ApiError> {
        use futures_util::{StreamExt, TryStreamExt};
        let pulls = self
            .get_all_query::<Vec<ApiPull>, ApiPull>(
                &format!("/repos/{repo}/pulls"),
                "state=open&",
                |p| p,
            )
            .await?;
        let open: Vec<u64> = pulls.iter().map(|p| p.number).collect();
        let kept: Vec<ApiPull> = pulls
            .into_iter()
            .filter(|p| {
                !p.draft
                    && p.labels
                        .iter()
                        .any(|l| labels.iter().any(|w| w.eq_ignore_ascii_case(&l.name)))
            })
            .collect();
        // Bounded, so a busy repository does not fire every PR's requests at once; `buffered`
        // keeps the results in the listing's order.
        let reviews = futures_util::stream::iter(kept)
            .map(|p| self.review_item(repo, p, me))
            .buffered(INBOX_CONCURRENCY)
            .try_collect()
            .await?;
        let counts = self.comment_counts(repo, false).await;
        Ok(ReviewInbox {
            reviews: with_comments(reviews, &counts),
            open,
        })
    }

    /// The most recently updated closed or merged pull requests carrying any of `labels`
    /// (one page of [`CLOSED_PAGE`]), each as an inbox row.
    pub async fn closed_reviews(
        &self,
        repo: &str,
        labels: &[String],
        me: &str,
    ) -> Result<ClosedReviews, ApiError> {
        use futures_util::{StreamExt, TryStreamExt};
        let pulls: Vec<ApiPull> = self
            .get_json(&format!(
                "/repos/{repo}/pulls?state=closed&sort=updated&direction=desc&per_page={CLOSED_PAGE}"
            ))
            .await?;
        let more = pulls.len() >= CLOSED_PAGE;
        let kept: Vec<ApiPull> = pulls
            .into_iter()
            .filter(|p| {
                p.labels
                    .iter()
                    .any(|l| labels.iter().any(|w| w.eq_ignore_ascii_case(&l.name)))
            })
            .collect();
        let reviews = futures_util::stream::iter(kept)
            .map(|p| self.review_item(repo, p, me))
            .buffered(INBOX_CONCURRENCY)
            .try_collect()
            .await?;
        let counts = self.comment_counts(repo, true).await;
        Ok(ClosedReviews {
            reviews: with_comments(reviews, &counts),
            more,
        })
    }

    /// Conversation comment counts by number, from the issues listing (which includes pull
    /// requests). Best effort: an unreadable listing counts nothing. Open ones are every page;
    /// closed ones the most recently updated page, matching [`Self::closed_reviews`].
    async fn comment_counts(&self, repo: &str, closed: bool) -> HashMap<u64, u64> {
        let path = format!("/repos/{repo}/issues");
        let issues = if closed {
            self.get_json::<Vec<ApiIssueCount>>(&format!(
                "{path}?state=closed&sort=updated&direction=desc&per_page=100"
            ))
            .await
        } else {
            self.get_all_query::<Vec<ApiIssueCount>, ApiIssueCount>(&path, "state=open&", |p| p)
                .await
        };
        issues
            .unwrap_or_default()
            .into_iter()
            .map(|i| (i.number, i.comments))
            .collect()
    }

    /// One pull request opened for review.
    pub async fn review_detail(
        &self,
        repo: &str,
        number: u64,
        me: &str,
    ) -> Result<ReviewDetail, ApiError> {
        let pull: ApiPull = self
            .get_json(&format!("/repos/{repo}/pulls/{number}"))
            .await?;
        let open = pull.state.as_deref() == Some("open") && pull.merged_at.is_none();
        let (draft, description, merged) =
            (pull.draft, pull.body.clone(), pull.merged_at.is_some());
        let labels = pull.labels.iter().map(|l| l.name.clone()).collect();
        let item = self.review_item(repo, pull, me).await?;
        Ok(ReviewDetail {
            item,
            description,
            open,
            draft,
            merged,
            labels,
        })
    }

    /// Change `number`'s conversation: comments, reviews and pushes oldest first, its head's
    /// checks, and where each reviewer stands.
    pub async fn conversation(&self, repo: &str, number: u64) -> Result<Conversation, ApiError> {
        let pull: ApiPull = self
            .get_json(&format!("/repos/{repo}/pulls/{number}"))
            .await?;
        let sha = pull
            .head
            .as_ref()
            .map(|h| h.sha.clone())
            .unwrap_or_default();
        let comments_path = format!("/repos/{repo}/issues/{number}/comments");
        let reviews_path = format!("/repos/{repo}/pulls/{number}/reviews");
        let commits_path = format!("/repos/{repo}/pulls/{number}/commits");
        let runs_path = format!("/repos/{repo}/commits/{sha}/check-runs");
        let comments = self.get_all::<Vec<ApiIssueComment>, ApiIssueComment>(&comments_path, |p| p);
        let reviews = self.get_all::<Vec<ApiReview>, ApiReview>(&reviews_path, |p| p);
        let commits = self.get_all::<Vec<ApiPullCommit>, ApiPullCommit>(&commits_path, |p| p);
        // Checks are a sidebar extra: unreadable checks show as none.
        let runs = async {
            if sha.is_empty() {
                return Vec::new();
            }
            self.get_all::<ApiCheckRuns, ApiCheckRun>(&runs_path, |p| p.check_runs)
                .await
                .unwrap_or_default()
        };
        let (comments, reviews, commits, runs) = tokio::join!(comments, reviews, commits, runs);
        let reviews = reviews?;
        let closed = match (&pull.merged_at, &pull.closed_at) {
            (Some(at), _) => Some((at.clone(), true)),
            (None, Some(at)) if pull.state.as_deref() == Some("closed") => {
                Some((at.clone(), false))
            }
            _ => None,
        };
        Ok(Conversation {
            items: timeline(comments?, &reviews, commits?, closed),
            checks: runs
                .into_iter()
                .map(|r| CheckItem {
                    name: r.name,
                    status: r.status,
                    conclusion: r.conclusion,
                    url: r.html_url.filter(|u| is_http_url(u)),
                })
                .collect(),
            reviewers: reviewers(&reviews, &pull.requested_reviewers),
        })
    }

    /// Comment on change `number` as the signed-in user; the new timeline entry.
    pub async fn add_comment(
        &self,
        repo: &str,
        number: u64,
        body: &str,
    ) -> Result<TimelineItem, ApiError> {
        let (status, bytes) = self
            .api_raw(
                reqwest::Method::POST,
                &format!("/repos/{repo}/issues/{number}/comments"),
                Some(&serde_json::json!({ "body": body })),
            )
            .await?;
        if !status.is_success() {
            return Err(match status {
                reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND => {
                    ApiError::sandbox("GitHub didn't let you comment on this change.")
                }
                _ => ApiError::github("Couldn't post your comment. Try again in a moment."),
            });
        }
        let c: ApiIssueComment = serde_json::from_slice(&bytes)
            .map_err(|e| ApiError::github(format!("unexpected answer from GitHub: {e}")))?;
        let avatar_url = c
            .user
            .as_ref()
            .and_then(|u| u.avatar_url.clone())
            .filter(|a| is_http_url(a));
        Ok(TimelineItem::Comment {
            id: c.id,
            author: c.user.map_or_else(|| "ghost".to_string(), |u| u.login),
            avatar_url,
            at: c.created_at,
            body: c.body.unwrap_or_default(),
            url: c.html_url,
        })
    }

    /// Greenlight (approve) or send back (request changes) `number` as the signed-in user.
    /// The review is pinned to `head_sha`, the commit the reviewer looked at: when the head
    /// has moved since, nothing is sent and the caller is told to reopen the change.
    pub async fn submit_review(
        &self,
        repo: &str,
        number: u64,
        head_sha: &str,
        decision: Decision,
        comment: Option<&str>,
    ) -> Result<(), ApiError> {
        let event = match decision {
            Decision::Greenlight => "APPROVE",
            Decision::SendBack => "REQUEST_CHANGES",
        };
        self.submit_review_event(repo, number, head_sha, event, comment.unwrap_or_default())
            .await
    }

    /// 409 `head_moved` unless `number`'s head is still `head_sha` (read fresh, not cached).
    pub async fn ensure_head(
        &self,
        repo: &str,
        number: u64,
        head_sha: &str,
    ) -> Result<(), ApiError> {
        let pull: ApiPull = self
            .api_json(
                reqwest::Method::GET,
                &format!("/repos/{repo}/pulls/{number}"),
                None,
            )
            .await?;
        if pull.head.as_ref().map(|h| h.sha.as_str()) != Some(head_sha) {
            return Err(
                ApiError::conflict("This change was updated since you opened it")
                    .with_reason("head_moved"),
            );
        }
        Ok(())
    }

    /// A review of `number` with GitHub's `event` (`APPROVE`, `REQUEST_CHANGES` or `COMMENT`)
    /// and `body`, pinned to `head_sha` (409 `head_moved` when the head has moved since).
    pub async fn submit_review_event(
        &self,
        repo: &str,
        number: u64,
        head_sha: &str,
        event: &str,
        body: &str,
    ) -> Result<(), ApiError> {
        self.ensure_head(repo, number, head_sha).await?;
        let (status, _) = self
            .api_raw(
                reqwest::Method::POST,
                &format!("/repos/{repo}/pulls/{number}/reviews"),
                Some(&serde_json::json!({
                    "event": event,
                    "body": body,
                    "commit_id": head_sha,
                })),
            )
            .await?;
        if status.is_success() {
            return Ok(());
        }
        Err(match status {
            reqwest::StatusCode::FORBIDDEN => {
                ApiError::sandbox("Ask an owner to add you to this site, then try again.")
            }
            reqwest::StatusCode::UNPROCESSABLE_ENTITY => ApiError::conflict(
                "GitHub didn't accept that review. Reopen the change and try again.",
            ),
            _ => ApiError::github("Couldn't send your review. Try again in a moment."),
        })
    }

    /// Commits `files` (path → bytes) to `branch` in `repo` through the Git Data API, creating
    /// the branch as an orphan on first use, and returns the new commit's sha. Never touches
    /// any other branch. A branch that moved underneath (non-fast-forward, or created in the
    /// meantime) is retried once from its new tip.
    pub async fn commit_files_to_branch(
        &self,
        repo: &str,
        branch: &str,
        message: &str,
        files: &[(String, Vec<u8>)],
    ) -> Result<String, ApiError> {
        use base64::Engine;
        let mut blobs = Vec::with_capacity(files.len());
        for (path, bytes) in files {
            let blob: ApiSha = self
                .git_post(
                    repo,
                    "blobs",
                    serde_json::json!({
                        "content": base64::engine::general_purpose::STANDARD.encode(bytes),
                        "encoding": "base64",
                    }),
                )
                .await?;
            blobs.push((path.clone(), blob.sha));
        }
        let mut last = None;
        for _ in 0..2 {
            match self.commit_blobs(repo, branch, message, &blobs).await? {
                Ok(sha) => return Ok(sha),
                Err(refused) => last = Some(refused),
            }
        }
        Err(last.unwrap_or_else(|| ApiError::github("Couldn't save the screenshots.")))
    }

    /// Removes review images of pull requests that `keep` rejects from `branch` (files under
    /// `shots/{pr}/` and `uploads/{pr}/`) in one commit. When no kept pull request has images
    /// left, the branch itself is deleted, so its history stops riding along with every clone;
    /// the next upload starts it again.
    pub async fn clean_review_assets(
        &self,
        repo: &str,
        branch: &str,
        keep: impl Fn(u64) -> bool,
    ) -> Result<AssetsCleanup, ApiError> {
        let (status, body) = self
            .api_raw(
                reqwest::Method::GET,
                &format!("/repos/{repo}/git/ref/heads/{branch}"),
                None,
            )
            .await?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(AssetsCleanup::Nothing);
        }
        if !status.is_success() {
            return Err(github_error(status, &body));
        }
        let tip = serde_json::from_slice::<ApiRef>(&body)
            .map_err(|_| ApiError::github("unexpected response from GitHub"))?
            .object
            .sha;
        let commit: ApiGitCommit = self
            .api_json(
                reqwest::Method::GET,
                &format!("/repos/{repo}/git/commits/{tip}"),
                None,
            )
            .await?;
        let tree: ApiTree = self
            .api_json(
                reqwest::Method::GET,
                &format!("/repos/{repo}/git/trees/{}?recursive=1", commit.tree.sha),
                None,
            )
            .await?;
        // The pull request each review image belongs to.
        let pr_of = |path: &str| -> Option<u64> {
            let mut parts = path.split('/');
            matches!(parts.next(), Some("shots" | "uploads"))
                .then(|| parts.next()?.parse().ok())
                .flatten()
        };
        let files: Vec<(&str, u64)> = tree
            .tree
            .iter()
            .filter(|e| e.kind == "blob")
            .filter_map(|e| Some((e.path.as_str(), pr_of(&e.path)?)))
            .collect();
        if !files.iter().any(|(_, pr)| keep(*pr)) {
            let (status, body) = self
                .api_raw(
                    reqwest::Method::DELETE,
                    &format!("/repos/{repo}/git/refs/heads/{branch}"),
                    None,
                )
                .await?;
            return if status.is_success() || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
                Ok(AssetsCleanup::DeletedBranch)
            } else {
                Err(github_error(status, &body))
            };
        }
        let mut gone: Vec<u64> = files
            .iter()
            .map(|(_, pr)| *pr)
            .filter(|pr| !keep(*pr))
            .collect();
        gone.sort_unstable();
        gone.dedup();
        if gone.is_empty() {
            return Ok(AssetsCleanup::Nothing);
        }
        // A tree entry with a null sha removes that file.
        let removals: Vec<_> = files
            .iter()
            .filter(|(_, pr)| gone.contains(pr))
            .map(|(path, _)| serde_json::json!({ "path": path, "mode": "100644", "type": "blob", "sha": null }))
            .collect();
        let new_tree: ApiSha = self
            .git_post(
                repo,
                "trees",
                serde_json::json!({ "base_tree": commit.tree.sha, "tree": removals }),
            )
            .await?;
        let list = gone
            .iter()
            .map(|n| format!("#{n}"))
            .collect::<Vec<_>>()
            .join(", ");
        let new_commit: ApiSha = self
            .git_post(
                repo,
                "commits",
                serde_json::json!({
                    "message": format!("Remove review images of closed changes {list}"),
                    "tree": new_tree.sha,
                    "parents": [tip],
                }),
            )
            .await?;
        let (status, body) = self
            .api_raw(
                reqwest::Method::PATCH,
                &format!("/repos/{repo}/git/refs/heads/{branch}"),
                Some(&serde_json::json!({ "sha": new_commit.sha, "force": false })),
            )
            .await?;
        if !status.is_success() {
            // Someone uploaded meanwhile; the next inbox load tries again.
            return Err(github_error(status, &body));
        }
        Ok(AssetsCleanup::Removed(gone))
    }

    /// One attempt: tree and commit on the branch's current tip, then move (or create) the ref.
    /// The outer `Err` is a failure; the inner one a ref update GitHub refused, worth a retry.
    async fn commit_blobs(
        &self,
        repo: &str,
        branch: &str,
        message: &str,
        blobs: &[(String, String)],
    ) -> Result<Result<String, ApiError>, ApiError> {
        let (status, body) = self
            .api_raw(
                reqwest::Method::GET,
                &format!("/repos/{repo}/git/ref/heads/{branch}"),
                None,
            )
            .await?;
        let parent = match status {
            s if s.is_success() => Some(
                serde_json::from_slice::<ApiRef>(&body)
                    .map_err(|_| ApiError::github("unexpected response from GitHub"))?
                    .object
                    .sha,
            ),
            reqwest::StatusCode::NOT_FOUND => None,
            s => return Err(github_error(s, &body)),
        };
        let base_tree = match &parent {
            Some(sha) => {
                let commit: ApiGitCommit = self
                    .api_json(
                        reqwest::Method::GET,
                        &format!("/repos/{repo}/git/commits/{sha}"),
                        None,
                    )
                    .await?;
                Some(commit.tree.sha)
            }
            None => None,
        };
        let entries: Vec<_> = blobs
            .iter()
            .map(|(path, sha)| serde_json::json!({ "path": path, "mode": "100644", "type": "blob", "sha": sha }))
            .collect();
        let mut tree_body = serde_json::json!({ "tree": entries });
        if let Some(base) = base_tree {
            tree_body["base_tree"] = serde_json::json!(base);
        }
        let tree: ApiSha = self.git_post(repo, "trees", tree_body).await?;
        let parents: Vec<&String> = parent.iter().collect();
        let commit: ApiSha = self
            .git_post(
                repo,
                "commits",
                serde_json::json!({ "message": message, "tree": tree.sha, "parents": parents }),
            )
            .await?;
        let (status, body) = match &parent {
            Some(_) => {
                self.api_raw(
                    reqwest::Method::PATCH,
                    &format!("/repos/{repo}/git/refs/heads/{branch}"),
                    Some(&serde_json::json!({ "sha": commit.sha, "force": false })),
                )
                .await?
            }
            None => {
                self.api_raw(
                    reqwest::Method::POST,
                    &format!("/repos/{repo}/git/refs"),
                    Some(&serde_json::json!({ "ref": format!("refs/heads/{branch}"), "sha": commit.sha })),
                )
                .await?
            }
        };
        if status.is_success() {
            return Ok(Ok(commit.sha));
        }
        // Behind (someone else's upload landed first) is worth one more try; a refusal isn't.
        let text = String::from_utf8_lossy(&body).to_lowercase();
        if status == reqwest::StatusCode::CONFLICT
            || (status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
                && (text.contains("fast forward") || text.contains("already exists")))
        {
            return Ok(Err(ApiError::conflict(
                "Another image upload landed at the same time. Try again.",
            )));
        }
        Err(assets_write_error(status, &body))
    }

    /// `POST /repos/{repo}/git/{what}` → its `{ sha }`; a refusal is `assets_refused`.
    async fn git_post<T: DeserializeOwned>(
        &self,
        repo: &str,
        what: &str,
        body: serde_json::Value,
    ) -> Result<T, ApiError> {
        let (status, bytes) = self
            .api_raw(
                reqwest::Method::POST,
                &format!("/repos/{repo}/git/{what}"),
                Some(&body),
            )
            .await?;
        if !status.is_success() {
            return Err(assets_write_error(status, &bytes));
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| ApiError::github("unexpected response from GitHub"))
    }

    /// The bytes of `path` at commit `sha` in `repo`, kept in memory once read (a commit's file
    /// never changes; the most recent 64 are kept).
    pub async fn raw_file(
        &self,
        repo: &str,
        sha: &str,
        path: &str,
    ) -> Result<bytes::Bytes, ApiError> {
        const KEEP: usize = 64;
        let key = format!("{repo}@{sha}:{path}");
        if let Some((_, b)) = self
            .raw_files
            .lock()
            .unwrap()
            .iter()
            .find(|(k, _)| *k == key)
        {
            return Ok(b.clone());
        }
        let api_path = format!("/repos/{repo}/contents/{path}?ref={}", urlencode(sha));
        const RAW: &str = "application/vnd.github.raw+json";
        let bytes = if let Some(token) = self.token().await {
            let res = self
                .http
                .get(format!("{}{api_path}", self.api()))
                .bearer_auth(&token)
                .header(ACCEPT, RAW)
                .send()
                .await
                .map_err(unreachable)?;
            let status = res.status();
            let body = res.bytes().await.map_err(unreachable)?;
            if status == reqwest::StatusCode::NOT_FOUND {
                return Err(ApiError::enoent("That image isn't there any more"));
            }
            if !status.is_success() {
                return Err(github_error(status, &body));
            }
            body
        } else {
            let accept = format!("Accept: {RAW}");
            let out = self
                .run_gh_input(&["api", "-H", &accept, &api_path], 30, None)
                .await
                .map_err(|f| f.err.with_status(502))?;
            if out.code != Some(0) {
                return Err(self.gh_api_error(out).await);
            }
            bytes::Bytes::from(out.stdout)
        };
        let mut kept = self.raw_files.lock().unwrap();
        kept.push_back((key, bytes.clone()));
        while kept.len() > KEEP {
            kept.pop_front();
        }
        Ok(bytes)
    }

    /// A link that shows `path` at `commit` in `repo` as an image in GitHub markdown.
    pub fn raw_blob_url(&self, repo: &str, commit: &str, path: &str) -> String {
        format!(
            "{}/{repo}/blob/{commit}/{path}?raw=true",
            self.web().trim_end_matches('/')
        )
    }

    async fn review_item(
        &self,
        repo: &str,
        pull: ApiPull,
        me: &str,
    ) -> Result<ReviewItem, ApiError> {
        let sha = pull.head.map(|h| h.sha).unwrap_or_default();
        let reviews_path = format!("/repos/{repo}/pulls/{}/reviews", pull.number);
        let runs_path = format!("/repos/{repo}/commits/{sha}/check-runs");
        let reviews = self.get_all::<Vec<ApiReview>, ApiReview>(&reviews_path, |p| p);
        let runs = async {
            if sha.is_empty() {
                return Vec::new();
            }
            self.get_all::<ApiCheckRuns, ApiCheckRun>(&runs_path, |p| p.check_runs)
                .await
                .unwrap_or_default()
        };
        let (reviews, runs) = tokio::join!(reviews, runs);
        let my_state = my_review_state(&reviews?, me, &sha);
        let mine = pull
            .user
            .as_ref()
            .is_some_and(|u| u.login.eq_ignore_ascii_case(me));
        let passed = runs
            .iter()
            .filter(|r| {
                r.status == "completed"
                    && matches!(
                        r.conclusion.as_deref(),
                        Some("success" | "neutral" | "skipped")
                    )
            })
            .count();
        let state = if pull.merged_at.is_some() {
            "merged"
        } else if pull.state.as_deref() == Some("closed") {
            "closed"
        } else {
            "open"
        };
        Ok(ReviewItem {
            number: pull.number,
            title: pull.title,
            author: pull.user.map_or_else(|| "ghost".to_string(), |u| u.login),
            url: pull.html_url,
            updated_at: pull.updated_at,
            head_sha: sha,
            checks: aggregate_checks(&runs),
            checks_passed: passed as u64,
            checks_total: runs.len() as u64,
            my_state,
            mine,
            labels: pull
                .labels
                .iter()
                .map(|l| ReviewLabel {
                    name: l.name.clone(),
                    color: l.color.clone(),
                })
                .collect(),
            created_at: pull.created_at,
            comments: 0,
            state: state.to_string(),
            draft: pull.draft,
        })
    }

    /// The commit GitHub's `main` currently points at.
    pub async fn main_head(&self, repo: &str) -> Result<String, ApiError> {
        let branch: ApiBranch = self
            .get_json(&format!("/repos/{repo}/branches/main"))
            .await?;
        Ok(branch.commit.sha)
    }

    /// Production site URL. Prefers a successful deployment of GitHub's `main` tip; when the
    /// template only records PR previews (Cloudflare Workers), derives production from a recent
    /// preview URL and the project's worker `name`.
    pub async fn live_url(
        &self,
        repo: &str,
        worker_name: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        if let Ok(sha) = self.main_head(repo).await {
            if let Some(url) = self.preview_url(repo, &sha).await? {
                return Ok(Some(url));
            }
        }
        let Some(worker) = worker_name.filter(|n| !n.is_empty()) else {
            return Ok(None);
        };
        self.live_url_from_previews(repo, worker).await
    }

    /// Newest successful preview deployment's `environment_url`, rewritten to the production
    /// host (`{alias}-{worker}.…` → `{worker}.…`).
    async fn live_url_from_previews(
        &self,
        repo: &str,
        worker: &str,
    ) -> Result<Option<String>, ApiError> {
        let deployments: Vec<ApiDeployment> = self
            .get_json(&format!("/repos/{repo}/deployments?per_page=20"))
            .await?;
        for d in deployments {
            let Ok(statuses) = self
                .get_json::<Vec<ApiDeploymentStatus>>(&format!(
                    "/repos/{repo}/deployments/{}/statuses?per_page=1",
                    d.id
                ))
                .await
            else {
                continue;
            };
            let Some(latest) = statuses.into_iter().next() else {
                continue;
            };
            if latest.state != "success" {
                continue;
            }
            let Some(url) = latest.environment_url.filter(|u| is_http_url(u)) else {
                continue;
            };
            if let Some(prod) = production_url_from_preview(&url, worker) {
                return Ok(Some(prod));
            }
        }
        Ok(None)
    }

    /// Just the [`PrState`] and head of `head_branch`'s pull request (no checks or deployments).
    pub async fn pr_ref(&self, repo: &str, head_branch: &str) -> Result<Option<PrRef>, ApiError> {
        Ok(self
            .pull_and_state(repo, head_branch)
            .await?
            .map(|(pull, state, _)| PrRef {
                state,
                head_sha: pull.head.map(|h| h.sha).unwrap_or_default(),
            }))
    }

    /// Newest pull request for `head_branch` in any state, with its reviews folded into a state.
    async fn pull_and_state(
        &self,
        repo: &str,
        head_branch: &str,
    ) -> Result<Option<(ApiPull, PrState, Option<SentBack>)>, ApiError> {
        let owner = repo
            .split_once('/')
            .map(|(o, _)| o)
            .ok_or_else(|| ApiError::badreq(format!("not a GitHub repository: {repo:?}")))?;
        // Any state: a merged or closed PR still describes the change.
        let pulls: Vec<ApiPull> = self
            .get_json(&format!(
                "/repos/{repo}/pulls?head={}:{}&state=all&sort=created&direction=desc",
                urlencode(owner),
                urlencode(head_branch)
            ))
            .await?;
        let Some(pull) = pulls.into_iter().next() else {
            return Ok(None);
        };
        let facts = PullFacts {
            draft: pull.draft,
            closed: pull.state.as_deref() == Some("closed"),
            merged: pull.merged_at.is_some(),
        };
        // Reviews only matter for an open, ready PR.
        let reviews: Vec<ApiReview> = if facts.draft || facts.closed {
            Vec::new()
        } else {
            self.get_all::<Vec<ApiReview>, ApiReview>(
                &format!("/repos/{repo}/pulls/{}/reviews", pull.number),
                |p| p,
            )
            .await?
        };
        let state = pr_state(&facts, &reviews);
        let back = if state == PrState::ChangesRequested {
            sent_back(&reviews)
        } else {
            None
        };
        Ok(Some((pull, state, back)))
    }

    /// `environment_url` of the newest deployment of `sha` whose latest status is a success.
    pub async fn preview_url(&self, repo: &str, sha: &str) -> Result<Option<String>, ApiError> {
        let deployments: Vec<ApiDeployment> = self
            .get_json(&format!(
                "/repos/{repo}/deployments?sha={}&per_page=10",
                urlencode(sha)
            ))
            .await?;
        for d in deployments.iter().filter(|d| d.sha == sha) {
            // A deployment whose statuses can't be read is skipped, not fatal.
            let Ok(statuses) = self
                .get_json::<Vec<ApiDeploymentStatus>>(&format!(
                    "/repos/{repo}/deployments/{}/statuses?per_page=1",
                    d.id
                ))
                .await
            else {
                continue;
            };
            // Statuses are newest first.
            if let Some(latest) = statuses.into_iter().next() {
                if latest.state == "success" {
                    if let Some(url) = latest.environment_url.filter(|u| is_http_url(u)) {
                        return Ok(Some(url));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Whether the deploy that runs on `sha` (a publish's merge commit) has shipped, from
    /// that commit's check runs. Never an error: a site or token GitHub can't describe stays
    /// [`DeployState::Unknown`] rather than making a publish look failed.
    pub async fn deploy_state(&self, repo: &str, sha: &str) -> DeployState {
        let runs_path = format!("/repos/{repo}/commits/{sha}/check-runs");
        match self
            .get_all::<ApiCheckRuns, ApiCheckRun>(&runs_path, |p| p.check_runs)
            .await
        {
            Ok(runs) => deploy_state_of(&runs),
            Err(_) => DeployState::Unknown,
        }
    }

    /// [`Self::pr_ref`] of each branch, looked up concurrently. Best effort: an error, or a
    /// lookup slower than `per_branch`, yields `None` for that branch.
    pub async fn pr_states(
        &self,
        repo: &str,
        branches: &[String],
        per_branch: Duration,
    ) -> Vec<Option<PrRef>> {
        futures_util::future::join_all(branches.iter().map(|b| async move {
            match tokio::time::timeout(per_branch, self.pr_ref(repo, b)).await {
                Ok(Ok(state)) => state,
                _ => None,
            }
        }))
        .await
    }

    /// Every item of a paginated list endpoint (`path` has no query string): pages of
    /// [`PAGE_SIZE`] until a short page, up to [`MAX_PAGES`].
    async fn get_all<P: DeserializeOwned, T>(
        &self,
        path: &str,
        items: impl Fn(P) -> Vec<T>,
    ) -> Result<Vec<T>, ApiError> {
        self.get_all_query(path, "", items).await
    }

    /// [`Self::get_all`] with extra query parameters (`extra` is `k=v&` pairs, or empty).
    async fn get_all_query<P: DeserializeOwned, T>(
        &self,
        path: &str,
        extra: &str,
        items: impl Fn(P) -> Vec<T>,
    ) -> Result<Vec<T>, ApiError> {
        let mut all = Vec::new();
        for page in 1..=MAX_PAGES {
            let batch = items(
                self.get_json::<P>(&format!("{path}?{extra}per_page={PAGE_SIZE}&page={page}"))
                    .await?,
            );
            let short = batch.len() < PAGE_SIZE;
            all.extend(batch);
            if short {
                break;
            }
        }
        Ok(all)
    }

    /// A GET of `path`. With a token it goes through the GET cache; through `gh api` it
    /// doesn't.
    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let Some(token) = self.token().await else {
            return self.api_json(reqwest::Method::GET, path, None).await;
        };
        let body = self.cached_get(&token, path).await?;
        serde_json::from_slice(&body)
            .map_err(|_| ApiError::github("unexpected response from GitHub"))
    }

    /// The body of a token GET of `path`: reused while fresh, revalidated once stale, and
    /// fetched once for concurrent identical calls.
    async fn cached_get(&self, token: &str, path: &str) -> Result<bytes::Bytes, ApiError> {
        let url = format!("{}{path}", self.api());
        let key = crate::github_cache::GetCache::key(token, &url);
        let flight = self.get_cache.flight(&key).await;
        let out = self.cached_get_in_flight(token, &url, &key).await;
        self.get_cache.land(flight).await;
        out
    }

    async fn cached_get_in_flight(
        &self,
        token: &str,
        url: &str,
        key: &str,
    ) -> Result<bytes::Bytes, ApiError> {
        use crate::github_cache::Lookup;
        use reqwest::header::{IF_MODIFIED_SINCE, IF_NONE_MATCH};
        let (etag, last_modified) = match self.get_cache.lookup(key) {
            Lookup::Fresh(body) => return Ok(body),
            Lookup::Stale {
                etag,
                last_modified,
            } => (etag, last_modified),
            Lookup::Miss => (None, None),
        };
        let get = || {
            self.http
                .get(url)
                .bearer_auth(token)
                .header(ACCEPT, GITHUB_JSON)
        };
        let mut req = get();
        if let Some(etag) = &etag {
            req = req.header(IF_NONE_MATCH, etag);
        }
        if let Some(since) = &last_modified {
            req = req.header(IF_MODIFIED_SINCE, since);
        }
        let res = req.send().await.map_err(unreachable)?;
        if res.status() == reqwest::StatusCode::NOT_MODIFIED {
            if let Some(body) = self.get_cache.revalidated(key) {
                return Ok(body);
            }
            // Evicted in the meantime: ask again without validators.
            let res = get().send().await.map_err(unreachable)?;
            return self.store_get(res, token, url, key).await;
        }
        self.store_get(res, token, url, key).await
    }

    /// A GET response's body, stored when 2xx; otherwise the same errors as `parse_github`.
    async fn store_get(
        &self,
        res: reqwest::Response,
        token: &str,
        url: &str,
        key: &str,
    ) -> Result<bytes::Bytes, ApiError> {
        use reqwest::header::{ETAG, LAST_MODIFIED};
        let status = res.status();
        let header = |name| {
            res.headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        let (etag, last_modified) = (header(ETAG), header(LAST_MODIFIED));
        let body = res.bytes().await.map_err(unreachable)?;
        if status == reqwest::StatusCode::UNAUTHORIZED {
            let _ = self.sign_out_if_token(token).await;
            return Err(ApiError::unauth(
                "your GitHub session expired — sign in again",
            ));
        }
        if !status.is_success() {
            return Err(github_error(status, &body));
        }
        self.get_cache
            .store(key, url, body.clone(), etag, last_modified);
        Ok(body)
    }

    /// After a write to `path`, cached GETs of its repository (or, for a path that names
    /// none, every cached GET) may be out of date.
    fn forget_after_write(&self, path: &str) {
        match crate::github_cache::repo_prefix(path) {
            Some(repo) => self.get_cache.invalidate(&format!("{}{repo}", self.api())),
            None => self.get_cache.clear(),
        }
    }

    /// A REST call to `path` with the session token, else through `gh api` when only the
    /// GitHub CLI is logged in. A write also drops the cached GETs it may have changed.
    async fn api_json<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<T, ApiError> {
        let write = method != reqwest::Method::GET;
        let out = self.api_json_uncached(method, path, body).await;
        if write {
            self.forget_after_write(path);
        }
        out
    }

    async fn api_json_uncached<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<T, ApiError> {
        if let Some(token) = self.token().await {
            let mut req = self
                .http
                .request(method, format!("{}{path}", self.api()))
                .bearer_auth(&token)
                .header(ACCEPT, GITHUB_JSON);
            if let Some(body) = body {
                req = req.json(body);
            }
            let res = req.send().await.map_err(unreachable)?;
            return parse_github(res, self, &token).await;
        }
        let out = self.gh_api(method.as_str(), path, body).await?;
        if out.code != Some(0) {
            return Err(self.gh_api_error(out).await);
        }
        serde_json::from_slice(&out.stdout)
            .map_err(|_| ApiError::github("unexpected response from the GitHub CLI"))
    }

    /// Like [`Self::api_json`], but returns GitHub's status and body instead of mapping a
    /// failure to an error, so the caller can say what a refusal means for its own action.
    async fn api_raw(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(reqwest::StatusCode, Vec<u8>), ApiError> {
        let write = method != reqwest::Method::GET;
        let out = self.api_raw_uncached(method, path, body).await;
        if write {
            self.forget_after_write(path);
        }
        out
    }

    async fn api_raw_uncached(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(reqwest::StatusCode, Vec<u8>), ApiError> {
        if let Some(token) = self.token().await {
            let mut req = self
                .http
                .request(method, format!("{}{path}", self.api()))
                .bearer_auth(&token)
                .header(ACCEPT, GITHUB_JSON);
            if let Some(body) = body {
                req = req.json(body);
            }
            let res = req.send().await.map_err(unreachable)?;
            let status = res.status();
            let bytes = res.bytes().await.map_err(unreachable)?.to_vec();
            if status == reqwest::StatusCode::UNAUTHORIZED {
                let _ = self.sign_out_if_token(&token).await;
                return Err(ApiError::unauth(
                    "your GitHub session expired — sign in again",
                ));
            }
            return Ok((status, bytes));
        }
        let out = self.gh_raw(method.as_str(), path, body).await?;
        if out.logged_out() {
            *self.cli.lock().await = None;
            return Err(not_signed_in());
        }
        match (out.code, gh_http_status(&out.stderr)) {
            (Some(0), _) => Ok((reqwest::StatusCode::OK, out.stdout)),
            (_, Some(status)) => Ok((status, out.stdout)),
            _ => {
                let tail: String = out.stderr.chars().take(300).collect();
                Err(ApiError::github(format!("GitHub CLI: {tail}")).with_status(502))
            }
        }
    }

    /// `gh api --method {method} {path}`, with `body` as the JSON request body.
    async fn gh_api(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<GhOutput, ApiError> {
        let out = self.gh_raw(method, path, body).await?;
        if out.code != Some(0) {
            return Err(self.gh_api_error(out).await);
        }
        Ok(out)
    }

    /// Run `gh api` without interpreting its exit code (see [`Self::gh_api`] / [`Self::api_raw`]).
    async fn gh_raw(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<GhOutput, ApiError> {
        if self.cli_state().await.user.is_none() {
            return Err(not_signed_in());
        }
        let input = body.map(|b| b.to_string().into_bytes());
        let mut args = vec!["api", "--method", method, path];
        if input.is_some() {
            args.extend(["--input", "-"]);
        }
        self.run_gh_input(&args, 30, input.as_deref())
            .await
            .map_err(|f| f.err.with_status(502))
    }

    /// The error for a failed `gh api`: GitHub's HTTP status (from gh's stderr) is mapped
    /// like a direct API response; a logged-out gh drops the cached login.
    async fn gh_api_error(&self, out: GhOutput) -> ApiError {
        if out.logged_out() {
            *self.cli.lock().await = None;
            return not_signed_in();
        }
        match gh_http_status(&out.stderr) {
            Some(status) => github_error(status, &out.stdout),
            None => {
                let tail: String = out.stderr.chars().take(300).collect();
                ApiError::github(format!("GitHub CLI: {tail}")).with_status(502)
            }
        }
    }

    /// Branch protection of `main` on `owner/name`, from the classic protection endpoint
    /// and repository rulesets (both read concurrently):
    /// - protected: classic rule exists (200), or an active ruleset requires a pull request;
    /// - unprotected: classic says 404 "Branch not protected" and no ruleset requires a PR;
    /// - unknown: anything else (403 without admin rights, a 404 "Not Found" for a repo we
    ///   cannot see, errors) or no answer within [`PROTECTION_TIMEOUT`]. Never fails.
    pub async fn main_protection(&self, repo: &str) -> BranchProtection {
        tokio::time::timeout(PROTECTION_TIMEOUT, self.lookup_main_protection(repo))
            .await
            .unwrap_or(BranchProtection::Unknown)
    }

    async fn lookup_main_protection(&self, repo: &str) -> BranchProtection {
        let Some(token) = self.token().await else {
            return BranchProtection::Unknown;
        };
        let (classic, rules_need_pr) = tokio::join!(
            self.classic_main_protection(repo, &token),
            self.rulesets_require_pr(repo, &token),
        );
        match (classic, rules_need_pr) {
            (BranchProtection::Protected, _) | (_, Some(true)) => BranchProtection::Protected,
            (BranchProtection::Unprotected, Some(false)) => BranchProtection::Unprotected,
            _ => BranchProtection::Unknown,
        }
    }

    /// `GET /repos/{repo}/branches/main/protection`.
    async fn classic_main_protection(&self, repo: &str, token: &str) -> BranchProtection {
        let res = self
            .http
            .get(format!(
                "{}/repos/{repo}/branches/main/protection",
                self.api()
            ))
            .bearer_auth(token)
            .header(ACCEPT, GITHUB_JSON)
            .send()
            .await;
        let Ok(res) = res else {
            return BranchProtection::Unknown;
        };
        let status = res.status();
        if status.is_success() {
            return BranchProtection::Protected;
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            #[derive(Deserialize)]
            struct Message {
                message: Option<String>,
            }
            let msg = res.json::<Message>().await.ok().and_then(|m| m.message);
            if msg.as_deref() == Some("Branch not protected") {
                return BranchProtection::Unprotected;
            }
        }
        BranchProtection::Unknown
    }

    /// `GET /repos/{repo}/rules/branches/main`: whether an active ruleset requires a pull
    /// request. `None` when the rules could not be read.
    async fn rulesets_require_pr(&self, repo: &str, token: &str) -> Option<bool> {
        #[derive(Deserialize)]
        struct Rule {
            #[serde(rename = "type")]
            kind: String,
        }
        let res = self
            .http
            .get(format!("{}/repos/{repo}/rules/branches/main", self.api()))
            .bearer_auth(token)
            .header(ACCEPT, GITHUB_JSON)
            .send()
            .await
            .ok()?;
        if !res.status().is_success() {
            return None;
        }
        let rules: Vec<Rule> = res.json().await.ok()?;
        Some(rules.iter().any(|r| r.kind == "pull_request"))
    }

    /// The repo's branch protection settings page on the GitHub web UI.
    pub fn branch_settings_url(&self, repo: &str) -> String {
        format!("{}/{repo}/settings/branches", self.web())
    }

    fn client_id(&self) -> Result<&str, ApiError> {
        self.cfg.client_id.as_deref().ok_or_else(|| {
            ApiError::github(
                "GitHub sign-in is not configured: start the daemon with --github-client-id",
            )
            .with_status(400)
        })
    }

    fn web(&self) -> &str {
        self.cfg.web_url.trim_end_matches('/')
    }

    fn api(&self) -> &str {
        self.cfg.api_url.trim_end_matches('/')
    }
}

/// Write `bytes` to `path` readable only by this user, atomically.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), ApiError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    // A leftover file could carry looser permissions; `create_new` also refuses a symlink.
    match std::fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

async fn parse<T: DeserializeOwned>(res: reqwest::Response) -> Result<T, ApiError> {
    let status = res.status();
    let body = res.bytes().await.map_err(unreachable)?;
    if !status.is_success() {
        let text: String = String::from_utf8_lossy(&body).chars().take(300).collect();
        return Err(ApiError::github(format!(
            "GitHub returned {status}: {text}"
        )));
    }
    // Not serde's message: it can quote parts of the body (e.g. a token).
    serde_json::from_slice(&body).map_err(|_| ApiError::github("unexpected response from GitHub"))
}

/// Like [`parse`], but maps 401/403/422 to distinct [`ApiError`] codes and clears an
/// expired oauth session on 401.
async fn parse_github<T: DeserializeOwned>(
    res: reqwest::Response,
    auth: &GithubAuth,
    token: &str,
) -> Result<T, ApiError> {
    let status = res.status();
    let body = res.bytes().await.map_err(unreachable)?;
    if status == reqwest::StatusCode::UNAUTHORIZED {
        let _ = auth.sign_out_if_token(token).await;
        return Err(ApiError::unauth(
            "your GitHub session expired — sign in again",
        ));
    }
    if !status.is_success() {
        return Err(github_error(status, &body));
    }
    serde_json::from_slice(&body).map_err(|_| ApiError::github("unexpected response from GitHub"))
}

/// Why images can't be stored in the repository: the account may not create or move the
/// assets branch (no push access, or a ruleset restricting branches).
pub const ASSETS_REFUSED: &str = "Couldn't upload images to this repository";

/// A refused write to the assets branch (403, 404 or 422) is `assets_refused`, so the reviewer
/// is offered to send without images; anything else is an ordinary GitHub error.
fn assets_write_error(status: reqwest::StatusCode, body: &[u8]) -> ApiError {
    match status {
        reqwest::StatusCode::FORBIDDEN
        | reqwest::StatusCode::NOT_FOUND
        | reqwest::StatusCode::UNPROCESSABLE_ENTITY => {
            ApiError::sandbox(ASSETS_REFUSED).with_reason("assets_refused")
        }
        _ => github_error(status, body),
    }
}

/// A failed (non-401) GitHub API response: 403 and 422 get their own [`ApiError`] codes.
fn github_error(status: reqwest::StatusCode, body: &[u8]) -> ApiError {
    if status == reqwest::StatusCode::FORBIDDEN {
        return ApiError::sandbox("You don't have push access");
    }
    if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
        let text: String = String::from_utf8_lossy(body).chars().take(200).collect();
        return ApiError::conflict(format!("GitHub couldn't share this change: {text}"));
    }
    let text: String = String::from_utf8_lossy(body).chars().take(300).collect();
    ApiError::github(format!("GitHub returned {status}: {text}"))
}

/// The `ApiError` for a failed publish (merge) response, in writer-plain words: permission
/// refusals ask an owner, merge refusals suggest getting latest, anything else offers a retry.
fn publish_refused(status: reqwest::StatusCode) -> ApiError {
    if status == reqwest::StatusCode::FORBIDDEN {
        return ApiError::sandbox("Ask an owner to add you to this site, then try again.");
    }
    if status == reqwest::StatusCode::METHOD_NOT_ALLOWED
        || status == reqwest::StatusCode::CONFLICT
        || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
    {
        return ApiError::conflict("Couldn't publish. Get latest, then try again.");
    }
    publish_failed()
}

/// A publish that GitHub accepted but did nothing with, or answered unexpectedly.
fn publish_failed() -> ApiError {
    ApiError::github("Couldn't publish. Try again in a moment.")
}

/// The HTTP status in a failed `gh api`'s stderr, e.g. `gh: Not Found (HTTP 404)`.
fn gh_http_status(stderr: &str) -> Option<reqwest::StatusCode> {
    let rest = &stderr[stderr.find("(HTTP ")? + "(HTTP ".len()..];
    let code: u16 = rest.get(..3)?.parse().ok()?;
    reqwest::StatusCode::from_u16(code).ok()
}

fn unreachable(e: reqwest::Error) -> ApiError {
    ApiError::github(format!("cannot reach GitHub: {e}"))
}

pub fn not_signed_in() -> ApiError {
    ApiError::github("sign in with GitHub first")
        .with_status(400)
        .with_reason("github_sign_in")
}

/// `owner/name` with GitHub's allowed characters; returns it trimmed.
pub fn validate_repo(s: &str) -> Result<String, ApiError> {
    let s = s.trim();
    let ok = s
        .split_once('/')
        .is_some_and(|(owner, name)| valid_part(owner) && valid_part(name));
    if ok && !s.ends_with(".git") {
        Ok(s.to_string())
    } else {
        Err(ApiError::badreq(format!(
            "not a GitHub repository (owner/name): {s:?}"
        )))
    }
}

fn valid_part(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 100
        && p != "."
        && p != ".."
        && !p.starts_with('-')
        && p.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", urlencode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Environment for one git command that authenticates HTTPS requests to `git_url` the way
/// `actions/checkout` does, without touching any config file or argv.
pub fn git_auth_env(git_url: &str, token: &str) -> Vec<(String, String)> {
    let basic = base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{token}"));
    let base = git_url.trim_end_matches('/');
    let key = format!("http.{base}/.extraheader");
    vec![
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_ASKPASS".into(), String::new()),
        ("GIT_CONFIG_COUNT".into(), "3".into()),
        // Reset any user-level extraheader for this host, then set ours.
        ("GIT_CONFIG_KEY_0".into(), key.clone()),
        ("GIT_CONFIG_VALUE_0".into(), String::new()),
        ("GIT_CONFIG_KEY_1".into(), key),
        (
            "GIT_CONFIG_VALUE_1".into(),
            format!("AUTHORIZATION: basic {basic}"),
        ),
        ("GIT_CONFIG_KEY_2".into(), "credential.helper".into()),
        ("GIT_CONFIG_VALUE_2".into(), String::new()),
    ]
}

/// Environment for one git command that gets HTTPS credentials for `git_url` from
/// `gh auth git-credential`. The token never passes through this process.
pub fn git_cli_env(git_url: &str, gh: &Path) -> Vec<(String, String)> {
    let key = format!("credential.{}.helper", git_url.trim_end_matches('/'));
    let gh = gh.to_string_lossy();
    let gh = if gh
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"/._-+@:".contains(&b))
    {
        gh.into_owned()
    } else {
        format!("'{}'", gh.replace('\'', "'\\''"))
    };
    vec![
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_ASKPASS".into(), String::new()),
        ("GIT_CONFIG_COUNT".into(), "2".into()),
        // Reset any helpers for this host, then ask gh.
        ("GIT_CONFIG_KEY_0".into(), key.clone()),
        ("GIT_CONFIG_VALUE_0".into(), String::new()),
        ("GIT_CONFIG_KEY_1".into(), key),
        (
            "GIT_CONFIG_VALUE_1".into(),
            format!("!{gh} auth git-credential"),
        ),
    ]
}

/// `git config --global user.name/email`, when both are set. `global` overrides the
/// global config file (tests); `None` uses the real one.
async fn global_git_identity(global: Option<&Path>) -> Option<Identity> {
    async fn get(key: &str, global: Option<&Path>) -> Option<String> {
        let mut cmd = tokio::process::Command::new("git");
        if let Some(g) = global {
            cmd.env("GIT_CONFIG_GLOBAL", g);
        }
        cmd.args(["config", "--global", "--get", key])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let out = tokio::time::timeout(Duration::from_secs(5), cmd.output())
            .await
            .ok()?
            .ok()?;
        let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (out.status.success() && !v.is_empty()).then_some(v)
    }
    Some(Identity {
        name: get("user.name", global).await?,
        email: get("user.email", global).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    #[test]
    fn draft_pr_copy_uses_change_title() {
        assert_eq!(pr_title("Launch week"), "Launch week");
        assert_eq!(
            pr_body("Launch week", None),
            "Preview of change: Launch week"
        );
        assert_eq!(
            pr_body("Launch week", Some("  ")),
            "Preview of change: Launch week"
        );
        assert_eq!(
            pr_body("Launch week", Some(" New hero copy and pricing table. ")),
            "New hero copy and pricing table."
        );
    }

    #[test]
    fn repo_names_are_owner_slash_name() {
        assert_eq!(validate_repo("acme/site").unwrap(), "acme/site");
        assert_eq!(validate_repo("acme/.github").unwrap(), "acme/.github");
        assert_eq!(
            validate_repo(" Acme-Co/site.docs_2 ").unwrap(),
            "Acme-Co/site.docs_2"
        );
        for bad in [
            "",
            "acme",
            "acme/",
            "/site",
            "a/b/c",
            "acme/..",
            "acme/si te",
            "-x/../y",
            "acme/site.git",
            "acme/.",
            "./site",
        ] {
            assert!(validate_repo(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn repo_info_exposes_the_push_time() {
        let api: ApiRepo = serde_json::from_str(
            r#"{"full_name":"acme/site","private":false,"default_branch":"main","pushed_at":"2026-10-20T12:00:00Z"}"#,
        )
        .unwrap();
        let info = RepoInfo::from(api);
        assert_eq!(info.pushed_at.as_deref(), Some("2026-10-20T12:00:00Z"));
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["pushedAt"], "2026-10-20T12:00:00Z");
    }

    #[test]
    fn repo_info_tolerates_a_missing_push_time() {
        let api: ApiRepo = serde_json::from_str(
            r#"{"full_name":"acme/site","private":false,"default_branch":"main"}"#,
        )
        .unwrap();
        assert_eq!(RepoInfo::from(api).pushed_at, None);
    }

    #[test]
    fn repo_info_tolerates_a_null_push_time() {
        let api: ApiRepo = serde_json::from_str(
            r#"{"full_name":"acme/site","private":false,"default_branch":"main","pushed_at":null}"#,
        )
        .unwrap();
        assert_eq!(RepoInfo::from(api).pushed_at, None);
    }

    #[test]
    fn form_bodies_are_percent_encoded() {
        assert_eq!(
            form(&[("client_id", "Iv1.abc"), ("scope", "repo read:user")]),
            "client_id=Iv1.abc&scope=repo%20read%3Auser"
        );
    }

    #[test]
    fn git_auth_env_uses_an_extraheader_and_disables_prompts() {
        let env = git_auth_env("https://github.com/", "tok");
        let get = |k: &str| env.iter().find(|(a, _)| a == k).map(|(_, b)| b.as_str());
        assert_eq!(get("GIT_TERMINAL_PROMPT"), Some("0"));
        assert_eq!(get("GIT_ASKPASS"), Some(""));
        assert_eq!(get("GIT_CONFIG_COUNT"), Some("3"));
        let key = "http.https://github.com/.extraheader";
        assert_eq!(get("GIT_CONFIG_KEY_0"), Some(key));
        assert_eq!(get("GIT_CONFIG_VALUE_0"), Some(""));
        assert_eq!(get("GIT_CONFIG_KEY_1"), Some(key));
        // base64("x-access-token:tok")
        assert_eq!(
            get("GIT_CONFIG_VALUE_1"),
            Some("AUTHORIZATION: basic eC1hY2Nlc3MtdG9rZW46dG9r")
        );
        assert_eq!(get("GIT_CONFIG_KEY_2"), Some("credential.helper"));
        assert_eq!(get("GIT_CONFIG_VALUE_2"), Some(""));
    }

    #[test]
    fn identity_prefers_the_profile_name_and_uses_the_noreply_email() {
        let u = GithubUser {
            login: "ada".into(),
            id: 42,
            name: Some("Ada Lovelace".into()),
            avatar_url: String::new(),
        };
        assert_eq!(u.identity().name, "Ada Lovelace");
        assert_eq!(u.identity().email, "42+ada@users.noreply.github.com");
        let u = GithubUser {
            name: Some("  ".into()),
            ..u
        };
        assert_eq!(u.identity().name, "ada");
    }

    #[tokio::test]
    async fn opens_without_a_session_and_reports_unconfigured() {
        let dir = std::env::temp_dir().join(format!("wb-gh-{}", std::process::id()));
        let auth = GithubAuth::open(test_cfg(), &dir);
        let st = auth.status().await;
        assert!(!st.configured);
        assert!(st.user.is_none());
        assert_eq!(auth.identity().await, Identity::orso());
    }

    /// Never the real `gh`.
    fn test_cfg() -> GithubConfig {
        GithubConfig {
            gh_path: PathBuf::new(),
            ..GithubConfig::default()
        }
    }

    #[test]
    fn git_cli_env_asks_gh_for_credentials_for_the_host_only() {
        let env = git_cli_env("https://github.com/", Path::new("/opt/bin/gh"));
        let get = |k: &str| env.iter().find(|(a, _)| a == k).map(|(_, b)| b.as_str());
        assert_eq!(get("GIT_TERMINAL_PROMPT"), Some("0"));
        assert_eq!(get("GIT_CONFIG_COUNT"), Some("2"));
        let key = "credential.https://github.com.helper";
        assert_eq!(get("GIT_CONFIG_KEY_0"), Some(key));
        assert_eq!(get("GIT_CONFIG_VALUE_0"), Some(""));
        assert_eq!(get("GIT_CONFIG_KEY_1"), Some(key));
        assert_eq!(
            get("GIT_CONFIG_VALUE_1"),
            Some("!/opt/bin/gh auth git-credential")
        );
        let env = git_cli_env("https://github.com", Path::new("/a b/gh"));
        assert!(env
            .iter()
            .any(|(_, v)| v == "!'/a b/gh' auth git-credential"));
    }

    fn write_gh(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("gh");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    const OK_GH: &str = r#"printf '%s' '{"login":"ada","id":42,"name":"Ada","avatar_url":"a"}'"#;

    async fn expire(auth: &GithubAuth) {
        auth.cli.lock().await.as_mut().unwrap().expires = Instant::now();
    }

    #[tokio::test]
    async fn a_transient_gh_failure_keeps_the_last_login_but_a_logout_clears_it() {
        let dir = tmp_dir("stale");
        let gh = write_gh(&dir, OK_GH);
        let cfg = GithubConfig {
            gh_path: gh.clone(),
            git_global_config: Some(dir.join("none")),
            ..GithubConfig::default()
        };
        let auth = GithubAuth::open(cfg, &dir);
        assert_eq!(auth.status().await.user.unwrap().login, "ada");
        let before = auth.identity().await;

        // Network error, exit 1: still signed in, identity unchanged.
        write_gh(&dir, "echo 'dial tcp: i/o timeout' >&2; exit 1");
        expire(&auth).await;
        let st = auth.status().await;
        assert_eq!(st.user.unwrap().login, "ada");
        assert_eq!(st.source, Some("cli"));
        assert_eq!(auth.identity().await, before);
        // ...and it is retried soon, not after a minute.
        let left = auth.cli.lock().await.as_ref().unwrap().expires - Instant::now();
        assert!(left <= CLI_STALE_RETRY);

        // Exit 4: logged out.
        write_gh(
            &dir,
            "echo 'To get started, run: gh auth login' >&2; exit 4",
        );
        expire(&auth).await;
        assert!(auth.status().await.user.is_none());
        assert_eq!(auth.identity().await, Identity::orso());
        let left = auth.cli.lock().await.as_ref().unwrap().expires - Instant::now();
        assert!(left <= CLI_RETRY);
    }

    #[tokio::test]
    async fn a_failing_gh_without_a_previous_login_is_signed_out_and_rechecked_soon() {
        let dir = tmp_dir("nofirst");
        let gh = write_gh(&dir, "exit 1");
        let cfg = GithubConfig {
            gh_path: gh,
            ..GithubConfig::default()
        };
        let auth = GithubAuth::open(cfg, &dir);
        assert!(auth.status().await.user.is_none());
        let left = auth.cli.lock().await.as_ref().unwrap().expires - Instant::now();
        assert!(left <= CLI_RETRY);
    }

    fn tmp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wb-gh-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn ada() -> GithubUser {
        GithubUser {
            login: "ada".into(),
            id: 42,
            name: Some("Ada Lovelace".into()),
            avatar_url: "https://a/x.png".into(),
        }
    }

    #[test]
    fn write_private_resets_permissions_of_a_stale_tmp_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmp_dir("perm");
        let path = dir.join("github.json");
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, "old").unwrap();
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&path, b"secret").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(std::fs::read(&path).unwrap(), b"secret");
    }

    #[tokio::test]
    async fn commit_session_refuses_after_sign_out_or_a_new_flow() {
        let dir = tmp_dir("race");
        let auth = GithubAuth::open(test_cfg(), &dir);
        let pending = |code: &str| Pending {
            device_code: code.into(),
            expires_at: Instant::now() + Duration::from_secs(60),
            interval: 5,
        };
        let session = || Session {
            token: "tok".into(),
            user: ada(),
        };

        // Signed out while the poll was in flight.
        auth.inner.lock().await.pending = Some(pending("dc1"));
        auth.sign_out().await;
        assert!(!auth.commit_session("dc1", session()).await.unwrap());
        assert!(!dir.join("github.json").exists());
        assert!(auth.token().await.is_none());

        // A newer flow replaced the one being polled.
        auth.inner.lock().await.pending = Some(pending("dc2"));
        assert!(!auth.commit_session("dc1", session()).await.unwrap());
        assert!(!dir.join("github.json").exists());

        // Still the same flow: committed.
        assert!(auth.commit_session("dc2", session()).await.unwrap());
        assert!(dir.join("github.json").exists());
        assert_eq!(auth.token().await.as_deref(), Some("tok"));
        assert!(auth.inner.lock().await.pending.is_none());
    }

    #[tokio::test]
    async fn sign_out_if_token_only_clears_the_matching_session() {
        let dir = tmp_dir("signout-if");
        let auth = GithubAuth::open(test_cfg(), &dir);
        auth.inner.lock().await.pending = Some(Pending {
            device_code: "dc".into(),
            expires_at: Instant::now() + Duration::from_secs(60),
            interval: 5,
        });
        let session = Session {
            token: "new".into(),
            user: ada(),
        };
        assert!(auth.commit_session("dc", session).await.unwrap());

        assert!(!auth.sign_out_if_token("old").await);
        assert_eq!(auth.token().await.as_deref(), Some("new"));
        assert!(dir.join("github.json").exists());

        assert!(auth.sign_out_if_token("new").await);
        assert!(auth.token().await.is_none());
        assert!(!dir.join("github.json").exists());
    }

    #[test]
    fn poll_outcomes_serialize_to_the_web_contract() {
        let j = |o: PollOutcome| serde_json::to_value(o).unwrap();
        assert_eq!(
            j(PollOutcome::Pending),
            serde_json::json!({"status": "pending"})
        );
        assert_eq!(
            j(PollOutcome::SlowDown { interval: 10 }),
            serde_json::json!({"status": "slowDown", "interval": 10})
        );
        assert_eq!(
            j(PollOutcome::SignedIn { user: ada() }),
            serde_json::json!({
                "status": "signedIn",
                "user": {"login": "ada", "id": 42, "name": "Ada Lovelace", "avatarUrl": "https://a/x.png"}
            })
        );
        assert_eq!(
            j(PollOutcome::Expired),
            serde_json::json!({"status": "expired"})
        );
        assert_eq!(
            j(PollOutcome::Denied),
            serde_json::json!({"status": "denied"})
        );
    }

    fn write_session(dir: &Path, token: &str) {
        let user = ada();
        let json = serde_json::json!({
            "token": token,
            "user": {
                "login": user.login,
                "id": user.id,
                "name": user.name,
                "avatarUrl": user.avatar_url,
            }
        });
        std::fs::write(dir.join("github.json"), json.to_string()).unwrap();
    }

    async fn mock_api(routes: axum::Router) -> (String, impl FnOnce()) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, routes).await.ok();
        });
        (format!("http://{addr}"), move || {
            handle.abort();
        })
    }

    #[tokio::test]
    async fn find_pr_returns_existing_by_head_before_create() {
        use axum::routing::get;
        use axum::{Json, Router};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let creates = Arc::new(AtomicUsize::new(0));
        let creates2 = creates.clone();
        let routes = Router::new().route(
            "/repos/acme/site/pulls",
            get(|| async {
                Json(serde_json::json!([{
                    "number": 7,
                    "html_url": "https://github.com/acme/site/pull/7",
                    "title": "Launch week",
                    "draft": true
                }]))
            })
            .post(move || {
                let creates2 = creates2.clone();
                async move {
                    creates2.fetch_add(1, Ordering::SeqCst);
                    (
                        axum::http::StatusCode::CREATED,
                        Json(serde_json::json!({
                            "number": 99,
                            "html_url": "https://github.com/acme/site/pull/99",
                            "title": "new",
                            "draft": true
                        })),
                    )
                }
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let dir = tmp_dir("findpr");
        write_session(&dir, "tok");
        let auth = GithubAuth::open(
            GithubConfig {
                api_url: base.clone(),
                web_url: base,
                gh_path: PathBuf::new(),
                ..GithubConfig::default()
            },
            &dir,
        );

        let found = auth
            .find_pr("acme/site", "content/launch-week")
            .await
            .unwrap()
            .expect("existing PR");
        assert_eq!(found.number, 7);
        assert_eq!(found.html_url, "https://github.com/acme/site/pull/7");
        assert_eq!(creates.load(Ordering::SeqCst), 0);
        stop();
    }

    #[tokio::test]
    async fn create_draft_pr_sends_title_body_and_draft() {
        use axum::extract::Json as AxJson;
        use axum::routing::post;
        use axum::{Json, Router};
        use std::sync::{Arc, Mutex};

        let seen: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let seen2 = seen.clone();
        let labels: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let labels2 = labels.clone();
        let routes = Router::new()
            .route(
                "/repos/acme/site/issues/1/labels",
                post(move |AxJson(body): AxJson<serde_json::Value>| {
                    let labels2 = labels2.clone();
                    async move {
                        *labels2.lock().unwrap() = Some(body);
                        Json(serde_json::json!([{ "name": "orso" }]))
                    }
                }),
            )
            .route(
                "/repos/acme/site/pulls",
                post(move |AxJson(body): AxJson<serde_json::Value>| {
                    let seen2 = seen2.clone();
                    async move {
                        *seen2.lock().unwrap() = Some(body);
                        (
                            axum::http::StatusCode::CREATED,
                            Json(serde_json::json!({
                                "number": 1,
                                "html_url": "https://github.com/acme/site/pull/1",
                                "title": "Launch week",
                                "draft": true
                            })),
                        )
                    }
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let dir = tmp_dir("createdraft");
        write_session(&dir, "tok");
        let auth = GithubAuth::open(
            GithubConfig {
                api_url: base.clone(),
                web_url: base,
                gh_path: PathBuf::new(),
                ..GithubConfig::default()
            },
            &dir,
        );

        let title = pr_title("Launch week");
        let body = pr_body("Launch week", None);
        let pr = auth
            .create_draft_pr(
                "acme/site",
                "content/launch-week",
                &title,
                &body,
                &["orso".to_string()],
            )
            .await
            .unwrap();
        assert_eq!(pr.number, 1);
        let sent = seen.lock().unwrap().clone().unwrap();
        assert_eq!(sent["draft"], true);
        assert_eq!(sent["title"], "Launch week");
        assert_eq!(sent["body"], "Preview of change: Launch week");
        assert_eq!(sent["head"], "content/launch-week");
        assert_eq!(sent["base"], "main");
        let labelled = labels.lock().unwrap().clone().expect("PR labelled");
        assert_eq!(labelled["labels"], serde_json::json!(["orso"]));
        stop();
    }

    #[tokio::test]
    async fn create_draft_pr_survives_a_refused_label() {
        use axum::routing::post;
        use axum::{Json, Router};

        let routes = Router::new()
            .route(
                "/repos/acme/site/issues/2/labels",
                post(|| async { axum::http::StatusCode::FORBIDDEN }),
            )
            .route(
                "/repos/acme/site/pulls",
                post(|| async {
                    (
                        axum::http::StatusCode::CREATED,
                        Json(serde_json::json!({
                            "number": 2,
                            "html_url": "https://github.com/acme/site/pull/2",
                            "title": "X",
                            "draft": true
                        })),
                    )
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let dir = tmp_dir("createdraft-label-refused");
        write_session(&dir, "tok");
        let auth = GithubAuth::open(
            GithubConfig {
                api_url: base.clone(),
                web_url: base,
                gh_path: PathBuf::new(),
                ..GithubConfig::default()
            },
            &dir,
        );
        let pr = auth
            .create_draft_pr("acme/site", "content/x", "X", "b", &["orso".to_string()])
            .await
            .unwrap();
        assert_eq!(pr.number, 2);
        stop();
    }

    /// A logged-in fake `gh`: `api user` answers Ada; other calls log their args to
    /// `gh.calls`, their stdin to `gh.stdin`, and run `rest`.
    fn gh_with_api(dir: &Path, rest: &str) -> GithubAuth {
        let gh = write_gh(
            dir,
            &format!(
                r#"if [ "$1 $2" = "api user" ]; then {OK_GH}; exit 0; fi
echo "$@" >> "$0.calls"
cat > "$0.stdin"
{rest}"#
            ),
        );
        let cfg = GithubConfig {
            gh_path: gh,
            git_global_config: Some(dir.join("none")),
            ..GithubConfig::default()
        };
        GithubAuth::open(cfg, dir)
    }

    #[tokio::test]
    async fn without_a_session_pull_requests_go_through_gh() {
        let dir = tmp_dir("ghpr");
        let auth = gh_with_api(
            &dir,
            r#"case "$*" in
  *pulls\?head=*) printf '[]' ;;
  *) printf '%s' '{"number":7,"html_url":"https://github.com/acme/site/pull/7","title":"X","draft":true}' ;;
esac"#,
        );
        assert!(auth
            .find_pr("acme/site", "content/x")
            .await
            .unwrap()
            .is_none());
        let pr = auth
            .create_draft_pr(
                "acme/site",
                "content/x",
                "X",
                "Preview of change: X",
                &["orso".to_string()],
            )
            .await
            .unwrap();
        assert_eq!(pr.number, 7);

        let calls = std::fs::read_to_string(dir.join("gh.calls")).unwrap();
        assert!(calls
            .contains("api --method GET /repos/acme/site/pulls?head=acme:content%2Fx&state=open"));
        assert!(calls.contains("api --method POST /repos/acme/site/pulls --input -"));
        assert!(calls.contains("api --method POST /repos/acme/site/issues/7/labels --input -"));
        // `gh.stdin` holds the last call's body: the label request after the PR create.
        let sent: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("gh.stdin")).unwrap()).unwrap();
        assert_eq!(sent["labels"], serde_json::json!(["orso"]));
    }

    #[tokio::test]
    async fn no_labels_means_no_label_request() {
        let dir = tmp_dir("ghnolabels");
        let auth = gh_with_api(
            &dir,
            r#"printf '%s' '{"number":7,"html_url":"https://github.com/acme/site/pull/7","title":"X","draft":true}'"#,
        );
        auth.create_draft_pr("acme/site", "content/x", "X", "b", &[])
            .await
            .unwrap();
        let calls = std::fs::read_to_string(dir.join("gh.calls")).unwrap();
        assert!(calls.contains("api --method POST /repos/acme/site/pulls --input -"));
        assert!(!calls.contains("/labels"));
    }

    #[tokio::test]
    async fn gh_api_http_errors_map_like_direct_responses() {
        let dir = tmp_dir("gh422");
        let auth = gh_with_api(
            &dir,
            r#"printf '{"message":"Validation Failed"}'; echo 'gh: Validation Failed (HTTP 422)' >&2; exit 1"#,
        );
        let err = auth
            .create_draft_pr("acme/site", "content/x", "X", "b", &["orso".to_string()])
            .await
            .unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::Conflict);
    }

    #[tokio::test]
    async fn without_a_session_mark_ready_goes_through_gh_graphql() {
        let dir = tmp_dir("ghready");
        let auth = gh_with_api(
            &dir,
            r#"printf '%s' '{"data":{"markPullRequestReadyForReview":{"pullRequest":{"isDraft":false}}}}'"#,
        );
        auth.mark_ready_mutation("PR_1").await.unwrap();
        let calls = std::fs::read_to_string(dir.join("gh.calls")).unwrap();
        assert!(calls.contains("api --method POST graphql --input -"));
        let sent: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("gh.stdin")).unwrap()).unwrap();
        assert_eq!(sent["variables"]["id"], "PR_1");
    }

    #[test]
    fn gh_http_status_reads_gh_error_suffix() {
        assert_eq!(
            gh_http_status("gh: Not Found (HTTP 404)"),
            Some(reqwest::StatusCode::NOT_FOUND)
        );
        assert_eq!(gh_http_status("dial tcp: i/o timeout"), None);
    }

    #[tokio::test]
    async fn pulls_errors_map_to_distinct_api_error_codes() {
        use axum::routing::get;
        use axum::{Json, Router};

        for (status, want_code) in [
            (axum::http::StatusCode::UNAUTHORIZED, ErrorCode::Unauth),
            (axum::http::StatusCode::FORBIDDEN, ErrorCode::Sandbox),
            (
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::Conflict,
            ),
        ] {
            let routes = Router::new().route(
                "/repos/acme/site/pulls",
                get(
                    move || async move { (status, Json(serde_json::json!({ "message": "nope" }))) },
                ),
            );
            let (base, stop) = mock_api(routes).await;
            let dir = tmp_dir(&format!("err-{}", status.as_u16()));
            write_session(&dir, "tok");
            let auth = GithubAuth::open(
                GithubConfig {
                    api_url: base.clone(),
                    web_url: base,
                    gh_path: PathBuf::new(),
                    ..GithubConfig::default()
                },
                &dir,
            );
            let err = auth.find_pr("acme/site", "content/x").await.unwrap_err();
            assert_eq!(err.code, want_code, "status {status}");
            if want_code == ErrorCode::Sandbox {
                assert!(err.message.contains("push access"));
            }
            stop();
        }
    }

    fn review(login: &str, state: &str) -> ApiReview {
        ApiReview {
            html_url: None,
            user: Some(ApiLogin {
                login: login.into(),
                avatar_url: None,
            }),
            state: state.into(),
            commit_id: String::new(),
            body: None,
            submitted_at: None,
        }
    }

    fn pull(draft: bool, state: &str, merged: bool) -> PullFacts {
        PullFacts {
            draft,
            closed: state == "closed",
            merged,
        }
    }

    #[test]
    fn pr_state_maps_draft_open_merged_closed() {
        assert_eq!(pr_state(&pull(true, "open", false), &[]), PrState::Draft);
        assert_eq!(pr_state(&pull(false, "open", false), &[]), PrState::Open);
        assert_eq!(pr_state(&pull(false, "closed", true), &[]), PrState::Merged);
        assert_eq!(
            pr_state(&pull(false, "closed", false), &[]),
            PrState::Closed
        );
        // A merged/closed PR stays merged/closed whatever its reviews said.
        let approvals = [review("grace", "APPROVED")];
        assert_eq!(
            pr_state(&pull(false, "closed", true), &approvals),
            PrState::Merged
        );
        assert_eq!(
            pr_state(&pull(false, "closed", false), &approvals),
            PrState::Closed
        );
    }

    #[test]
    fn pr_state_is_approved_via_each_reviewers_latest_review() {
        let open = pull(false, "open", false);
        assert_eq!(
            pr_state(&open, &[review("grace", "APPROVED")]),
            PrState::Approved
        );
        // Comments don't override an approval.
        assert_eq!(
            pr_state(
                &open,
                &[review("grace", "APPROVED"), review("grace", "COMMENTED")]
            ),
            PrState::Approved
        );
        // A later change request from the same reviewer withdraws it and sends the change back.
        assert_eq!(
            pr_state(
                &open,
                &[
                    review("grace", "APPROVED"),
                    review("grace", "CHANGES_REQUESTED")
                ]
            ),
            PrState::ChangesRequested
        );
        // Any outstanding change request beats approval.
        assert_eq!(
            pr_state(
                &open,
                &[
                    review("grace", "APPROVED"),
                    review("alan", "CHANGES_REQUESTED")
                ]
            ),
            PrState::ChangesRequested
        );
        // Re-approval after a change request.
        assert_eq!(
            pr_state(
                &open,
                &[
                    review("alan", "CHANGES_REQUESTED"),
                    review("alan", "APPROVED")
                ]
            ),
            PrState::Approved
        );
        // A dismissed approval no longer counts.
        assert_eq!(
            pr_state(&open, &[review("grace", "DISMISSED")]),
            PrState::Open
        );
        // Drafts stay drafts.
        assert_eq!(
            pr_state(&pull(true, "open", false), &[review("grace", "APPROVED")]),
            PrState::Draft
        );
    }

    #[test]
    fn pr_state_changes_requested_beats_approval() {
        let open = pull(false, "open", false);
        assert_eq!(
            pr_state(&open, &[review("alan", "CHANGES_REQUESTED")]),
            PrState::ChangesRequested
        );
        assert_eq!(
            pr_state(
                &open,
                &[
                    review("grace", "APPROVED"),
                    review("alan", "CHANGES_REQUESTED")
                ]
            ),
            PrState::ChangesRequested
        );
        assert_eq!(
            serde_json::to_value(PrState::ChangesRequested).unwrap(),
            "changes_requested"
        );
    }

    #[test]
    fn sent_back_is_the_newest_outstanding_request() {
        let mut first = review("alan", "CHANGES_REQUESTED");
        first.body = Some("fix the title".into());
        let mut second = review("grace", "CHANGES_REQUESTED");
        second.body = Some("and the image".into());
        let mut resolved = review("alan", "CHANGES_REQUESTED");
        resolved.body = Some("old".into());
        let approve = review("alan", "APPROVED");
        assert_eq!(
            sent_back(&[first, second]),
            Some(SentBack {
                reviewer: "grace".into(),
                comment: "and the image".into()
            })
        );
        assert_eq!(sent_back(&[resolved, approve]), None);
    }

    /// `pulls/4` (head `h1`, or `head`) and `pulls/4/reviews` (GET: `reviews`; POST: recorded).
    fn review_routes(
        head: &'static str,
        reviews: serde_json::Value,
        posts: std::sync::Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    ) -> axum::Router {
        use axum::routing::get;
        use axum::{Json, Router};
        Router::new()
            .route(
                "/repos/acme/site/pulls/4",
                get(move || async move {
                    Json(
                        serde_json::json!({ "number": 4, "html_url": "u", "title": "t",
                        "state": "open", "head": { "sha": head } }),
                    )
                }),
            )
            .route(
                "/repos/acme/site/pulls",
                get(move || async move {
                    Json(
                        serde_json::json!([{ "number": 4, "html_url": "u", "title": "t",
                        "state": "open", "head": { "sha": head } }]),
                    )
                }),
            )
            .route(
                "/repos/acme/site/pulls/4/reviews",
                get(move || {
                    let reviews = reviews.clone();
                    async move { Json(reviews) }
                })
                .post(move |Json(b): Json<serde_json::Value>| {
                    let posts = posts.clone();
                    async move {
                        posts.lock().unwrap().push(b);
                        Json(serde_json::json!({ "id": 1 }))
                    }
                }),
            )
            .route(
                "/repos/acme/site/commits/{sha}/check-runs",
                get(|| async { Json(serde_json::json!({ "check_runs": [] })) }),
            )
            .route(
                "/repos/acme/site/commits/{sha}/status",
                get(|| async { Json(serde_json::json!({})) }),
            )
            .route(
                "/repos/acme/site/deployments",
                get(|| async { Json(serde_json::json!([])) }),
            )
    }

    #[tokio::test]
    async fn submit_review_pins_commit_id() {
        let posts = Default::default();
        let (base, stop) = mock_api(review_routes(
            "h1",
            serde_json::json!([]),
            std::sync::Arc::clone(&posts),
        ))
        .await;
        let auth = status_auth(base, "submit-review");
        auth.submit_review("acme/site", 4, "h1", Decision::SendBack, Some("fix it"))
            .await
            .unwrap();
        auth.submit_review("acme/site", 4, "h1", Decision::Greenlight, None)
            .await
            .unwrap();
        let sent = posts.lock().unwrap().clone();
        assert_eq!(
            sent,
            vec![
                serde_json::json!({ "event": "REQUEST_CHANGES", "body": "fix it", "commit_id": "h1" }),
                serde_json::json!({ "event": "APPROVE", "body": "", "commit_id": "h1" }),
            ]
        );
        stop();
    }

    #[tokio::test]
    async fn submit_review_refuses_moved_head() {
        let posts = Default::default();
        let (base, stop) = mock_api(review_routes(
            "h2",
            serde_json::json!([]),
            std::sync::Arc::clone(&posts),
        ))
        .await;
        let auth = status_auth(base, "submit-moved");
        let err = auth
            .submit_review("acme/site", 4, "h1", Decision::Greenlight, None)
            .await
            .unwrap_err();
        assert_eq!(err.message, "This change was updated since you opened it");
        assert_eq!(err.reason, Some("head_moved"));
        assert_eq!(err.http_status(), 409);
        assert!(posts.lock().unwrap().is_empty());
        stop();
    }

    #[tokio::test]
    async fn pr_status_carries_send_back_comment() {
        let reviews = serde_json::json!([
            { "user": { "login": "alan" }, "state": "CHANGES_REQUESTED",
              "commit_id": "h1", "body": "Please fix the headline" }
        ]);
        let (base, stop) = mock_api(review_routes("h1", reviews, Default::default())).await;
        let auth = status_auth(base, "sent-back-status");
        let st = auth
            .pr_status("acme/site", "content/x")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(st.state, PrState::ChangesRequested);
        assert_eq!(
            st.sent_back,
            Some(SentBack {
                reviewer: "alan".into(),
                comment: "Please fix the headline".into()
            })
        );
        let json = serde_json::to_value(&st).unwrap();
        assert_eq!(json["sentBack"]["reviewer"], "alan");
        assert_eq!(json["state"], "changes_requested");
        stop();
    }

    fn run(status: &str, conclusion: Option<&str>) -> ApiCheckRun {
        named("test", status, conclusion)
    }

    fn named(name: &str, status: &str, conclusion: Option<&str>) -> ApiCheckRun {
        ApiCheckRun {
            name: name.into(),
            status: status.into(),
            conclusion: conclusion.map(Into::into),
            html_url: None,
        }
    }

    #[test]
    fn checks_aggregate_failing_over_pending_over_passing() {
        assert_eq!(aggregate_checks(&[]), ChecksState::None);
        assert_eq!(
            aggregate_checks(&[
                run("completed", Some("success")),
                run("completed", Some("skipped"))
            ]),
            ChecksState::Passing
        );
        assert_eq!(
            aggregate_checks(&[run("completed", Some("success")), run("in_progress", None)]),
            ChecksState::Pending
        );
        assert_eq!(
            aggregate_checks(&[run("queued", None)]),
            ChecksState::Pending
        );
        for bad in ["failure", "timed_out", "cancelled", "action_required"] {
            assert_eq!(
                aggregate_checks(&[run("in_progress", None), run("completed", Some(bad))]),
                ChecksState::Failing,
                "{bad}"
            );
        }
    }

    #[test]
    fn deploy_is_live_only_when_its_check_succeeds() {
        assert_eq!(
            deploy_state_of(&[
                named("deploy", "completed", Some("success")),
                named("test", "completed", Some("failure")),
            ]),
            DeployState::Live,
            "an unrelated failing check is not a failed deploy"
        );
        assert_eq!(
            deploy_state_of(&[named(
                "pages build and deployment",
                "completed",
                Some("success")
            )]),
            DeployState::Live
        );
    }

    #[test]
    fn deploy_is_building_while_its_check_runs() {
        for status in ["queued", "in_progress", "waiting", "requested"] {
            assert_eq!(
                deploy_state_of(&[named("deploy", status, None)]),
                DeployState::Building,
                "{status}"
            );
        }
    }

    #[test]
    fn deploy_fails_when_its_check_fails_even_beside_a_running_one() {
        for bad in ["failure", "timed_out", "cancelled", "action_required"] {
            assert_eq!(
                deploy_state_of(&[
                    named("deploy", "completed", Some(bad)),
                    named("deploy preview", "in_progress", None),
                ]),
                DeployState::Failed,
                "{bad}"
            );
        }
    }

    #[test]
    fn deploy_is_unknown_without_a_deploy_check() {
        assert_eq!(deploy_state_of(&[]), DeployState::Unknown);
        assert_eq!(
            deploy_state_of(&[named("test", "completed", Some("success"))]),
            DeployState::Unknown
        );
    }

    /// A mock whose `/repos/acme/site/pulls/1` answers with an ETag (and 304 when it
    /// matches), counting every request and the `If-None-Match` headers it saw.
    async fn cache_mock(
        delay: Duration,
    ) -> (
        String,
        impl FnOnce(),
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    ) {
        use axum::http::{HeaderMap, StatusCode};
        use axum::response::IntoResponse;
        use axum::routing::{get, post};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let (h, sn) = (hits.clone(), seen.clone());
        let routes = axum::Router::new()
            .route(
                "/repos/acme/site/pulls/1",
                get(move |headers: HeaderMap| {
                    let (h, sn) = (h.clone(), sn.clone());
                    async move {
                        h.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(delay).await;
                        if let Some(v) = headers.get("if-none-match") {
                            sn.lock().unwrap().push(v.to_str().unwrap().to_string());
                            if v == "\"v1\"" {
                                return StatusCode::NOT_MODIFIED.into_response();
                            }
                        }
                        (
                            [("etag", "\"v1\"")],
                            axum::Json(serde_json::json!({ "n": 1 })),
                        )
                            .into_response()
                    }
                }),
            )
            .route(
                "/repos/acme/site/issues/1/comments",
                post(|| async {
                    (
                        StatusCode::CREATED,
                        axum::Json(serde_json::json!({ "id": 5, "user": { "login": "ada" },
                            "created_at": "2026-01-01T00:00:00Z", "body": "hi",
                            "html_url": "u" })),
                    )
                }),
            );
        let (base, stop) = mock_api(routes).await;
        (base, stop, hits, seen)
    }

    #[tokio::test]
    async fn get_cache_reuses_a_fresh_response_without_asking() {
        let (base, stop, hits, _) = cache_mock(Duration::ZERO).await;
        let auth = status_auth(base, "cache-fresh");
        for _ in 0..2 {
            let v: serde_json::Value = auth.get_json("/repos/acme/site/pulls/1").await.unwrap();
            assert_eq!(v["n"], 1);
        }
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        stop();
    }

    #[tokio::test]
    async fn get_cache_revalidates_a_stale_response_and_reuses_it_on_304() {
        let (base, stop, hits, seen) = cache_mock(Duration::ZERO).await;
        let auth = status_auth(base, "cache-stale");
        auth.get_cache.set_ttl(Duration::ZERO);
        let first: serde_json::Value = auth.get_json("/repos/acme/site/pulls/1").await.unwrap();
        let second: serde_json::Value = auth.get_json("/repos/acme/site/pulls/1").await.unwrap();
        assert_eq!(first, second);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(*seen.lock().unwrap(), vec!["\"v1\"".to_string()]);
        stop();
    }

    #[tokio::test]
    async fn get_cache_shares_one_request_between_concurrent_calls() {
        let (base, stop, hits, _) = cache_mock(Duration::from_millis(150)).await;
        let auth = status_auth(base, "cache-dedupe");
        let path = "/repos/acme/site/pulls/1";
        let (a, b) = tokio::join!(
            auth.get_json::<serde_json::Value>(path),
            auth.get_json::<serde_json::Value>(path)
        );
        assert_eq!(a.unwrap(), b.unwrap());
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        stop();
    }

    #[tokio::test]
    async fn get_cache_forgets_a_repository_after_a_write_to_it() {
        let (base, stop, hits, seen) = cache_mock(Duration::ZERO).await;
        let auth = status_auth(base, "cache-write");
        let path = "/repos/acme/site/pulls/1";
        auth.get_json::<serde_json::Value>(path).await.unwrap();
        auth.add_comment("acme/site", 1, "hi").await.unwrap();
        auth.get_json::<serde_json::Value>(path).await.unwrap();
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
        // Dropped, not merely stale: the re-read is unconditional.
        assert!(seen.lock().unwrap().is_empty());
        stop();
    }

    #[tokio::test]
    async fn get_cache_never_shares_responses_between_tokens() {
        let (base, stop, hits, seen) = cache_mock(Duration::ZERO).await;
        let auth = status_auth(base, "cache-tokens");
        let path = "/repos/acme/site/pulls/1";
        auth.cached_get("token-a", path).await.unwrap();
        auth.cached_get("token-b", path).await.unwrap();
        auth.cached_get("token-a", path).await.unwrap();
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert!(seen.lock().unwrap().is_empty());
        stop();
    }

    /// A Git Data API mock for `acme/site`: `tip` is the assets branch's commit (None: no
    /// branch), `refuse` how many ref updates to refuse with 422 before accepting. Records every
    /// write as (method, path, body).
    async fn git_data_mock(
        tip: Option<&'static str>,
        refuse: usize,
    ) -> (
        String,
        impl FnOnce(),
        std::sync::Arc<std::sync::Mutex<Vec<(String, String, serde_json::Value)>>>,
    ) {
        use axum::http::{Method, StatusCode, Uri};
        use axum::response::IntoResponse;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Mutex};
        type Log = Arc<Mutex<Vec<(String, String, serde_json::Value)>>>;
        let log: Log = Arc::default();
        let refused = Arc::new(AtomicUsize::new(0));
        // After a refused update the branch has moved on.
        let tip_now = Arc::new(Mutex::new(tip.map(str::to_string)));
        let (l, rf, t) = (log.clone(), refused.clone(), tip_now.clone());
        let handler = move |method: Method,
                            uri: Uri,
                            body: Option<axum::Json<serde_json::Value>>| {
            let (log, refused, tip) = (l.clone(), rf.clone(), t.clone());
            async move {
                let path = uri.path().to_string();
                let body = body.map(|b| b.0).unwrap_or(serde_json::Value::Null);
                if method != Method::GET {
                    log.lock()
                        .unwrap()
                        .push((method.to_string(), path.clone(), body.clone()));
                }
                let ok = |v: serde_json::Value| (StatusCode::OK, axum::Json(v)).into_response();
                match (method.as_str(), path.as_str()) {
                    ("GET", "/repos/acme/site/git/ref/heads/orso-review-assets") => {
                        match tip.lock().unwrap().clone() {
                            Some(sha) => ok(serde_json::json!({ "object": { "sha": sha } })),
                            None => StatusCode::NOT_FOUND.into_response(),
                        }
                    }
                    ("GET", p) if p.starts_with("/repos/acme/site/git/commits/") => {
                        let sha = p.rsplit('/').next().unwrap();
                        ok(serde_json::json!({ "tree": { "sha": format!("tree-of-{sha}") } }))
                    }
                    ("POST", "/repos/acme/site/git/blobs") if refuse == usize::MAX => {
                        // No push access.
                        StatusCode::FORBIDDEN.into_response()
                    }
                    ("POST", "/repos/acme/site/git/blobs") => {
                        ok(serde_json::json!({ "sha": "blob1" }))
                    }
                    ("POST", "/repos/acme/site/git/trees") => {
                        ok(serde_json::json!({ "sha": "tree1" }))
                    }
                    ("POST", "/repos/acme/site/git/commits") => {
                        ok(serde_json::json!({ "sha": "commit1" }))
                    }
                    ("POST", "/repos/acme/site/git/refs")
                    | ("PATCH", "/repos/acme/site/git/refs/heads/orso-review-assets") => {
                        if refuse == usize::MAX - 1 {
                            // A ruleset: branch creation is restricted.
                            let msg = serde_json::json!({ "message": "Repository rule violations found" });
                            return (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(msg))
                                .into_response();
                        }
                        if refused.fetch_add(1, Ordering::SeqCst) < refuse {
                            *tip.lock().unwrap() = Some("moved".into());
                            let msg =
                                serde_json::json!({ "message": "Update is not a fast forward" });
                            return (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(msg))
                                .into_response();
                        }
                        ok(serde_json::json!({ "object": { "sha": "commit1" } }))
                    }
                    _ => StatusCode::NOT_FOUND.into_response(),
                }
            }
        };
        let routes = axum::Router::new().fallback(handler);
        let (base, stop) = mock_api(routes).await;
        (base, stop, log)
    }

    fn writes(log: &std::sync::Mutex<Vec<(String, String, serde_json::Value)>>) -> Vec<String> {
        log.lock()
            .unwrap()
            .iter()
            .map(|(m, p, _)| format!("{m} {}", p.trim_start_matches("/repos/acme/site/git/")))
            .collect()
    }

    #[tokio::test]
    async fn commit_files_creates_the_assets_branch_as_an_orphan() {
        let (base, stop, log) = git_data_mock(None, 0).await;
        let auth = status_auth(base, "assets-orphan");
        let files = vec![("shots/3/2026-11-19/a.png".to_string(), b"png".to_vec())];
        let sha = auth
            .commit_files_to_branch(
                "acme/site",
                "orso-review-assets",
                "Add review images",
                &files,
            )
            .await
            .unwrap();
        assert_eq!(sha, "commit1");
        assert_eq!(
            writes(&log),
            ["POST blobs", "POST trees", "POST commits", "POST refs"]
        );
        let log = log.lock().unwrap();
        assert_eq!(log[0].2["content"], "cG5n");
        assert_eq!(log[0].2["encoding"], "base64");
        // No base tree and no parent: the branch shares nothing with the site's history.
        assert!(log[1].2.get("base_tree").is_none());
        assert_eq!(log[1].2["tree"][0]["path"], "shots/3/2026-11-19/a.png");
        assert_eq!(log[2].2["parents"], serde_json::json!([]));
        assert_eq!(log[3].2["ref"], "refs/heads/orso-review-assets");
        stop();
    }

    #[tokio::test]
    async fn commit_files_appends_to_an_existing_assets_branch() {
        let (base, stop, log) = git_data_mock(Some("tip0"), 0).await;
        let auth = status_auth(base, "assets-append");
        let files = vec![("shots/3/2026-11-19/b.png".to_string(), b"png".to_vec())];
        auth.commit_files_to_branch("acme/site", "orso-review-assets", "m", &files)
            .await
            .unwrap();
        assert_eq!(
            writes(&log),
            [
                "POST blobs",
                "POST trees",
                "POST commits",
                "PATCH refs/heads/orso-review-assets"
            ]
        );
        let log = log.lock().unwrap();
        assert_eq!(log[1].2["base_tree"], "tree-of-tip0");
        assert_eq!(log[2].2["parents"], serde_json::json!(["tip0"]));
        assert_eq!(log[3].2["force"], false);
        stop();
    }

    #[tokio::test]
    async fn commit_files_retries_once_when_the_branch_moved() {
        let (base, stop, log) = git_data_mock(Some("tip0"), 1).await;
        let auth = status_auth(base, "assets-retry");
        let files = vec![("shots/3/2026-11-19/c.png".to_string(), b"png".to_vec())];
        auth.commit_files_to_branch("acme/site", "orso-review-assets", "m", &files)
            .await
            .unwrap();
        // The blob is uploaded once; tree, commit and ref are redone on the moved tip.
        assert_eq!(
            writes(&log),
            [
                "POST blobs",
                "POST trees",
                "POST commits",
                "PATCH refs/heads/orso-review-assets",
                "POST trees",
                "POST commits",
                "PATCH refs/heads/orso-review-assets",
            ]
        );
        let log = log.lock().unwrap();
        assert_eq!(log[5].2["parents"], serde_json::json!(["moved"]));
        stop();
    }

    #[tokio::test]
    async fn commit_files_gives_up_after_a_second_refusal() {
        let (base, stop, log) = git_data_mock(Some("tip0"), 2).await;
        let auth = status_auth(base, "assets-refused");
        let files = vec![("shots/3/x.png".to_string(), b"png".to_vec())];
        let err = auth
            .commit_files_to_branch("acme/site", "orso-review-assets", "m", &files)
            .await
            .unwrap_err();
        assert_eq!(err.code.status(), 409, "{err:?}");
        assert_eq!(
            writes(&log)
                .iter()
                .filter(|w| w.starts_with("PATCH"))
                .count(),
            2
        );
        stop();
    }

    #[tokio::test]
    async fn commit_files_reports_a_refused_branch_without_retrying() {
        // A ruleset refuses creating the branch: no second attempt.
        let (base, stop, log) = git_data_mock(None, usize::MAX - 1).await;
        let auth = status_auth(base, "assets-rules");
        let files = vec![("uploads/3/x.png".to_string(), b"png".to_vec())];
        let err = auth
            .commit_files_to_branch("acme/site", "orso-review-assets", "m", &files)
            .await
            .unwrap_err();
        assert_eq!(err.reason, Some("assets_refused"), "{err:?}");
        assert_eq!(err.message, ASSETS_REFUSED);
        assert_eq!(
            writes(&log)
                .iter()
                .filter(|w| w.starts_with("POST refs"))
                .count(),
            1
        );
        stop();

        // No push access: refused at the first blob.
        let (base, stop, log) = git_data_mock(None, usize::MAX).await;
        let auth = status_auth(base, "assets-403");
        let err = auth
            .commit_files_to_branch("acme/site", "orso-review-assets", "m", &files)
            .await
            .unwrap_err();
        assert_eq!(err.reason, Some("assets_refused"));
        assert_eq!(err.code.status(), 403);
        assert_eq!(writes(&log), ["POST blobs"]);
        stop();
    }

    /// An assets branch at `tip0` holding `files`, recording writes like `git_data_mock`.
    async fn assets_tree_mock(
        files: &'static [&'static str],
    ) -> (
        String,
        impl FnOnce(),
        std::sync::Arc<std::sync::Mutex<Vec<(String, String, serde_json::Value)>>>,
    ) {
        use axum::http::{Method, StatusCode, Uri};
        use axum::response::IntoResponse;
        use std::sync::{Arc, Mutex};
        let log: Arc<Mutex<Vec<(String, String, serde_json::Value)>>> = Arc::default();
        let l = log.clone();
        let handler =
            move |method: Method, uri: Uri, body: Option<axum::Json<serde_json::Value>>| {
                let log = l.clone();
                async move {
                    let path = uri.path().to_string();
                    if method != Method::GET {
                        let b = body.map(|b| b.0).unwrap_or(serde_json::Value::Null);
                        log.lock()
                            .unwrap()
                            .push((method.to_string(), path.clone(), b));
                    }
                    let ok = |v: serde_json::Value| axum::Json(v).into_response();
                    match (method.as_str(), path.as_str()) {
                        ("GET", "/repos/acme/site/git/ref/heads/orso-review-assets") => {
                            ok(serde_json::json!({ "object": { "sha": "tip0" } }))
                        }
                        ("GET", "/repos/acme/site/git/commits/tip0") => {
                            ok(serde_json::json!({ "tree": { "sha": "tree0" } }))
                        }
                        ("GET", "/repos/acme/site/git/trees/tree0") => {
                            let mut entries =
                                vec![serde_json::json!({ "path": "shots", "type": "tree" })];
                            entries.extend(
                                files
                                    .iter()
                                    .map(|p| serde_json::json!({ "path": p, "type": "blob" })),
                            );
                            ok(serde_json::json!({ "tree": entries }))
                        }
                        ("POST", _) => ok(serde_json::json!({ "sha": "new1" })),
                        ("PATCH", _) => ok(serde_json::json!({})),
                        ("DELETE", _) => StatusCode::NO_CONTENT.into_response(),
                        _ => StatusCode::NOT_FOUND.into_response(),
                    }
                }
            };
        let (base, stop) = mock_api(axum::Router::new().fallback(handler)).await;
        (base, stop, log)
    }

    const ASSET_FILES: &[&str] = &[
        "shots/3/2026-11-19/a.png",
        "uploads/3/2026-11-19/b.png",
        "shots/5/2026-11-19/c.png",
        "README.md",
    ];

    #[tokio::test]
    async fn cleanup_removes_closed_changes_images_in_one_commit() {
        let (base, stop, log) = assets_tree_mock(ASSET_FILES).await;
        let auth = status_auth(base, "assets-clean");
        let done = auth
            .clean_review_assets("acme/site", "orso-review-assets", |pr| pr == 5)
            .await
            .unwrap();
        assert_eq!(done, AssetsCleanup::Removed(vec![3]));
        let log = log.lock().unwrap();
        let ops: Vec<_> = log.iter().map(|(m, p, _)| format!("{m} {p}")).collect();
        assert_eq!(
            ops,
            [
                "POST /repos/acme/site/git/trees",
                "POST /repos/acme/site/git/commits",
                "PATCH /repos/acme/site/git/refs/heads/orso-review-assets",
            ]
        );
        assert_eq!(log[0].2["base_tree"], "tree0");
        let removed: Vec<_> = log[0].2["tree"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e["path"].as_str().unwrap().to_string(), e["sha"].is_null()))
            .collect();
        assert_eq!(
            removed,
            [
                ("shots/3/2026-11-19/a.png".to_string(), true),
                ("uploads/3/2026-11-19/b.png".to_string(), true),
            ]
        );
        assert_eq!(log[1].2["parents"], serde_json::json!(["tip0"]));
        assert_eq!(log[2].2["force"], false);
        stop();
    }

    #[tokio::test]
    async fn cleanup_deletes_the_branch_when_no_open_change_has_images() {
        let (base, stop, log) = assets_tree_mock(ASSET_FILES).await;
        let auth = status_auth(base, "assets-drop");
        let done = auth
            .clean_review_assets("acme/site", "orso-review-assets", |_| false)
            .await
            .unwrap();
        assert_eq!(done, AssetsCleanup::DeletedBranch);
        let ops: Vec<_> = log
            .lock()
            .unwrap()
            .iter()
            .map(|(m, p, _)| format!("{m} {p}"))
            .collect();
        assert_eq!(
            ops,
            ["DELETE /repos/acme/site/git/refs/heads/orso-review-assets"]
        );
        stop();
    }

    #[tokio::test]
    async fn cleanup_leaves_open_changes_alone() {
        let (base, stop, log) = assets_tree_mock(ASSET_FILES).await;
        let auth = status_auth(base, "assets-keep");
        let done = auth
            .clean_review_assets("acme/site", "orso-review-assets", |pr| pr == 3 || pr == 5)
            .await
            .unwrap();
        assert_eq!(done, AssetsCleanup::Nothing);
        assert!(log.lock().unwrap().is_empty());
        stop();
    }

    #[tokio::test]
    async fn head_check_reads_past_the_get_cache() {
        use axum::routing::{get, post};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let reads = std::sync::Arc::new(AtomicUsize::new(0));
        let posts = std::sync::Arc::new(AtomicUsize::new(0));
        let (r, p) = (reads.clone(), posts.clone());
        let routes = axum::Router::new()
            .route(
                "/repos/acme/site/pulls/3",
                get(move || {
                    let r = r.clone();
                    async move {
                        // The creator pushes right after the first read.
                        let head = if r.fetch_add(1, Ordering::SeqCst) == 0 {
                            "h1"
                        } else {
                            "h2"
                        };
                        axum::Json(
                            serde_json::json!({ "number": 3, "html_url": "u", "title": "t",
                            "head": { "sha": head } }),
                        )
                    }
                }),
            )
            .route(
                "/repos/acme/site/pulls/3/reviews",
                post(move || {
                    let p = p.clone();
                    async move {
                        p.fetch_add(1, Ordering::SeqCst);
                        axum::Json(serde_json::json!({}))
                    }
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "head-fresh");
        // A cached read still says h1 for the next 15 s.
        let cached: ApiPull = auth.get_json("/repos/acme/site/pulls/3").await.unwrap();
        assert_eq!(cached.head.unwrap().sha, "h1");
        let again: ApiPull = auth.get_json("/repos/acme/site/pulls/3").await.unwrap();
        assert_eq!(again.head.unwrap().sha, "h1");
        assert_eq!(reads.load(Ordering::SeqCst), 1);

        let err = auth
            .submit_review("acme/site", 3, "h1", Decision::Greenlight, None)
            .await
            .unwrap_err();
        assert_eq!(err.code.status(), 409, "{err:?}");
        assert_eq!(err.reason, Some("head_moved"));
        assert_eq!(
            reads.load(Ordering::SeqCst),
            2,
            "the head check asked GitHub"
        );
        assert_eq!(posts.load(Ordering::SeqCst), 0, "nothing was submitted");
        stop();
    }

    #[tokio::test]
    async fn get_cache_keeps_no_failed_response() {
        use axum::http::StatusCode;
        use axum::routing::get;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let routes = axum::Router::new().route(
            "/repos/acme/site/pulls/2",
            get(move || {
                let h = h.clone();
                async move {
                    h.fetch_add(1, Ordering::SeqCst);
                    StatusCode::BAD_GATEWAY
                }
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "cache-error");
        for _ in 0..2 {
            assert!(auth
                .get_json::<serde_json::Value>("/repos/acme/site/pulls/2")
                .await
                .is_err());
        }
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        stop();
    }

    fn status_auth(base: String, tag: &str) -> GithubAuth {
        let dir = tmp_dir(tag);
        write_session(&dir, "tok");
        GithubAuth::open(
            GithubConfig {
                api_url: base.clone(),
                web_url: base,
                gh_path: PathBuf::new(),
                ..GithubConfig::default()
            },
            &dir,
        )
    }

    fn review_on(user: &str, state: &str, commit: &str) -> ApiReview {
        ApiReview {
            html_url: None,
            user: Some(ApiLogin {
                login: user.to_string(),
                avatar_url: None,
            }),
            state: state.to_string(),
            commit_id: commit.to_string(),
            body: None,
            submitted_at: None,
        }
    }

    #[test]
    fn my_review_state_uses_latest_review_on_current_head() {
        let rs = vec![
            review_on("ada", "APPROVED", "aaa"),
            review_on("ada", "CHANGES_REQUESTED", "bbb"),
        ];
        assert_eq!(my_review_state(&rs, "ada", "bbb"), MyReviewState::SentBack);
        assert_eq!(my_review_state(&rs, "ada", "aaa"), MyReviewState::Greenlit);
        assert_eq!(my_review_state(&rs, "ada", "ccc"), MyReviewState::New);
        let others = vec![review_on("bob", "APPROVED", "bbb")];
        assert_eq!(my_review_state(&others, "ada", "bbb"), MyReviewState::New);
        assert_eq!(my_review_state(&rs, "Ada", "bbb"), MyReviewState::SentBack);
        let commented = vec![
            review_on("ada", "APPROVED", "bbb"),
            review_on("ada", "COMMENTED", "bbb"),
        ];
        assert_eq!(
            my_review_state(&commented, "ada", "bbb"),
            MyReviewState::Greenlit
        );
        assert_eq!(
            serde_json::to_value(MyReviewState::SentBack).unwrap(),
            "sent_back"
        );
    }

    fn submitted(user: &str, state: &str, at: Option<&str>, body: &str) -> ApiReview {
        ApiReview {
            html_url: None,
            user: Some(ApiLogin {
                login: user.into(),
                avatar_url: None,
            }),
            state: state.into(),
            commit_id: "h".into(),
            body: Some(body.into()),
            submitted_at: at.map(String::from),
        }
    }

    fn commit(sha: &str, message: &str, date: &str) -> ApiPullCommit {
        ApiPullCommit {
            sha: sha.into(),
            commit: ApiCommitInfo {
                message: message.into(),
                author: Some(ApiCommitPerson {
                    name: "Bob B".into(),
                    date: date.into(),
                }),
                committer: Some(ApiCommitPerson {
                    name: "Bob B".into(),
                    date: date.into(),
                }),
            },
            author: Some(ApiLogin {
                login: "bob".into(),
                avatar_url: None,
            }),
        }
    }

    #[test]
    fn timeline_is_oldest_first_and_groups_consecutive_commits() {
        let comments = vec![ApiIssueComment {
            id: 9,
            user: Some(ApiLogin {
                login: "ada".into(),
                avatar_url: None,
            }),
            created_at: "2026-01-02T00:00:00Z".into(),
            body: Some("Looks close".into()),
            html_url: "https://gh/c/9".into(),
        }];
        let reviews = vec![
            submitted("ada", "COMMENTED", Some("2026-01-02T01:00:00Z"), ""),
            submitted("ada", "PENDING", None, "draft"),
            submitted(
                "ada",
                "CHANGES_REQUESTED",
                Some("2026-01-03T00:00:00Z"),
                "Fix title",
            ),
            submitted("cy", "COMMENTED", Some("2026-01-03T01:00:00Z"), "nit"),
            submitted("ada", "APPROVED", Some("2026-01-05T00:00:00Z"), ""),
        ];
        let commits = vec![
            commit("aaaaaaaaaa", "First\n\nbody", "2026-01-01T00:00:00Z"),
            commit("bbbbbbbbbb", "Second", "2026-01-01T01:00:00Z"),
            commit("cccccccccc", "Fix the title", "2026-01-04T00:00:00Z"),
        ];
        let items = timeline(
            comments,
            &reviews,
            commits,
            Some(("2026-01-06T00:00:00Z".into(), true)),
        );
        let kinds: Vec<String> = items
            .iter()
            .map(|i| {
                serde_json::to_value(i).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(
            kinds,
            ["push", "comment", "review", "review", "push", "review", "state"]
        );
        // A group is dated by its latest commit; its commits keep their own order.
        let TimelineItem::Push { commits, at } = &items[0] else {
            panic!("{:?}", items[0])
        };
        assert_eq!(at, "2026-01-01T01:00:00Z");
        assert_eq!(
            commits,
            &vec![
                PushedCommit {
                    sha: "aaaaaaa".into(),
                    message: "First".into(),
                    author: "bob".into()
                },
                PushedCommit {
                    sha: "bbbbbbb".into(),
                    message: "Second".into(),
                    author: "bob".into()
                },
            ]
        );
        assert_eq!(
            items[2],
            TimelineItem::Review {
                author: "ada".into(),
                avatar_url: None,
                at: "2026-01-03T00:00:00Z".into(),
                verdict: ReviewVerdict::SentBack,
                body: Some("Fix title".into()),
                url: None,
            }
        );
        assert!(
            matches!(&items[3], TimelineItem::Review { verdict: ReviewVerdict::Commented, author, .. } if author == "cy")
        );
        assert!(matches!(
            &items[5],
            TimelineItem::Review {
                verdict: ReviewVerdict::Greenlit,
                body: None,
                ..
            }
        ));
        assert_eq!(
            items[6],
            TimelineItem::State {
                at: "2026-01-06T00:00:00Z".into(),
                state: "merged".into()
            }
        );
        let json = serde_json::to_value(&items[2]).unwrap();
        assert_eq!(json["verdict"], "sent_back");
    }

    #[test]
    fn reviewers_keep_their_latest_decision_and_list_requested_ones() {
        let reviews = vec![
            submitted("ada", "CHANGES_REQUESTED", Some("1"), "x"),
            submitted("ada", "COMMENTED", Some("2"), "but"),
            submitted("cy", "COMMENTED", Some("2"), "q"),
            submitted("bo", "APPROVED", Some("1"), ""),
            submitted("bo", "DISMISSED", Some("3"), ""),
            submitted("ed", "APPROVED", None, ""),
        ];
        let requested = vec![
            ApiLogin {
                login: "dee".into(),
                avatar_url: None,
            },
            ApiLogin {
                login: "CY".into(),
                avatar_url: None,
            },
        ];
        let got = reviewers(&reviews, &requested);
        let pairs: Vec<(&str, ReviewerState)> =
            got.iter().map(|r| (r.login.as_str(), r.state)).collect();
        assert_eq!(
            pairs,
            vec![
                ("ada", ReviewerState::SentBack),
                ("cy", ReviewerState::Commented),
                ("bo", ReviewerState::Dismissed),
                ("dee", ReviewerState::Requested),
            ]
        );
    }

    #[tokio::test]
    async fn closed_reviews_say_there_may_be_more_after_a_full_page() {
        use axum::routing::get;
        use axum::{Json, Router};
        let routes = Router::new()
            .route(
                "/repos/acme/site/pulls",
                get(|| async {
                    let pulls: Vec<_> = (1..=CLOSED_PAGE as u64)
                        .map(|n| serde_json::json!({ "number": n, "html_url": "u", "title": "t",
                            "state": "closed", "labels": [{ "name": if n == 1 { "orso" } else { "x" } }],
                            "user": { "login": "bob" }, "head": { "sha": "" } }))
                        .collect();
                    Json(serde_json::Value::Array(pulls))
                }),
            )
            .route(
                "/repos/acme/site/pulls/1/reviews",
                get(|| async { Json(serde_json::json!([])) }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "closedmore");
        let closed = auth
            .closed_reviews("acme/site", &["orso".to_string()], "ada")
            .await
            .unwrap();
        stop();
        assert!(closed.more);
        assert_eq!(closed.reviews.len(), 1);
        assert_eq!(closed.reviews[0].state, "closed");
        // No issues listing here: counts fall back to 0.
        assert_eq!(closed.reviews[0].comments, 0);
    }

    #[tokio::test]
    async fn conversation_reads_comments_reviews_commits_and_checks() {
        use axum::routing::get;
        use axum::{Json, Router};
        let routes = Router::new()
            .route(
                "/repos/acme/site/pulls/3",
                get(|| async {
                    Json(
                        serde_json::json!({ "number": 3, "html_url": "u", "title": "t",
                        "state": "open", "head": { "sha": "h3" },
                        "requested_reviewers": [{ "login": "dee" }] }),
                    )
                }),
            )
            .route(
                "/repos/acme/site/issues/3/comments",
                get(|| async {
                    Json(serde_json::json!([{ "id": 1, "user": { "login": "ada",
                        "avatar_url": "https://avatars.example/ada" },
                        "created_at": "2026-01-02T00:00:00Z", "body": "hi", "html_url": "c1" }]))
                }),
            )
            .route(
                "/repos/acme/site/pulls/3/reviews",
                get(|| async {
                    Json(serde_json::json!([{ "user": { "login": "bo",
                        "avatar_url": "javascript:alert(1)" }, "state": "APPROVED",
                        "commit_id": "h3", "submitted_at": "2026-01-03T00:00:00Z",
                        "html_url": "https://gh/pull/3#review-1" }]))
                }),
            )
            .route(
                "/repos/acme/site/pulls/3/commits",
                get(|| async {
                    Json(
                        serde_json::json!([{ "sha": "abcdef123", "commit": { "message": "Edit",
                        "committer": { "name": "b", "date": "2026-01-01T00:00:00Z" } } }]),
                    )
                }),
            )
            .route(
                "/repos/acme/site/commits/h3/check-runs",
                get(|| async {
                    Json(serde_json::json!({ "check_runs": [{ "name": "build",
                        "status": "completed", "conclusion": "failure",
                        "html_url": "https://gh/runs/1" }] }))
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "conversation");
        let c = auth.conversation("acme/site", 3).await.unwrap();
        stop();
        let json = serde_json::to_value(&c).unwrap();
        // Oldest first: the push came before the comment.
        assert_eq!(json["items"][0]["kind"], "push");
        assert_eq!(json["items"][0]["commits"][0]["sha"], "abcdef1");
        assert_eq!(json["items"][0]["commits"][0]["author"], "b");
        assert_eq!(json["items"][1]["kind"], "comment");
        assert_eq!(json["items"][1]["body"], "hi");
        assert_eq!(json["items"][1]["avatarUrl"], "https://avatars.example/ada");
        // A review links to itself on GitHub; an avatar that isn't http(s) is dropped.
        assert_eq!(json["items"][2]["kind"], "review");
        assert_eq!(json["items"][2]["url"], "https://gh/pull/3#review-1");
        assert!(json["items"][2].get("avatarUrl").is_none());
        assert_eq!(
            json["checks"],
            serde_json::json!([{ "name": "build", "status": "completed",
                "conclusion": "failure", "url": "https://gh/runs/1" }])
        );
        assert_eq!(
            json["reviewers"],
            serde_json::json!([{ "login": "bo", "state": "greenlit" },
                { "login": "dee", "state": "requested" }])
        );
    }

    #[tokio::test]
    async fn review_inbox_filters_label_and_draft_and_marks_mine() {
        use axum::routing::get;
        use axum::{Json, Router};

        let routes = Router::new()
            .route(
                "/repos/acme/site/pulls",
                get(|| async {
                    Json(serde_json::json!([
                        { "number": 1, "html_url": "u1", "title": "keep", "draft": false,
                          "labels": [{ "name": "Orso" }], "user": { "login": "bob" },
                          "updated_at": "2026-01-01T00:00:00Z",
                          "head": { "sha": "h1" } },
                        { "number": 2, "html_url": "u2", "title": "draft", "draft": true,
                          "labels": [{ "name": "orso" }], "user": { "login": "bob" } },
                        { "number": 3, "html_url": "u3", "title": "nolabel", "draft": false,
                          "labels": [], "user": { "login": "bob" } },
                        { "number": 4, "html_url": "u4", "title": "mine", "draft": false,
                          "labels": [{ "name": "orso" }], "user": { "login": "ADA" } },
                        { "number": 5, "html_url": "u5", "title": "ghost", "draft": false,
                          "labels": [{ "name": "orso" }], "user": null }
                    ]))
                }),
            )
            .route(
                "/repos/acme/site/pulls/1/reviews",
                get(|| async {
                    Json(serde_json::json!([
                        { "user": { "login": "ada" }, "state": "APPROVED", "commit_id": "h1" }
                    ]))
                }),
            )
            .route(
                "/repos/acme/site/pulls/4/reviews",
                get(|| async { Json(serde_json::json!([])) }),
            )
            .route(
                "/repos/acme/site/pulls/5/reviews",
                get(|| async { Json(serde_json::json!([])) }),
            )
            .route(
                "/repos/acme/site/commits/h1/check-runs",
                get(|| async {
                    Json(serde_json::json!({
                        "check_runs": [{ "status": "completed", "conclusion": "success" }]
                    }))
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "reviewinbox");
        let inbox = auth
            .review_inbox("acme/site", &["orso".to_string()], "ada")
            .await
            .unwrap();
        assert_eq!(inbox.open, vec![1, 2, 3, 4, 5]);
        let items = inbox.reviews;
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].number, 1);
        assert!(!items[0].mine);
        assert_eq!(items[0].author, "bob");
        assert_eq!(items[0].head_sha, "h1");
        assert_eq!(items[0].checks, ChecksState::Passing);
        assert_eq!(items[0].my_state, MyReviewState::Greenlit);
        assert_eq!(items[1].number, 4);
        assert!(
            items[1].mine,
            "the signed-in user's own change is listed, marked mine"
        );
        assert_eq!(items[2].number, 5);
        assert_eq!(items[2].author, "ghost");
        assert!(!items[2].mine);
        let json = serde_json::to_value(&items[0]).unwrap();
        assert_eq!(json["headSha"], "h1");
        assert_eq!(json["myState"], "greenlit");
        stop();
    }

    #[tokio::test]
    async fn review_inbox_paginates() {
        use axum::extract::Query;
        use axum::routing::get;
        use axum::{Json, Router};
        use std::collections::HashMap;

        let routes = Router::new()
            .route(
                "/repos/acme/site/pulls",
                get(|Query(q): Query<HashMap<String, String>>| async move {
                    if q.get("page").map(String::as_str) == Some("2") {
                        return Json(serde_json::json!([
                            { "number": 101, "html_url": "u", "title": "late", "draft": false,
                              "labels": [{ "name": "orso" }], "user": { "login": "bob" } }
                        ]));
                    }
                    let page: Vec<_> = (1..=100)
                        .map(|n| {
                            serde_json::json!({ "number": n, "html_url": "u", "title": "x",
                                "draft": false, "labels": [], "user": { "login": "bob" } })
                        })
                        .collect();
                    Json(serde_json::Value::Array(page))
                }),
            )
            .route(
                "/repos/acme/site/pulls/101/reviews",
                get(|| async { Json(serde_json::json!([])) }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "reviewpages");
        let inbox = auth
            .review_inbox("acme/site", &["orso".to_string()], "ada")
            .await
            .unwrap();
        assert_eq!(inbox.open.len(), 101);
        let items = inbox.reviews;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].number, 101);
        stop();
    }

    #[tokio::test]
    async fn review_inbox_bounds_concurrency_and_keeps_order() {
        use axum::extract::Path as AxPath;
        use axum::routing::get;
        use axum::{Json, Router};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let live = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (l, pk) = (live.clone(), peak.clone());
        let routes = Router::new()
            .route(
                "/repos/acme/site/pulls",
                get(|| async {
                    let page: Vec<_> = (1..=12)
                        .map(|n| {
                            serde_json::json!({ "number": n, "html_url": "u", "title": "x",
                                "draft": false, "labels": [{ "name": "orso" }],
                                "user": { "login": "bob" } })
                        })
                        .collect();
                    Json(serde_json::Value::Array(page))
                }),
            )
            .route(
                "/repos/acme/site/pulls/{n}/reviews",
                get(move |AxPath(n): AxPath<u64>| {
                    let (live, peak) = (l.clone(), pk.clone());
                    async move {
                        let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        // Earlier PRs answer last, so order comes from the listing, not timing.
                        tokio::time::sleep(std::time::Duration::from_millis(5 * (13 - n))).await;
                        live.fetch_sub(1, Ordering::SeqCst);
                        Json(serde_json::json!([]))
                    }
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "reviewbounded");
        let inbox = auth
            .review_inbox("acme/site", &["orso".to_string()], "ada")
            .await
            .unwrap();
        let nums: Vec<u64> = inbox.reviews.iter().map(|r| r.number).collect();
        assert_eq!(nums, (1..=12).collect::<Vec<_>>());
        let peak = peak.load(Ordering::SeqCst);
        assert!((1..=INBOX_CONCURRENCY).contains(&peak), "peak {peak}");
        stop();
    }

    #[tokio::test]
    async fn can_push_reads_permissions() {
        use axum::routing::get;
        use axum::{Json, Router};

        let routes = Router::new()
            .route(
                "/repos/acme/site",
                get(|| async {
                    Json(
                        serde_json::json!({ "full_name": "acme/site", "private": false,
                        "default_branch": "main", "permissions": { "push": true } }),
                    )
                }),
            )
            .route(
                "/repos/acme/ro",
                get(|| async {
                    Json(
                        serde_json::json!({ "full_name": "acme/ro", "private": false,
                        "default_branch": "main" }),
                    )
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "canpush");
        assert!(auth.can_push("acme/site").await.unwrap());
        assert!(!auth.can_push("acme/ro").await.unwrap());
        stop();
    }

    #[test]
    fn production_url_strips_the_preview_alias_using_the_worker_name() {
        assert_eq!(
            production_url_from_preview(
                "https://content-setup-orso-skill-astro-marketing.blaisetiong-dev.workers.dev",
                "astro-marketing",
            )
            .as_deref(),
            Some("https://astro-marketing.blaisetiong-dev.workers.dev")
        );
        assert_eq!(
            production_url_from_preview("https://other.example.com", "astro-marketing"),
            None
        );
        assert_eq!(
            production_url_from_preview("javascript:alert(1)", "astro-marketing"),
            None
        );
    }

    #[tokio::test]
    async fn live_url_is_the_successful_deployment_of_main_tip() {
        use axum::extract::{Path as AxPath, Query};
        use axum::routing::get;
        use axum::{Json, Router};
        use std::collections::HashMap;

        let routes = Router::new()
            .route(
                "/repos/acme/site/branches/main",
                get(|| async {
                    Json(serde_json::json!({ "name": "main", "commit": { "sha": "mainsha" } }))
                }),
            )
            .route(
                "/repos/acme/site/deployments",
                get(|Query(q): Query<HashMap<String, String>>| async move {
                    if q.get("sha").map(String::as_str) != Some("mainsha") {
                        return Json(serde_json::json!([]));
                    }
                    Json(serde_json::json!([{ "id": 9, "sha": "mainsha" }]))
                }),
            )
            .route(
                "/repos/acme/site/deployments/{id}/statuses",
                get(|AxPath(_id): AxPath<u64>| async move {
                    Json(serde_json::json!([
                        { "state": "success", "environment_url": "https://acme.pages.dev" }
                    ]))
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "liveurl");
        assert_eq!(
            auth.live_url("acme/site", None).await.unwrap().as_deref(),
            Some("https://acme.pages.dev")
        );
        stop();
    }

    #[tokio::test]
    async fn live_url_derives_production_from_a_preview_when_main_has_none() {
        use axum::extract::{Path as AxPath, Query};
        use axum::routing::get;
        use axum::{Json, Router};
        use std::collections::HashMap;

        let routes = Router::new()
            .route(
                "/repos/acme/site/branches/main",
                get(|| async {
                    Json(serde_json::json!({ "name": "main", "commit": { "sha": "mainsha" } }))
                }),
            )
            .route(
                "/repos/acme/site/deployments",
                get(|Query(q): Query<HashMap<String, String>>| async move {
                    // Main tip has no deployment; unfiltered list still has PR previews.
                    if q.contains_key("sha") {
                        return Json(serde_json::json!([]));
                    }
                    Json(serde_json::json!([{ "id": 1, "sha": "previewsha" }]))
                }),
            )
            .route(
                "/repos/acme/site/deployments/{id}/statuses",
                get(|AxPath(_id): AxPath<u64>| async move {
                    Json(serde_json::json!([{
                        "state": "success",
                        "environment_url": "https://content-seo-astro-marketing.acct.workers.dev"
                    }]))
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "livefrompreview");
        assert_eq!(
            auth.live_url("acme/site", Some("astro-marketing"))
                .await
                .unwrap()
                .as_deref(),
            Some("https://astro-marketing.acct.workers.dev")
        );
        stop();
    }

    #[tokio::test]
    async fn preview_url_is_the_latest_successful_deployment_for_the_head_sha() {
        use axum::extract::{Path as AxPath, Query};
        use axum::routing::get;
        use axum::{Json, Router};
        use std::collections::HashMap;

        let routes = Router::new()
            .route(
                "/repos/acme/site/deployments",
                get(|Query(q): Query<HashMap<String, String>>| async move {
                    if q.get("sha").map(String::as_str) != Some("abc") {
                        return Json(serde_json::json!([]));
                    }
                    // Newest first, as GitHub lists them.
                    Json(serde_json::json!([
                        { "id": 3, "sha": "abc" },
                        { "id": 2, "sha": "abc" },
                        { "id": 1, "sha": "abc" },
                    ]))
                }),
            )
            .route(
                "/repos/acme/site/deployments/{id}/statuses",
                get(|AxPath(id): AxPath<u64>| async move {
                    Json(match id {
                        3 => serde_json::json!([{ "state": "failure", "environment_url": "" }]),
                        2 => serde_json::json!([
                            { "state": "success", "environment_url": "https://two.pages.dev" },
                            { "state": "in_progress" },
                        ]),
                        _ => serde_json::json!([
                            { "state": "success", "environment_url": "https://one.pages.dev" },
                        ]),
                    })
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "deploys");
        assert_eq!(
            auth.preview_url("acme/site", "abc")
                .await
                .unwrap()
                .as_deref(),
            Some("https://two.pages.dev")
        );
        assert_eq!(auth.preview_url("acme/site", "other").await.unwrap(), None);
        stop();
    }

    #[test]
    fn deploy_state_serializes_as_a_plain_word() {
        assert_eq!(
            serde_json::to_value(DeployState::Live).unwrap(),
            serde_json::json!("live")
        );
        assert_eq!(
            serde_json::to_value(DeployState::Unknown).unwrap(),
            serde_json::json!("unknown")
        );
    }

    #[tokio::test]
    async fn deploy_state_reads_the_merge_commits_check_runs() {
        use axum::extract::Path as AxPath;
        use axum::routing::get;
        use axum::{Json, Router};

        let routes = Router::new().route(
            "/repos/acme/site/commits/{sha}/check-runs",
            get(|AxPath(sha): AxPath<String>| async move {
                Json(match sha.as_str() {
                    "live" => serde_json::json!({ "check_runs": [
                        { "name": "test", "status": "completed", "conclusion": "failure" },
                        { "name": "deploy", "status": "completed", "conclusion": "success" },
                    ]}),
                    "building" => serde_json::json!({ "check_runs": [
                        { "name": "deploy", "status": "in_progress", "conclusion": null },
                    ]}),
                    _ => serde_json::json!({ "check_runs": [] }),
                })
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "deploy-state");
        assert_eq!(
            auth.deploy_state("acme/site", "live").await,
            DeployState::Live
        );
        assert_eq!(
            auth.deploy_state("acme/site", "building").await,
            DeployState::Building
        );
        assert_eq!(
            auth.deploy_state("acme/site", "none").await,
            DeployState::Unknown
        );
        stop();
    }

    #[tokio::test]
    async fn deploy_state_is_unknown_when_github_cannot_be_read() {
        use axum::routing::get;
        use axum::{Json, Router};

        let routes = Router::new().route(
            "/repos/acme/site/commits/{sha}/check-runs",
            get(|| async {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "message": "boom" })),
                )
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "deploy-broken");
        assert_eq!(
            auth.deploy_state("acme/site", "abc").await,
            DeployState::Unknown
        );
        stop();
    }

    #[tokio::test]
    async fn pr_status_combines_pr_reviews_checks_and_deployment() {
        use axum::routing::get;
        use axum::{Json, Router};

        let routes = Router::new()
            .route(
                "/repos/acme/site/pulls",
                get(|| async {
                    Json(serde_json::json!([{
                        "number": 4,
                        "html_url": "https://github.com/acme/site/pull/4",
                        "title": "Launch week",
                        "draft": false,
                        "state": "open",
                        "merged_at": null,
                        "mergeable": false,
                        "mergeable_state": "dirty",
                        "head": { "ref": "content/launch-week", "sha": "abc" }
                    }]))
                }),
            )
            .route(
                "/repos/acme/site/pulls/4/reviews",
                get(|| async {
                    Json(serde_json::json!([{ "user": { "login": "grace" }, "state": "APPROVED" }]))
                }),
            )
            .route(
                "/repos/acme/site/commits/abc/check-runs",
                get(|| async {
                    Json(serde_json::json!({
                        "total_count": 1,
                        "check_runs": [{ "status": "completed", "conclusion": "success" }]
                    }))
                }),
            )
            .route(
                "/repos/acme/site/deployments",
                get(|| async { Json(serde_json::json!([{ "id": 9, "sha": "abc" }])) }),
            )
            .route(
                "/repos/acme/site/deployments/9/statuses",
                get(|| async {
                    Json(serde_json::json!([
                        { "state": "success", "environment_url": "https://lw.pages.dev" }
                    ]))
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "prstatus");
        let st = auth
            .pr_status("acme/site", "content/launch-week")
            .await
            .unwrap()
            .expect("a PR");
        assert_eq!(st.number, 4);
        assert_eq!(st.url, "https://github.com/acme/site/pull/4");
        assert_eq!(st.state, PrState::Approved);
        assert_eq!(st.checks, ChecksState::Passing);
        assert_eq!(st.preview_url.as_deref(), Some("https://lw.pages.dev"));
        assert_eq!(st.mergeable, Some(false));
        assert_eq!(st.mergeable_state.as_deref(), Some("dirty"));
        let json = serde_json::to_value(&st).unwrap();
        assert_eq!(json["state"], "approved");
        assert_eq!(json["checks"], "passing");
        assert_eq!(json["previewUrl"], "https://lw.pages.dev");
        assert_eq!(json["mergeable"], false);
        assert_eq!(json["mergeableState"], "dirty");
        assert_eq!(json["upToDate"], false);
        stop();
    }

    #[tokio::test]
    async fn pr_status_is_none_without_a_pr_for_the_branch() {
        use axum::routing::get;
        use axum::{Json, Router};

        let routes = Router::new().route(
            "/repos/acme/site/pulls",
            get(|| async { Json(serde_json::json!([])) }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "noprstatus");
        assert_eq!(
            auth.pr_status("acme/site", "content/x").await.unwrap(),
            None
        );
        stop();
    }

    #[tokio::test]
    async fn main_head_reads_the_github_branch_tip() {
        use axum::routing::get;
        use axum::{Json, Router};

        let routes = Router::new().route(
            "/repos/acme/site/branches/main",
            get(|| async {
                Json(serde_json::json!({ "name": "main", "commit": { "sha": "abc" } }))
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "mainhead");
        assert_eq!(auth.main_head("acme/site").await.unwrap(), "abc");
        stop();
    }

    fn open_pull_json(number: u64, head: &str) -> serde_json::Value {
        serde_json::json!({
            "number": number,
            "html_url": format!("https://github.com/acme/site/pull/{number}"),
            "title": head,
            "draft": false,
            "state": "open",
            "merged_at": null,
            "head": { "ref": head, "sha": "abc" }
        })
    }

    #[tokio::test]
    async fn pr_status_survives_failing_checks_and_deployments_calls() {
        use axum::http::StatusCode;
        use axum::routing::get;
        use axum::{Json, Router};

        let routes = Router::new()
            .route(
                "/repos/acme/site/pulls",
                get(|| async { Json(serde_json::json!([open_pull_json(4, "content/x")])) }),
            )
            .route(
                "/repos/acme/site/pulls/4/reviews",
                get(|| async { Json(serde_json::json!([])) }),
            )
            .route(
                "/repos/acme/site/commits/abc/check-runs",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
            )
            .route(
                "/repos/acme/site/deployments",
                get(|| async { (StatusCode::BAD_GATEWAY, "boom") }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "statusfail");
        let st = auth
            .pr_status("acme/site", "content/x")
            .await
            .unwrap()
            .expect("a PR");
        assert_eq!(st.state, PrState::Open);
        assert_eq!(st.checks, ChecksState::None);
        assert_eq!(st.preview_url, None);
        stop();
    }

    #[tokio::test]
    async fn preview_url_survives_a_failing_statuses_call() {
        use axum::http::StatusCode;
        use axum::routing::get;
        use axum::{Json, Router};

        let routes = Router::new()
            .route(
                "/repos/acme/site/pulls",
                get(|| async { Json(serde_json::json!([open_pull_json(4, "content/x")])) }),
            )
            .route(
                "/repos/acme/site/pulls/4/reviews",
                get(|| async { Json(serde_json::json!([])) }),
            )
            .route(
                "/repos/acme/site/commits/abc/check-runs",
                get(|| async {
                    Json(serde_json::json!({ "check_runs": [{ "status": "queued" }] }))
                }),
            )
            .route(
                "/repos/acme/site/deployments",
                get(|| async { Json(serde_json::json!([{ "id": 1, "sha": "abc" }])) }),
            )
            .route(
                "/repos/acme/site/deployments/1/statuses",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "statusesfail");
        let st = auth
            .pr_status("acme/site", "content/x")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(st.checks, ChecksState::Pending);
        assert_eq!(st.preview_url, None);
        stop();
    }

    #[tokio::test]
    async fn preview_url_only_accepts_http_and_https() {
        use axum::extract::Path as AxPath;
        use axum::routing::get;
        use axum::{Json, Router};

        let routes = Router::new()
            .route(
                "/repos/acme/site/deployments",
                get(|| async {
                    Json(serde_json::json!([
                        { "id": 3, "sha": "abc" },
                        { "id": 2, "sha": "abc" },
                        { "id": 1, "sha": "abc" },
                    ]))
                }),
            )
            .route(
                "/repos/acme/site/deployments/{id}/statuses",
                get(|AxPath(id): AxPath<u64>| async move {
                    let url = match id {
                        3 => "javascript:alert(document.cookie)",
                        2 => "data:text/html,<script>1</script>",
                        _ => "http://ok.pages.dev",
                    };
                    Json(serde_json::json!([{ "state": "success", "environment_url": url }]))
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "unsafeurl");
        assert_eq!(
            auth.preview_url("acme/site", "abc")
                .await
                .unwrap()
                .as_deref(),
            Some("http://ok.pages.dev")
        );
        stop();
    }

    #[test]
    fn only_http_urls_are_safe_links() {
        assert!(is_http_url("https://x.pages.dev/a?b"));
        assert!(is_http_url("HTTP://x.dev"));
        for bad in [
            "javascript:alert(1)",
            " https://x",
            "data:text/html,x",
            "//x.dev",
            "ftp://x",
            "",
        ] {
            assert!(!is_http_url(bad), "{bad}");
        }
    }

    /// Serves `total` items over `per_page`-sized pages, honouring `?page=`.
    fn paged(
        q: &std::collections::HashMap<String, String>,
        total: usize,
        item: impl Fn(usize) -> serde_json::Value,
    ) -> Vec<serde_json::Value> {
        let per: usize = q.get("per_page").and_then(|v| v.parse().ok()).unwrap_or(30);
        let page: usize = q.get("page").and_then(|v| v.parse().ok()).unwrap_or(1);
        let start = (page - 1) * per;
        (start..total.min(start + per)).map(item).collect()
    }

    #[tokio::test]
    async fn reviews_and_check_runs_are_read_across_pages() {
        use axum::extract::Query;
        use axum::routing::get;
        use axum::{Json, Router};
        use std::collections::HashMap;

        // 150 reviews: alan requests changes early, approves on page 2.
        let routes = Router::new()
            .route(
                "/repos/acme/site/pulls",
                get(|| async { Json(serde_json::json!([open_pull_json(4, "content/x")])) }),
            )
            .route(
                "/repos/acme/site/pulls/4/reviews",
                get(|Query(q): Query<HashMap<String, String>>| async move {
                    Json(serde_json::json!(paged(&q, 150, |i| match i {
                        0 => serde_json::json!({ "user": { "login": "alan" }, "state": "CHANGES_REQUESTED" }),
                        149 => serde_json::json!({ "user": { "login": "alan" }, "state": "APPROVED" }),
                        _ => serde_json::json!({ "user": { "login": "bob" }, "state": "COMMENTED" }),
                    })))
                }),
            )
            .route(
                "/repos/acme/site/commits/abc/check-runs",
                get(|Query(q): Query<HashMap<String, String>>| async move {
                    let runs = paged(&q, 130, |i| {
                        if i == 129 {
                            serde_json::json!({ "status": "completed", "conclusion": "failure" })
                        } else {
                            serde_json::json!({ "status": "completed", "conclusion": "success" })
                        }
                    });
                    Json(serde_json::json!({ "total_count": 130, "check_runs": runs }))
                }),
            )
            .route(
                "/repos/acme/site/deployments",
                get(|| async { Json(serde_json::json!([])) }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "paging");
        let st = auth
            .pr_status("acme/site", "content/x")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(st.state, PrState::Approved);
        assert_eq!(st.checks, ChecksState::Failing);
        stop();
    }

    #[tokio::test]
    async fn pr_states_run_concurrently_and_time_out_per_branch() {
        use axum::extract::Query;
        use axum::routing::get;
        use axum::{Json, Router};
        use std::collections::HashMap;

        let routes = Router::new().route(
            "/repos/acme/site/pulls",
            get(|Query(q): Query<HashMap<String, String>>| async move {
                let head = q.get("head").cloned().unwrap_or_default();
                if head.ends_with("hung") {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                } else {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
                let mut pr = open_pull_json(1, &head);
                pr["draft"] = serde_json::json!(true);
                Json(serde_json::json!([pr]))
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "prstates");
        let branches: Vec<String> = [
            "content/a",
            "content/b",
            "content/c",
            "content/d",
            "content/hung",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let started = Instant::now();
        let states = auth
            .pr_states("acme/site", &branches, Duration::from_millis(1000))
            .await;
        let took = started.elapsed();
        let states: Vec<Option<PrState>> = states.into_iter().map(|r| r.map(|r| r.state)).collect();
        assert_eq!(
            states,
            vec![
                Some(PrState::Draft),
                Some(PrState::Draft),
                Some(PrState::Draft),
                Some(PrState::Draft),
                None
            ]
        );
        // Sequential would be ≥ 1.2s before the hung one; concurrent finishes at the timeout.
        assert!(took < Duration::from_millis(1500), "took {took:?}");
        stop();
    }

    /// `main_protection` against a mock with the given classic-protection and ruleset
    /// answers (`rules` = None → the rules endpoint fails with 500).
    async fn protection_with(
        tag: &str,
        classic: (u16, serde_json::Value),
        rules: Option<serde_json::Value>,
        delay: std::time::Duration,
    ) -> BranchProtection {
        use axum::http::StatusCode;
        use axum::routing::get;
        use axum::{Json, Router};

        let (code, body) = classic;
        let routes = Router::new()
            .route(
                "/repos/acme/site/branches/main/protection",
                get(move || async move {
                    tokio::time::sleep(delay).await;
                    (StatusCode::from_u16(code).unwrap(), Json(body))
                }),
            )
            .route(
                "/repos/acme/site/rules/branches/main",
                get(move || async move {
                    match rules {
                        Some(r) => (StatusCode::OK, Json(r)),
                        None => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({ "message": "boom" })),
                        ),
                    }
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let dir = tmp_dir(&format!("protection-{tag}"));
        write_session(&dir, "tok");
        let auth = GithubAuth::open(
            GithubConfig {
                api_url: base.clone(),
                web_url: base,
                gh_path: PathBuf::new(),
                ..GithubConfig::default()
            },
            &dir,
        );
        let got = auth.main_protection("acme/site").await;
        stop();
        got
    }

    #[tokio::test]
    async fn main_protection_maps_classic_protection_and_rulesets() {
        use serde_json::json;
        let now = std::time::Duration::ZERO;
        let unprotected = || (404, json!({ "message": "Branch not protected" }));
        let none = || Some(json!([]));
        let cases = [
            (
                "classic 200",
                (200, json!({})),
                none(),
                BranchProtection::Protected,
            ),
            (
                "classic 404 unprotected",
                unprotected(),
                none(),
                BranchProtection::Unprotected,
            ),
            // Repo hidden from the caller: GitHub says "Not Found", not "Branch not protected".
            (
                "404 Not Found",
                (404, json!({ "message": "Not Found" })),
                none(),
                BranchProtection::Unknown,
            ),
            // No admin rights: GitHub refuses to say, so we must not nag.
            (
                "403",
                (403, json!({ "message": "x" })),
                none(),
                BranchProtection::Unknown,
            ),
            (
                "500",
                (500, json!({ "message": "x" })),
                none(),
                BranchProtection::Unknown,
            ),
            (
                "ruleset requires a pull request",
                unprotected(),
                Some(json!([{ "type": "deletion" }, { "type": "pull_request" }])),
                BranchProtection::Protected,
            ),
            (
                "ruleset without a pull request rule",
                unprotected(),
                Some(json!([{ "type": "deletion" }])),
                BranchProtection::Unprotected,
            ),
            (
                "rules endpoint fails",
                unprotected(),
                None,
                BranchProtection::Unknown,
            ),
        ];
        for (i, (name, classic, rules, want)) in cases.into_iter().enumerate() {
            let got = protection_with(&format!("case{i}"), classic, rules, now).await;
            assert_eq!(got, want, "{name}");
        }
    }

    #[tokio::test]
    async fn a_slow_protection_lookup_gives_up_as_unknown() {
        let started = std::time::Instant::now();
        let got = protection_with(
            "slow",
            (200, serde_json::json!({})),
            Some(serde_json::json!([])),
            std::time::Duration::from_secs(20),
        )
        .await;
        assert_eq!(got, BranchProtection::Unknown);
        assert!(started.elapsed() < std::time::Duration::from_secs(6));
    }

    #[test]
    fn branch_settings_url_points_at_the_repo_settings_page() {
        let auth = GithubAuth::open(
            GithubConfig {
                web_url: "https://github.com/".into(),
                gh_path: PathBuf::new(),
                ..GithubConfig::default()
            },
            &tmp_dir("settings-url"),
        );
        assert_eq!(
            auth.branch_settings_url("acme/site"),
            "https://github.com/acme/site/settings/branches"
        );
    }

    /// `GET /repos/acme/site/pulls` serving one open PR with node id `PR_abc`.
    fn one_open_pull(draft: bool) -> axum::Router {
        use axum::routing::get;
        axum::Router::new().route(
            "/repos/acme/site/pulls",
            get(move || async move {
                axum::Json(serde_json::json!([{
                    "number": 4,
                    "node_id": "PR_abc",
                    "html_url": "https://github.com/acme/site/pull/4",
                    "title": "Launch week",
                    "draft": draft,
                    "state": "open",
                    "head": { "ref": "content/launch-week", "sha": "abc" }
                }]))
            }),
        )
    }

    #[tokio::test]
    async fn mark_ready_for_review_sends_the_graphql_mutation_with_the_pr_node_id() {
        use axum::extract::Json as AxJson;
        use axum::routing::post;
        use std::sync::{Arc, Mutex};

        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        let routes = one_open_pull(true).route(
            "/graphql",
            post(move |AxJson(body): AxJson<serde_json::Value>| {
                let seen2 = seen2.clone();
                async move {
                    seen2.lock().unwrap().push(body);
                    axum::Json(serde_json::json!({
                        "data": { "markPullRequestReadyForReview": { "pullRequest": { "isDraft": false } } }
                    }))
                }
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "ready");
        auth.mark_ready_for_review("acme/site", "content/launch-week")
            .await
            .unwrap();
        let sent = seen.lock().unwrap().clone();
        assert_eq!(sent.len(), 1);
        assert!(sent[0]["query"]
            .as_str()
            .unwrap()
            .contains("markPullRequestReadyForReview(input: { pullRequestId: $id })"));
        assert_eq!(sent[0]["variables"]["id"], "PR_abc");
        stop();
    }

    #[tokio::test]
    async fn mark_ready_for_review_skips_a_pr_that_is_not_a_draft() {
        use axum::routing::post;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let calls = Arc::new(AtomicUsize::new(0));
        let calls2 = calls.clone();
        let routes = one_open_pull(false).route(
            "/graphql",
            post(move || {
                calls2.fetch_add(1, Ordering::SeqCst);
                async { axum::Json(serde_json::json!({ "data": {} })) }
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "ready-noop");
        auth.mark_ready_for_review("acme/site", "content/launch-week")
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        stop();
    }

    #[tokio::test]
    async fn mark_ready_for_review_without_an_open_pr_is_not_found() {
        use axum::routing::get;
        let routes = axum::Router::new().route(
            "/repos/acme/site/pulls",
            get(|| async { axum::Json(serde_json::json!([])) }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "ready-none");
        let err = auth
            .mark_ready_for_review("acme/site", "content/launch-week")
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Enoent);
        stop();
    }

    #[tokio::test]
    async fn mark_ready_for_review_surfaces_403_and_graphql_errors() {
        use axum::routing::post;

        // HTTP 403 from the GraphQL endpoint.
        let routes = one_open_pull(true).route(
            "/graphql",
            post(|| async {
                (
                    axum::http::StatusCode::FORBIDDEN,
                    axum::Json(
                        serde_json::json!({ "message": "Resource not accessible by integration" }),
                    ),
                )
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "ready-403");
        let err = auth
            .mark_ready_for_review("acme/site", "content/launch-week")
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Sandbox);
        assert_eq!(err.http_status(), 403);
        stop();

        // GraphQL reports a FORBIDDEN error with HTTP 200.
        let routes = one_open_pull(true).route(
            "/graphql",
            post(|| async {
                axum::Json(serde_json::json!({
                    "data": { "markPullRequestReadyForReview": null },
                    "errors": [{ "type": "FORBIDDEN", "message": "Ada does not have permission" }]
                }))
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "ready-gql-403");
        let err = auth
            .mark_ready_for_review("acme/site", "content/launch-week")
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Sandbox);
        assert!(
            err.message.contains("Ada does not have permission"),
            "{}",
            err.message
        );
        stop();

        // Any other GraphQL error.
        let routes = one_open_pull(true).route(
            "/graphql",
            post(|| async {
                axum::Json(serde_json::json!({
                    "errors": [{ "type": "NOT_FOUND", "message": "Could not resolve" }]
                }))
            }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "ready-gql-err");
        let err = auth
            .mark_ready_for_review("acme/site", "content/launch-week")
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::GithubErr);
        assert!(err.message.contains("Could not resolve"), "{}", err.message);
        stop();
    }

    /// `/graphql` answering every request with `body` (HTTP 200).
    fn graphql_answers(draft: bool, body: serde_json::Value) -> axum::Router {
        use axum::routing::post;
        one_open_pull(draft).route(
            "/graphql",
            post(move || {
                let body = body.clone();
                async move { axum::Json(body) }
            }),
        )
    }

    async fn mark_ready_err(routes: axum::Router, tag: &str) -> ApiError {
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, tag);
        let err = auth
            .mark_ready_for_review("acme/site", "content/launch-week")
            .await
            .unwrap_err();
        stop();
        err
    }

    #[tokio::test]
    async fn find_pr_carries_the_pr_node_id() {
        let (base, stop) = mock_api(one_open_pull(true)).await;
        let auth = status_auth(base, "find-node-id");
        let pr = auth
            .find_pr("acme/site", "content/launch-week")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(pr.node_id, "PR_abc");
        // Internal: not part of any API payload.
        assert!(serde_json::to_value(&pr).unwrap().get("nodeId").is_none());
        stop();
    }

    #[tokio::test]
    async fn mark_ready_for_review_needs_the_mutation_to_report_the_pr_out_of_draft() {
        for (tag, body) in [
            (
                "ready-null",
                serde_json::json!({ "data": { "markPullRequestReadyForReview": null } }),
            ),
            ("ready-nodata", serde_json::json!({})),
            (
                "ready-still-draft",
                serde_json::json!({
                    "data": { "markPullRequestReadyForReview": { "pullRequest": { "isDraft": true } } }
                }),
            ),
        ] {
            let err = mark_ready_err(graphql_answers(true, body), tag).await;
            assert_eq!(err.code, ErrorCode::GithubErr, "{tag}");
            assert!(
                err.message.contains("for a greenlight"),
                "{tag}: {}",
                err.message
            );
        }
    }

    #[tokio::test]
    async fn mark_ready_for_review_maps_permission_error_types_to_sandbox() {
        for kind in ["FORBIDDEN", "INSUFFICIENT_SCOPES"] {
            let err = mark_ready_err(
                graphql_answers(
                    true,
                    serde_json::json!({ "errors": [{ "type": kind, "message": "nope" }] }),
                ),
                &format!("ready-{kind}"),
            )
            .await;
            assert_eq!(err.code, ErrorCode::Sandbox, "{kind}");
            assert_eq!(err.http_status(), 403, "{kind}");
        }
    }

    #[tokio::test]
    async fn mark_ready_for_review_http_errors_say_what_failed() {
        use axum::routing::post;
        for (status, tag) in [
            (axum::http::StatusCode::FORBIDDEN, "ready-msg-403"),
            (
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "ready-msg-422",
            ),
            (axum::http::StatusCode::BAD_GATEWAY, "ready-msg-502"),
        ] {
            let routes = one_open_pull(true).route(
                "/graphql",
                post(move || async move {
                    (status, axum::Json(serde_json::json!({ "message": "nope" })))
                }),
            );
            let err = mark_ready_err(routes, tag).await;
            assert!(
                err.message.contains("for a greenlight"),
                "{tag}: {}",
                err.message
            );
            assert!(
                !err.message.contains("push access"),
                "{tag}: {}",
                err.message
            );
            assert!(
                !err.message.contains("share this change"),
                "{tag}: {}",
                err.message
            );
        }
    }

    #[tokio::test]
    async fn mark_ready_for_review_returns_an_open_status_even_if_the_reread_lags_or_fails() {
        use axum::extract::Query;
        use axum::routing::get;
        use std::collections::HashMap;

        let ok = serde_json::json!({
            "data": { "markPullRequestReadyForReview": { "pullRequest": { "isDraft": false } } }
        });

        // REST still reports the PR as a draft right after the GraphQL write.
        let (base, stop) = mock_api(graphql_answers(true, ok.clone())).await;
        let auth = status_auth(base, "ready-stale");
        let st = auth
            .mark_ready_for_review("acme/site", "content/launch-week")
            .await
            .unwrap();
        assert_eq!(st.state, PrState::Open);
        assert_eq!(st.number, 4);
        stop();

        // The follow-up status read fails outright.
        let routes = axum::Router::new()
            .route(
                "/repos/acme/site/pulls",
                get(|Query(q): Query<HashMap<String, String>>| async move {
                    if q.get("state").map(String::as_str) == Some("all") {
                        return (
                            axum::http::StatusCode::BAD_GATEWAY,
                            axum::Json(serde_json::json!({ "message": "boom" })),
                        );
                    }
                    (
                        axum::http::StatusCode::OK,
                        axum::Json(serde_json::json!([{
                            "number": 4,
                            "node_id": "PR_abc",
                            "html_url": "https://github.com/acme/site/pull/4",
                            "title": "Launch week",
                            "draft": true,
                            "state": "open"
                        }])),
                    )
                }),
            )
            .route(
                "/graphql",
                axum::routing::post(move || {
                    let ok = ok.clone();
                    async move { axum::Json(ok) }
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "ready-reread-fails");
        let st = auth
            .mark_ready_for_review("acme/site", "content/launch-week")
            .await
            .unwrap();
        assert_eq!(
            st,
            PrStatus {
                number: 4,
                url: "https://github.com/acme/site/pull/4".into(),
                state: PrState::Open,
                checks: ChecksState::None,
                preview_url: None,
                mergeable: None,
                mergeable_state: None,
                up_to_date: false,
                sent_back: None,
                head_sha: String::new(),
            }
        );
        stop();
    }

    /// `/repos/acme/site/pulls/4/merge` records each request body and answers `status`/`body`.
    fn merge_endpoint(
        status: axum::http::StatusCode,
        response: serde_json::Value,
        seen: std::sync::Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    ) -> axum::Router {
        use axum::extract::Json as AxJson;
        use axum::routing::put;
        one_open_pull(false)
            .route(
                "/repos/acme/site/pulls/4/reviews",
                axum::routing::get(|| async { axum::Json(serde_json::json!([])) }),
            )
            .route(
                "/repos/acme/site/pulls/4/merge",
                put(move |AxJson(body): AxJson<serde_json::Value>| {
                    let seen = seen.clone();
                    let response = response.clone();
                    async move {
                        seen.lock().unwrap().push(body);
                        (status, axum::Json(response))
                    }
                }),
            )
    }

    #[tokio::test]
    async fn merge_pr_refuses_a_sent_back_change_without_merging() {
        use std::sync::{Arc, Mutex};

        let seen = Arc::new(Mutex::new(Vec::new()));
        let merges = seen.clone();
        let routes = one_open_pull(false)
            .route(
                "/repos/acme/site/pulls/4/reviews",
                axum::routing::get(|| async {
                    axum::Json(serde_json::json!([
                        { "user": { "login": "alan" }, "state": "CHANGES_REQUESTED",
                          "commit_id": "abc", "body": "Fix the headline" }
                    ]))
                }),
            )
            .route(
                "/repos/acme/site/pulls/4/merge",
                axum::routing::put(move |axum::Json(b): axum::Json<serde_json::Value>| {
                    let merges = merges.clone();
                    async move {
                        merges.lock().unwrap().push(b);
                        (
                            axum::http::StatusCode::OK,
                            axum::Json(serde_json::json!({ "merged": true, "sha": "def" })),
                        )
                    }
                }),
            );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "merge-sent-back");
        let err = auth
            .merge_pr("acme/site", "content/launch-week")
            .await
            .unwrap_err();
        stop();

        assert_eq!(err.http_status(), 409);
        assert_eq!(err.message, "A reviewer sent this change back.");
        assert!(seen.lock().unwrap().is_empty(), "no merge request was sent");
    }

    #[tokio::test]
    async fn merge_pr_merges_open_pull() {
        use std::sync::{Arc, Mutex};

        let seen = Arc::new(Mutex::new(Vec::new()));
        let routes = merge_endpoint(
            axum::http::StatusCode::OK,
            serde_json::json!({
                "merged": true,
                "message": "Pull Request successfully merged",
                "sha": "def",
            }),
            seen.clone(),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "merge-ok");
        let merged = auth
            .merge_pr("acme/site", "content/launch-week")
            .await
            .unwrap();
        stop();

        assert_eq!(
            merged,
            PublishResult {
                number: 4,
                url: "https://github.com/acme/site/pull/4".into(),
                state: PrState::Merged,
                merge_sha: Some("def".into()),
            }
        );
        // The merge is a PUT on the PR found by head, with an explicit merge method.
        assert_eq!(
            seen.lock().unwrap().clone(),
            vec![serde_json::json!({ "merge_method": "merge" })]
        );
    }

    /// None of the words the spec keeps out of writer-facing copy.
    fn assert_plain_publish_copy(message: &str) {
        for word in ["branch", "commit", "merge", "conflict", "checkout", "clone"] {
            assert!(
                !message.to_lowercase().contains(word),
                "{message:?} mentions {word}"
            );
        }
        for word in ["pr", "main", "pull request"] {
            assert!(
                !message
                    .to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|w| w == word),
                "{message:?} mentions {word}"
            );
        }
    }

    #[tokio::test]
    async fn merge_pr_maps_not_mergeable() {
        use std::sync::{Arc, Mutex};

        for status in [
            axum::http::StatusCode::METHOD_NOT_ALLOWED,
            axum::http::StatusCode::CONFLICT,
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        ] {
            let routes = merge_endpoint(
                status,
                serde_json::json!({ "message": "Pull Request is not mergeable" }),
                Arc::new(Mutex::new(Vec::new())),
            );
            let (base, stop) = mock_api(routes).await;
            let auth = status_auth(base, &format!("merge-{status}"));
            let err = auth
                .merge_pr("acme/site", "content/launch-week")
                .await
                .unwrap_err();
            stop();

            assert_eq!(err.code, ErrorCode::Conflict, "{status}");
            assert_eq!(err.http_status(), 409, "{status}");
            assert!(
                err.message.starts_with("Couldn't publish"),
                "{status}: {}",
                err.message
            );
            assert!(err.message.contains("latest"), "{status}: {}", err.message);
            assert_plain_publish_copy(&err.message);
        }
    }

    #[tokio::test]
    async fn merge_pr_asks_an_owner_for_permission() {
        use std::sync::{Arc, Mutex};

        let routes = merge_endpoint(
            axum::http::StatusCode::FORBIDDEN,
            serde_json::json!({ "message": "Resource not accessible by integration" }),
            Arc::new(Mutex::new(Vec::new())),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "merge-403");
        let err = auth
            .merge_pr("acme/site", "content/launch-week")
            .await
            .unwrap_err();
        stop();

        assert_eq!(err.code, ErrorCode::Sandbox);
        assert_eq!(err.http_status(), 403);
        assert!(err.message.contains("Ask an owner"), "{}", err.message);
        assert_plain_publish_copy(&err.message);
    }

    #[tokio::test]
    async fn merge_pr_without_an_open_pull_says_share_first() {
        use axum::routing::get;

        let routes = axum::Router::new().route(
            "/repos/acme/site/pulls",
            get(|| async { axum::Json(serde_json::json!([])) }),
        );
        let (base, stop) = mock_api(routes).await;
        let auth = status_auth(base, "merge-none");
        let err = auth
            .merge_pr("acme/site", "content/launch-week")
            .await
            .unwrap_err();
        stop();

        assert_eq!(err.code, ErrorCode::Enoent);
        assert!(
            err.message.starts_with("Couldn't publish"),
            "{}",
            err.message
        );
        assert_plain_publish_copy(&err.message);
    }

    #[test]
    fn graphql_endpoint_follows_the_rest_api_base() {
        assert_eq!(
            graphql_url("https://api.github.com"),
            "https://api.github.com/graphql"
        );
        assert_eq!(
            graphql_url("https://ghe.acme.dev/api/v3"),
            "https://ghe.acme.dev/api/graphql"
        );
        assert_eq!(
            graphql_url("http://127.0.0.1:9"),
            "http://127.0.0.1:9/graphql"
        );
    }
}
