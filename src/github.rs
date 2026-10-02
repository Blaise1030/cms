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
    async fn run_gh(&self, args: &[&str], secs: u64) -> Result<GhOutput, GhFail> {
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
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let out = match tokio::time::timeout(Duration::from_secs(secs), cmd.output()).await {
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

fn unreachable(e: reqwest::Error) -> ApiError {
    ApiError::github(format!("cannot reach GitHub: {e}"))
}

pub fn not_signed_in() -> ApiError {
    ApiError::github("sign in with GitHub first").with_status(400)
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
}
