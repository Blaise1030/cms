//! GitHub sign-in, the signed-in account, and repository access.
//!
//! Two sources, in order: a device-flow session saved by this daemon (`oauth`), else the
//! machine's GitHub CLI login (`cli`). The CLI token is never read: the daemon only runs
//! `gh api ...` and lets git call `gh auth git-credential`.
//!
//! The session (token + profile) lives in `{projects-dir}/.workbench/github.json`, mode 0600.
//! The token is sent to the GitHub API as a bearer token and to git per command through
//! `GIT_CONFIG_*` environment variables, so it never appears in argv or in a repo's config.

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

/// Draft PR body references the change title.
pub fn pr_body(change_title: &str) -> String {
    format!("Preview of change: {}", change_title.trim())
}

impl From<ApiRepo> for RepoInfo {
    fn from(r: ApiRepo) -> Self {
        RepoInfo {
            full_name: r.full_name,
            private: r.private,
            default_branch: r.default_branch,
            can_push: r.permissions.is_some_and(|p| p.push),
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
    #[serde(default)]
    head: Option<ApiPullHead>,
}

#[derive(Deserialize)]
struct ApiPullHead {
    #[serde(default)]
    sha: String,
}

/// Where a pull request stands, as shown in the preview browser bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PrState {
    Draft,
    Open,
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
    /// The PR's head commit (ties the PR to the local branch's commits; not sent to the UI).
    #[serde(skip)]
    pub head_sha: String,
}

/// A pull request's [`PrState`] and head commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrRef {
    pub state: PrState,
    pub head_sha: String,
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
}

#[derive(Deserialize)]
struct ApiReview {
    #[serde(default)]
    user: Option<ApiLogin>,
    state: String,
}

#[derive(Deserialize)]
struct ApiCheckRuns {
    #[serde(default)]
    check_runs: Vec<ApiCheckRun>,
}

#[derive(Deserialize)]
struct ApiCheckRun {
    status: String,
    #[serde(default)]
    conclusion: Option<String>,
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
    // Reviews arrive oldest first; keep each reviewer's latest decisive one.
    let mut latest: Vec<(&str, &str)> = Vec::new();
    for r in reviews {
        let state = r.state.as_str();
        if !matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "DISMISSED") {
            continue;
        }
        let who = r.user.as_ref().map_or("", |u| u.login.as_str());
        match latest.iter_mut().find(|(u, _)| *u == who) {
            Some(entry) => entry.1 = state,
            None => latest.push((who, state)),
        }
    }
    let approved = latest.iter().any(|(_, s)| *s == "APPROVED")
        && !latest.iter().any(|(_, s)| *s == "CHANGES_REQUESTED");
    if approved {
        PrState::Approved
    } else {
        PrState::Open
    }
}

/// Only `http://` / `https://` URLs may become links (a deployment's `environment_url` is
/// whatever the workflow wrote, e.g. `javascript:`).
fn is_http_url(u: &str) -> bool {
    let lower = u.get(..8).unwrap_or(u).to_ascii_lowercase();
    (lower.starts_with("http://") && u.len() > 7) || (lower.starts_with("https://") && u.len() > 8)
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
}

impl GithubAuth {
    /// Load a saved session from `state_dir` if there is one.
    pub fn open(cfg: GithubConfig, state_dir: &Path) -> Self {
        let path = state_dir.join(SESSION_FILE);
        let session = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Session>(&b).ok());
        let http = reqwest::Client::builder()
            .user_agent("workbench")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("HTTP client");
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

