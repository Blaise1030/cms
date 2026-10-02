//! GitHub sign-in (OAuth device flow), the signed-in account, and repository access.
//!
//! The session (token + profile) lives in `{projects-dir}/.workbench/github.json`, mode 0600.
//! The token is sent to the GitHub API as a bearer token and to git per command through
//! `GIT_CONFIG_*` environment variables, so it never appears in argv or in a repo's config.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
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

pub struct GithubAuth {
    cfg: GithubConfig,
    path: PathBuf,
    http: reqwest::Client,
    inner: Mutex<Inner>,
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
        }
    }

    pub async fn status(&self) -> Status {
        Status {
            configured: self.cfg.client_id.is_some(),
            user: self
                .inner
                .lock()
                .await
                .session
                .as_ref()
                .map(|s| s.user.clone()),
        }
    }

    pub async fn token(&self) -> Option<String> {
        self.inner
            .lock()
            .await
            .session
            .as_ref()
            .map(|s| s.token.clone())
    }

    /// The signed-in person's commit identity, or [`Identity::workbench`].
    pub async fn identity(&self) -> Identity {
        self.inner
            .lock()
            .await
            .session
            .as_ref()
            .map(|s| s.user.identity())
            .unwrap_or_else(Identity::workbench)
    }

    /// `{git_url}/{repo}.git` for a validated `owner/name`.
    pub fn clone_url(&self, repo: &str) -> String {
        format!("{}/{repo}.git", self.cfg.git_url.trim_end_matches('/'))
    }

    /// Environment that authenticates git to `git_url` with the session token.
    pub async fn git_env(&self) -> Result<Vec<(String, String)>, ApiError> {
        let token = self.token().await.ok_or_else(not_signed_in)?;
        Ok(git_auth_env(&self.cfg.git_url, &token))
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
            return (Err(not_signed_in()), false);
        };
        let mut cleared = false;
        let r = self.list_repos(&token, &mut cleared).await;
        (r, cleared)
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
        Ok(repos
            .into_iter()
            .map(|r| RepoInfo {
                full_name: r.full_name,
                private: r.private,
                default_branch: r.default_branch,
                can_push: r.permissions.is_some_and(|p| p.push),
            })
            .collect())
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
        let auth = GithubAuth::open(GithubConfig::default(), &dir);
        let st = auth.status().await;
        assert!(!st.configured);
        assert!(st.user.is_none());
        assert_eq!(auth.identity().await, Identity::workbench());
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
        let auth = GithubAuth::open(GithubConfig::default(), &dir);
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
        let auth = GithubAuth::open(GithubConfig::default(), &dir);
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