    /// The signed-in person's commit identity, or [`Identity::workbench`]. With the GitHub
    /// CLI it is the machine's global git identity when set, else the GitHub profile.
    pub async fn identity(&self) -> Identity {
        if let Some(s) = self.inner.lock().await.session.as_ref() {
            return s.user.identity();
        }
        let cli = self.cli_state().await;
        match (cli.git_identity, cli.user) {
            (Some(id), _) => id,
            (None, Some(u)) => u.identity(),
            (None, None) => Identity::workbench(),
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

    /// Create a draft pull request from `head_branch` into `main`.
    pub async fn create_draft_pr(
        &self,
        repo: &str,
        head_branch: &str,
        title: &str,
        body: &str,
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
            ApiError::enoent("this change has no open pull request — share it first")
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
        const WHAT: &str = "GitHub could not mark the pull request ready for review";
        if node_id.is_empty() {
            return Err(ApiError::github(format!("{WHAT}: no pull request id")));
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
        let excerpt =
            |n: usize| -> String { String::from_utf8_lossy(&body).chars().take(n).collect() };
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(ApiError::sandbox(
                "You don't have permission to mark this pull request ready for review",
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
                        "You don't have permission to mark this pull request ready for review: {msg}"
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

    /// PR state, checks and preview URL for `head_branch`; `None` when it has no pull request.
    pub async fn pr_status(
        &self,
        repo: &str,
        head_branch: &str,
    ) -> Result<Option<PrStatus>, ApiError> {
        let Some((pull, state)) = self.pull_and_state(repo, head_branch).await? else {
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
            head_sha: sha,
        }))
    }

    /// Just the [`PrState`] and head of `head_branch`'s pull request (no checks or deployments).
    pub async fn pr_ref(&self, repo: &str, head_branch: &str) -> Result<Option<PrRef>, ApiError> {
        Ok(self
            .pull_and_state(repo, head_branch)
            .await?
            .map(|(pull, state)| PrRef {
                state,
                head_sha: pull.head.map(|h| h.sha).unwrap_or_default(),
            }))
    }

    /// Newest pull request for `head_branch` in any state, with its reviews folded into a state.
    async fn pull_and_state(
        &self,
        repo: &str,
        head_branch: &str,
    ) -> Result<Option<(ApiPull, PrState)>, ApiError> {
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
        Ok(Some((pull, state)))
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
        let mut all = Vec::new();
        for page in 1..=MAX_PAGES {
            let batch = items(
                self.get_json::<P>(&format!("{path}?per_page={PAGE_SIZE}&page={page}"))
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

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        self.api_json(reqwest::Method::GET, path, None).await
    }

    /// A REST call to `path` with the session token, else through `gh api` when only the
    /// GitHub CLI is logged in.
    async fn api_json<T: DeserializeOwned>(
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

    /// `gh api --method {method} {path}`, with `body` as the JSON request body.
    async fn gh_api(
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

/// A failed (non-401) GitHub API response: 403 and 422 get their own [`ApiError`] codes.
fn github_error(status: reqwest::StatusCode, body: &[u8]) -> ApiError {
    if status == reqwest::StatusCode::FORBIDDEN {
        return ApiError::sandbox("You don't have push access");
    }
    if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
        let text: String = String::from_utf8_lossy(body).chars().take(200).collect();
        return ApiError::conflict(format!("GitHub could not open the pull request: {text}"));
    }
    let text: String = String::from_utf8_lossy(body).chars().take(300).collect();
    ApiError::github(format!("GitHub returned {status}: {text}"))
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
        assert_eq!(pr_body("Launch week"), "Preview of change: Launch week");
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
        assert_eq!(auth.identity().await, Identity::workbench());
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
        assert_eq!(auth.identity().await, Identity::workbench());
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
        let routes = Router::new().route(
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
        let body = pr_body("Launch week");
        let pr = auth
            .create_draft_pr("acme/site", "content/launch-week", &title, &body)
            .await
            .unwrap();
        assert_eq!(pr.number, 1);
        let sent = seen.lock().unwrap().clone().unwrap();
        assert_eq!(sent["draft"], true);
        assert_eq!(sent["title"], "Launch week");
        assert_eq!(sent["body"], "Preview of change: Launch week");
        assert_eq!(sent["head"], "content/launch-week");
        assert_eq!(sent["base"], "main");
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
            .create_draft_pr("acme/site", "content/x", "X", "Preview of change: X")
            .await
            .unwrap();
        assert_eq!(pr.number, 7);

        let calls = std::fs::read_to_string(dir.join("gh.calls")).unwrap();
        assert!(calls
            .contains("api --method GET /repos/acme/site/pulls?head=acme:content%2Fx&state=open"));
        assert!(calls.contains("api --method POST /repos/acme/site/pulls --input -"));
        let sent: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("gh.stdin")).unwrap()).unwrap();
        assert_eq!(sent["head"], "content/x");
        assert_eq!(sent["base"], "main");
        assert_eq!(sent["draft"], true);
    }

    #[tokio::test]
    async fn gh_api_http_errors_map_like_direct_responses() {
        let dir = tmp_dir("gh422");
        let auth = gh_with_api(
            &dir,
            r#"printf '{"message":"Validation Failed"}'; echo 'gh: Validation Failed (HTTP 422)' >&2; exit 1"#,
        );
        let err = auth
            .create_draft_pr("acme/site", "content/x", "X", "b")
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
            user: Some(ApiLogin {
                login: login.into(),
            }),
            state: state.into(),
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
        // A later change request from the same reviewer withdraws it.
        assert_eq!(
            pr_state(
                &open,
                &[
                    review("grace", "APPROVED"),
                    review("grace", "CHANGES_REQUESTED")
                ]
            ),
            PrState::Open
        );
        // Any outstanding change request blocks approval.
        assert_eq!(
            pr_state(
                &open,
                &[
                    review("grace", "APPROVED"),
                    review("alan", "CHANGES_REQUESTED")
                ]
            ),
            PrState::Open
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

    fn run(status: &str, conclusion: Option<&str>) -> ApiCheckRun {
        ApiCheckRun {
            status: status.into(),
            conclusion: conclusion.map(Into::into),
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
        let json = serde_json::to_value(&st).unwrap();
        assert_eq!(json["state"], "approved");
        assert_eq!(json["checks"], "passing");
        assert_eq!(json["previewUrl"], "https://lw.pages.dev");
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
                err.message.contains("ready for review"),
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
                err.message.contains("ready for review"),
                "{tag}: {}",
                err.message
            );
            assert!(
                !err.message.contains("push access"),
                "{tag}: {}",
                err.message
            );
            assert!(
                !err.message.contains("open the pull request"),
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
                head_sha: String::new(),
            }
        );
        stop();
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
