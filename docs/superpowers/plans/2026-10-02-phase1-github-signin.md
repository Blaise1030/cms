# Phase 1: GitHub Sign-in, Open from GitHub, Real Authors — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A writer signs in with GitHub inside the workbench, opens any repository they can access as a project, and every commit made in it is authored by them.

**Architecture:** A new `github` module in the daemon runs GitHub's OAuth device flow, stores the session in `{projects-dir}/.workbench/github.json` (mode 0600) and talks to the GitHub API with `reqwest`. Git authentication is passed per command through `GIT_CONFIG_*` environment variables (never argv, never repo config). Commit identity moves from the hardcoded `-c user.name=Workbench` to each repo's local `user.name`/`user.email`, which sign-in and sign-out rewrite. The web app gets a GitHub account control in the header, a device-code dialog, and an "Open from GitHub" dialog.

**Tech Stack:** Rust (axum 0.8, tokio, reqwest + rustls, base64), Vue 3 + Pinia + shadcn-vue, Vitest, git ≥ 2.31 (for `GIT_CONFIG_COUNT`).

Roadmap and assumptions: `docs/superpowers/plans/2026-10-02-multi-writer-roadmap.md`.

---

## File structure

| File | Responsibility |
|---|---|
| `src/github.rs` (new) | Device flow, session storage, `/user` + `/user/repos`, repo-name validation, git auth env |
| `src/http/github.rs` (new) | `/api/github/*` handlers |
| `src/git.rs` | `Identity`, `set_identity`, `has_identity`; `run()` stops forcing an identity |
| `src/projects.rs` | `Source` (template / clone), `create_from`, `repo` field, `apply_identity`, `ensure_identities` |
| `src/http/projects.rs` | `POST /api/projects/clone`; `create` uses the signed-in identity |
| `src/config.rs`, `src/main.rs` | `GithubConfig`, `--github-client-id` / `WORKBENCH_GITHUB_CLIENT_ID` |
| `src/error.rs` | `GITHUB_ERR` code |
| `src/state.rs`, `src/lib.rs` | `AppState.github`; identity migration at startup |
| `tests/github.rs` (new) | End-to-end against a fake GitHub server and a local bare repo |
| `web/src/core/types.ts`, `web/src/core/api.ts` | GitHub types and calls, `Project.repo` |
| `web/src/modules/github/store.ts` (new) | Status, device-flow polling, sign-out, repo list |
| `web/src/modules/github/GithubAccount.vue` (new) | Header sign-in button / account menu |
| `web/src/modules/github/DeviceCodeDialog.vue` (new) | Shows the code, opens GitHub, waits |
| `web/src/modules/projects/OpenRepoDialog.vue` (new) | Repo picker that clones |
| `docs/github-setup.md` (new) | Creating the GitHub OAuth App, running the daemon with it |

---

### Task 1: Dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add crates**

Run:
```bash
cargo add reqwest --no-default-features --features json,rustls-tls
cargo add base64@0.22
cargo add clap --features derive,env
```
If `cargo add reqwest` rejects `rustls-tls`, run `cargo info reqwest` and use the rustls feature name it lists.

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: `Finished`.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add reqwest, base64 and clap env for GitHub sign-in"
```

---

### Task 2: `GITHUB_ERR` error code

**Files:**
- Modify: `src/error.rs` (enum `ErrorCode`, `as_str`, `status`, constructors, test `all_codes_match_spec`)
- Modify: `docs/superpowers/specs/2026-09-29-workbench-lovable-builder-design.md:73`
- Modify: `web/src/core/types.ts` (`ErrorCode` union and `ERROR_CODES`)

- [ ] **Step 1: Update the failing test first**

In `src/error.rs`, add `ErrorCode::GithubErr` to the list the test iterates and `"GITHUB_ERR"` to the expected array, after `"BADREQ"`.

- [ ] **Step 2: Run it**

Run: `cargo test --lib error::tests::all_codes_match_spec`
Expected: FAIL (compile error: no variant `GithubErr`).

- [ ] **Step 3: Implement**

```rust
// enum ErrorCode — add after Badreq:
    GithubErr,
// as_str:
            Self::GithubErr => "GITHUB_ERR",
// status (GitHub is an upstream, like the agent):
            Self::AgentErr | Self::DevserverErr | Self::GithubErr => 502,
// impl ApiError:
    pub fn github(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::GithubErr, message)
    }
```

In the spec doc line 73 add `` `GITHUB_ERR` `` to the code list. In `web/src/core/types.ts` add `"GITHUB_ERR"` to the `ErrorCode` union and to `ERROR_CODES`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib error && pnpm -C web test -- api`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/error.rs docs/superpowers/specs web/src/core/types.ts
git commit -m "feat: GITHUB_ERR error code"
```

---

### Task 3: Config — `GithubConfig` and `--github-client-id`

**Files:**
- Modify: `src/config.rs`
- Modify: `src/main.rs`
- Modify: the `config()` helper in `tests/checkpoints.rs`, `tests/devserver.rs`, `tests/health.rs`, `tests/preview.rs`, `tests/projects.rs`, `tests/static.rs`, `tests/watch.rs`, and the test in `src/config.rs`

- [ ] **Step 1: Add the type and field**

In `src/config.rs`:

```rust
/// Where GitHub lives. Overridable so tests can point at a fake server and a local bare repo.
#[derive(Debug, Clone)]
pub struct GithubConfig {
    /// OAuth App client id (device flow enabled). `None` disables sign-in.
    pub client_id: Option<String>,
    /// REST API base, e.g. `https://api.github.com`.
    pub api_url: String,
    /// Web base for `/login/device/code` and `/login/oauth/access_token`.
    pub web_url: String,
    /// Base for clone URLs: `{git_url}/{owner}/{repo}.git`.
    pub git_url: String,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            client_id: None,
            api_url: "https://api.github.com".into(),
            web_url: "https://github.com".into(),
            git_url: "https://github.com".into(),
        }
    }
}
```

Change `#[derive(Debug, Clone)] pub struct Config` to `#[derive(Debug, Clone, Default)]` and add the field:

```rust
    pub github: GithubConfig,
```

- [ ] **Step 2: Fix every `Config { … }` literal**

Each listed test helper (and the test in `src/config.rs`) ends its literal with `agents_config: None,`. Append `..Default::default()` on the next line, e.g. in `tests/projects.rs`:

```rust
fn config(dir: &Path, no_auth: bool) -> Config {
    Config {
        port: 0,
        origin: None,
        projects_dir: dir.to_path_buf(),
        no_auth,
        token: if no_auth { None } else { Some("abc123".into()) },
        agents_config: None,
        ..Default::default()
    }
}
```

Find them with: `grep -rn "agents_config: None" src tests`.

- [ ] **Step 3: CLI flag**

In `src/main.rs`, add to `Command::Serve`:

```rust
        /// GitHub OAuth App client id (enables "Sign in with GitHub").
        #[arg(long, env = "WORKBENCH_GITHUB_CLIENT_ID")]
        github_client_id: Option<String>,
```

destructure it in `main`, and set it on the config:

```rust
        github: workbench::config::GithubConfig {
            client_id: github_client_id,
            ..Default::default()
        },
```

(`Config { … }` in `main` keeps all its other fields; add this one.)

- [ ] **Step 4: Build and test**

Run: `cargo test`
Expected: all pass (no behavior change yet).

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/main.rs tests
git commit -m "feat: GitHub config and --github-client-id"
```

---

### Task 4: Per-repo commit identity

Today `src/git.rs::run` and `src/projects.rs::git` pass `-c user.name=Workbench -c user.email=workbench@localhost` on every call, which overrides any identity. Move identity into each repo's local config.

**Files:**
- Modify: `src/git.rs` (`run`, new `Identity`, `set_identity`, `has_identity`)
- Modify: `src/projects.rs` (`git` helper, `scaffold`, new `apply_identity`, `ensure_identities`)
- Modify: `src/lib.rs` (`serve_on`)
- Test: `tests/checkpoints.rs`, `tests/projects.rs`

- [ ] **Step 1: Write the failing tests**

Append to `tests/projects.rs`:

```rust
#[tokio::test]
async fn new_projects_carry_a_local_identity_and_it_can_be_replaced() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(config(tmp.path(), true), false);
    let info = state.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    assert_eq!(git(&dir, &["config", "--local", "user.name"]), "Workbench");

    let ada = workbench::git::Identity {
        name: "Ada Lovelace".into(),
        email: "42+ada@users.noreply.github.com".into(),
    };
    state.projects.apply_identity(&ada).await;
    assert_eq!(git(&dir, &["config", "--local", "user.name"]), "Ada Lovelace");
    assert_eq!(
        git(&dir, &["config", "--local", "user.email"]),
        "42+ada@users.noreply.github.com"
    );
}

#[tokio::test]
async fn startup_gives_older_projects_a_local_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(config(tmp.path(), true), false);
    let info = state.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    git(&dir, &["config", "--local", "--unset", "user.name"]);
    git(&dir, &["config", "--local", "--unset", "user.email"]);

    state
        .projects
        .ensure_identities(&workbench::git::Identity::workbench())
        .await;
    assert_eq!(git(&dir, &["config", "--local", "user.name"]), "Workbench");
}
```

In `tests/checkpoints.rs`, after the existing `assert_eq!(cp.author, "Workbench");`, add a second checkpoint under a new identity:

```rust
    workbench::git::set_identity(
        &dir,
        &workbench::git::Identity { name: "Ada".into(), email: "ada@example.com".into() },
    )
    .await
    .unwrap();
    std::fs::write(dir.join("src/ada.txt"), "hi\n").unwrap();
    let by_ada = git::checkpoint(&dir, "Ada's edit").await.unwrap().unwrap();
    assert_eq!(by_ada.author, "Ada");
```

(Place it right after the first `cp` assertions; later assertions in that test that count checkpoints must then expect one more — update `list.len()` accordingly.)

- [ ] **Step 2: Run them**

Run: `cargo test --test projects identity && cargo test --test checkpoints`
Expected: FAIL to compile (`Identity`, `apply_identity`, `ensure_identities`, `set_identity` missing).

- [ ] **Step 3: Implement in `src/git.rs`**

Add near the top:

```rust
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
```

In `run`, delete the four lines `"-c", "user.name=Workbench", "-c", "user.email=workbench@localhost",` and update its doc comment to say identity comes from the repo's local config.

- [ ] **Step 4: Implement in `src/projects.rs`**

In the private `git` helper delete the same four `-c user.*` lines and change its doc comment to "Run git in `dir` (identity comes from the repo's local config)".

In `scaffold`, set the identity between `init` and the first commit. Change `scaffold` to take the identity (Task 6 extends it again):

```rust
    async fn scaffold(&self, dir: &Path, identity: &Identity) -> Result<(), ApiError> {
        // … unchanged up to and including `npm_install` …
        git(dir, &["init", "-b", "main"]).await?;
        crate::git::set_identity(dir, identity).await?;
        git(dir, &["add", "-A"]).await?;
        // … unchanged commit …
    }
```

and in `create` call `self.scaffold(&dir, &Identity::workbench())`. Add `use crate::git::Identity;`.

Add to `impl ProjectRegistry`:

```rust
    fn dirs(inner: &Inner) -> Vec<PathBuf> {
        inner.projects.iter().map(|p| p.dir.clone()).collect()
    }

    /// Author every project's future commits as `identity` (sign-in / sign-out).
    pub async fn apply_identity(&self, identity: &Identity) {
        let dirs = Self::dirs(&*self.inner.lock().await);
        for dir in dirs {
            if let Err(e) = crate::git::set_identity(&dir, identity).await {
                eprintln!("workbench: cannot set git identity in {}: {e}", dir.display());
            }
        }
    }

    /// Daemon start: projects created before per-repo identities get `identity`.
    pub async fn ensure_identities(&self, identity: &Identity) {
        let dirs = Self::dirs(&*self.inner.lock().await);
        for dir in dirs {
            if !crate::git::has_identity(&dir).await {
                if let Err(e) = crate::git::set_identity(&dir, identity).await {
                    eprintln!("workbench: cannot set git identity in {}: {e}", dir.display());
                }
            }
        }
    }
```

- [ ] **Step 5: Migrate at startup**

In `src/lib.rs::serve_on`, before `state.watch_all().await;`:

```rust
    state
        .projects
        .ensure_identities(&crate::git::Identity::workbench())
        .await;
```

(Task 7 replaces `Identity::workbench()` with the signed-in identity.)

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: PASS. If a test builds a repo by hand with `git init` and then calls `git::checkpoint`, give it an identity first with `workbench::git::set_identity(&dir, &workbench::git::Identity::workbench()).await.unwrap();`.

- [ ] **Step 7: Commit**

```bash
git add src tests
git commit -m "feat: per-repo git identity instead of a forced Workbench author"
```

---

### Task 5: `github` module — device flow, session, API

**Files:**
- Create: `src/github.rs`
- Modify: `src/lib.rs` (add `pub mod github;`)

- [ ] **Step 1: Write the unit tests**

Create `src/github.rs` with only this test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_names_are_owner_slash_name() {
        assert_eq!(validate_repo("acme/site").unwrap(), "acme/site");
        assert_eq!(validate_repo(" Acme-Co/site.docs_2 ").unwrap(), "Acme-Co/site.docs_2");
        for bad in ["", "acme", "acme/", "/site", "a/b/c", "acme/..", "acme/si te", "-x/../y"] {
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
        assert_eq!(get("GIT_CONFIG_COUNT"), Some("2"));
        assert_eq!(get("GIT_CONFIG_KEY_0"), Some("http.https://github.com/.extraheader"));
        // base64("x-access-token:tok")
        assert_eq!(
            get("GIT_CONFIG_VALUE_0"),
            Some("AUTHORIZATION: basic eC1hY2Nlc3MtdG9rZW46dG9r")
        );
        assert_eq!(get("GIT_CONFIG_KEY_1"), Some("credential.helper"));
        assert_eq!(get("GIT_CONFIG_VALUE_1"), Some(""));
    }

    #[test]
    fn identity_prefers_the_profile_name_and_uses_the_noreply_email() {
        let u = GithubUser { login: "ada".into(), id: 42, name: Some("Ada Lovelace".into()), avatar_url: String::new() };
        assert_eq!(u.identity().name, "Ada Lovelace");
        assert_eq!(u.identity().email, "42+ada@users.noreply.github.com");
        let u = GithubUser { name: Some("  ".into()), ..u };
        assert_eq!(u.identity().name, "ada");
    }
}
```

Add `pub mod github;` to `src/lib.rs`.

- [ ] **Step 2: Run them**

Run: `cargo test --lib github`
Expected: FAIL to compile (functions/types missing).

- [ ] **Step 3: Implement the module**

Put this above the test module in `src/github.rs`:

```rust
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
            user: self.inner.lock().await.session.as_ref().map(|s| s.user.clone()),
        }
    }

    pub async fn token(&self) -> Option<String> {
        self.inner.lock().await.session.as_ref().map(|s| s.token.clone())
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
                let user = self.fetch_user(&token).await?;
                self.save(Session {
                    token,
                    user: user.clone(),
                })
                .await?;
                Ok(PollOutcome::SignedIn { user })
            }
            (None, Some("authorization_pending")) => Ok(PollOutcome::Pending),
            (None, Some("slow_down")) => Ok(PollOutcome::SlowDown {
                interval: tok.interval.unwrap_or(10),
            }),
            (None, Some("expired_token")) => {
                self.inner.lock().await.pending = None;
                Ok(PollOutcome::Expired)
            }
            (None, Some("access_denied")) => {
                self.inner.lock().await.pending = None;
                Ok(PollOutcome::Denied)
            }
            (None, other) => Err(ApiError::github(format!(
                "GitHub sign-in failed: {}",
                tok.error_description
                    .or(other.map(str::to_string))
                    .unwrap_or_else(|| "no token in the response".into())
            ))),
        }
    }

    pub async fn sign_out(&self) {
        let mut inner = self.inner.lock().await;
        inner.session = None;
        inner.pending = None;
        let _ = std::fs::remove_file(&self.path);
    }

    /// Repositories the user can access, most recently pushed first (first 100).
    pub async fn repos(&self) -> Result<Vec<RepoInfo>, ApiError> {
        let token = self.token().await.ok_or_else(not_signed_in)?;
        let res = self
            .http
            .get(format!("{}/user/repos?per_page=100&sort=pushed", self.api()))
            .bearer_auth(&token)
            .header(ACCEPT, GITHUB_JSON)
            .send()
            .await
            .map_err(unreachable)?;
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

    async fn save(&self, session: Session) -> Result<(), ApiError> {
        let bytes = serde_json::to_vec_pretty(&session)
            .map_err(|e| ApiError::io(format!("serialize GitHub session: {e}")))?;
        write_private(&self.path, &bytes)?;
        let mut inner = self.inner.lock().await;
        inner.session = Some(session);
        inner.pending = None;
        Ok(())
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
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
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
        return Err(ApiError::github(format!("GitHub returned {status}: {text}")));
    }
    serde_json::from_slice(&body)
        .map_err(|e| ApiError::github(format!("unexpected response from GitHub: {e}")))
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
    if ok {
        Ok(s.to_string())
    } else {
        Err(ApiError::badreq(format!("not a GitHub repository (owner/name): {s:?}")))
    }
}

fn valid_part(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 100
        && !p.starts_with('.')
        && !p.starts_with('-')
        && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
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
    vec![
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_CONFIG_COUNT".into(), "2".into()),
        ("GIT_CONFIG_KEY_0".into(), format!("http.{base}/.extraheader")),
        ("GIT_CONFIG_VALUE_0".into(), format!("AUTHORIZATION: basic {basic}")),
        ("GIT_CONFIG_KEY_1".into(), "credential.helper".into()),
        ("GIT_CONFIG_VALUE_1".into(), String::new()),
    ]
}
```

- [ ] **Step 4: Run the unit tests**

Run: `cargo test --lib github`
Expected: 4 passed. (`"-x/../y"` fails `split_once` validation because `-x` starts with `-`.)

- [ ] **Step 5: Commit**

```bash
git add src/github.rs src/lib.rs
git commit -m "feat: GitHub device-flow sign-in, session storage and repo listing"
```

---

### Task 6: Clone a repository as a project

**Files:**
- Modify: `src/projects.rs` (`Source`, `Project.repo`, `ProjectInfo.repo`, `StoredProject.repo`, `create_from`, `scaffold`, `git_env`)
- Modify: `web/src/core/types.ts` (`Project.repo`)

- [ ] **Step 1: Add `repo` to the project records**

In `src/projects.rs`:

```rust
// Project:
    /// `owner/name` when the project was opened from GitHub.
    pub repo: Option<String>,
// ProjectInfo (serialized as `repo`, null for template projects):
    pub repo: Option<String>,
// From<&Project> for ProjectInfo:
            repo: p.repo.clone(),
// StoredProject:
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repo: Option<String>,
```

In `ProjectRegistry::open`, in the `.map(|s| Project { … })` closure add `repo: s.repo,`. In `persist`, add `repo: p.repo.clone(),` to the `StoredProject` literal. In `create`'s `Project { … }` literal add `repo: None,` for now. Update the `ProjectInfo` doc comment to `{ id, name, port, devserver, autoApprove, repo }`.

In `web/src/core/types.ts`, add to `interface Project`:

```ts
  /** `owner/name` when opened from GitHub; null for template projects. */
  repo: string | null;
```

- [ ] **Step 2: Introduce `Source` and `create_from`**

```rust
/// Where a new project's files come from.
pub enum Source {
    /// The embedded astro-marketing template, committed as the first checkpoint.
    Template,
    /// `git clone` of `url`; `env` authenticates git (see [`crate::github::git_auth_env`]).
    Clone {
        repo: String,
        url: String,
        env: Vec<(String, String)>,
    },
}
```

Rename the body of `create` to `pub async fn create_from(&self, name: &str, source: Source, identity: &Identity) -> Result<ProjectInfo, ApiError>` and keep `create` as the template shortcut the tests use:

```rust
    /// New project from the template, authored as [`Identity::workbench`] (tests, defaults).
    pub async fn create(&self, name: &str) -> Result<ProjectInfo, ApiError> {
        self.create_from(name, Source::Template, &Identity::workbench())
            .await
    }
```

Inside `create_from`: before scaffolding, `let repo = match &source { Source::Clone { repo, .. } => Some(repo.clone()), Source::Template => None };`, call `self.scaffold(&dir, &source, identity)`, and set `repo,` in the `Project { … }` literal.

- [ ] **Step 3: Teach `scaffold` to clone**

```rust
    async fn scaffold(&self, dir: &Path, source: &Source, identity: &Identity) -> Result<(), ApiError> {
        // `create_dir` (not `_all`) fails if something raced us into existence.
        tokio::fs::create_dir(dir).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                ApiError::conflict("project directory already exists")
            } else {
                e.into()
            }
        })?;

        match source {
            Source::Template => {
                let dest = dir.to_path_buf();
                tokio::task::spawn_blocking(move || static_files::extract_template(&dest))
                    .await
                    .map_err(|e| ApiError::io(format!("template copy task failed: {e}")))??;
                if self.install {
                    npm_install(dir).await?;
                }
                git(dir, &["init", "-b", "main"]).await?;
                crate::git::set_identity(dir, identity).await?;
                git(dir, &["add", "-A"]).await?;
                git(
                    dir,
                    &["commit", "-m", "Initial commit from astro-marketing template"],
                )
                .await?;
            }
            Source::Clone { url, env, .. } => {
                // Into the (empty) directory we just created.
                git_env(dir, &["clone", "--", url, "."], env).await?;
                crate::git::set_identity(dir, identity).await?;
                if self.install {
                    npm_install(dir).await?;
                }
            }
        }
        Ok(())
    }
```

Split the `git` helper so clone can carry the auth environment:

```rust
/// Run git in `dir` (identity comes from the repo's local config).
async fn git(dir: &Path, args: &[&str]) -> Result<(), ApiError> {
    git_env(dir, args, &[]).await
}

/// [`git`] with extra environment (e.g. GitHub auth for `clone`).
async fn git_env(dir: &Path, args: &[&str], env: &[(String, String)]) -> Result<(), ApiError> {
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
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
            tail(&String::from_utf8_lossy(&out.stderr))
        )));
    }
    Ok(())
}
```

- [ ] **Step 4: Build and run existing tests**

Run: `cargo test && pnpm -C web exec vue-tsc -b`
Expected: PASS (behavior for template projects unchanged; `repo` is `null` in API responses).

- [ ] **Step 5: Commit**

```bash
git add src/projects.rs web/src/core/types.ts
git commit -m "feat: projects can be cloned from a git URL and remember their GitHub repo"
```

---

### Task 7: HTTP endpoints and state wiring

**Files:**
- Modify: `src/state.rs` (`github: Arc<GithubAuth>`)
- Create: `src/http/github.rs`
- Modify: `src/http/mod.rs` (routes)
- Modify: `src/http/projects.rs` (`create` uses the signed-in identity; new `clone`)
- Modify: `src/lib.rs` (startup identity)
- Test: `tests/github.rs` (new)

- [ ] **Step 1: Write the end-to-end test**

Create `tests/github.rs`:

```rust
//! Sign in through a fake GitHub, list repos, open one (cloned from a local bare repo),
//! check authorship, survive a restart, sign out.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use workbench::config::{Config, GithubConfig};
use workbench::http;
use workbench::state::AppState;

async fn fake_github() -> String {
    let polls = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route(
            "/login/device/code",
            post(|body: String| async move {
                assert!(body.contains("client_id=cid"), "{body}");
                Json(json!({
                    "device_code": "dev123", "user_code": "ABCD-1234",
                    "verification_uri": "https://github.com/login/device",
                    "expires_in": 900, "interval": 5
                }))
            }),
        )
        .route(
            "/login/oauth/access_token",
            post(move |body: String| {
                let polls = polls.clone();
                async move {
                    assert!(body.contains("device_code=dev123"), "{body}");
                    if polls.fetch_add(1, Ordering::SeqCst) == 0 {
                        Json(json!({ "error": "authorization_pending" }))
                    } else {
                        Json(json!({ "access_token": "tok", "token_type": "bearer" }))
                    }
                }
            }),
        )
        .route(
            "/user",
            get(|headers: HeaderMap| async move {
                let authed = headers.get(header::AUTHORIZATION).is_some_and(|v| v == "Bearer tok");
                if !authed {
                    return (StatusCode::UNAUTHORIZED, Json(json!({ "message": "Bad credentials" })));
                }
                (StatusCode::OK, Json(json!({
                    "login": "ada", "id": 42, "name": "Ada Lovelace",
                    "avatar_url": "https://avatars.example/ada.png"
                })))
            }),
        )
        .route(
            "/user/repos",
            get(|| async {
                Json(json!([{
                    "full_name": "acme/site", "private": true,
                    "default_branch": "main", "permissions": { "push": true }
                }]))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// `{root}/acme/site.git`: a bare repo with one commit.
fn bare_repo(root: &Path) {
    let src = root.join("src-repo");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("package.json"), "{\"name\":\"site\"}\n").unwrap();
    git(&src, &["init", "-b", "main"]);
    git(&src, &["-c", "user.name=Grace", "-c", "user.email=g@example.com", "add", "-A"]);
    git(&src, &["-c", "user.name=Grace", "-c", "user.email=g@example.com", "commit", "-m", "init"]);
    std::fs::create_dir_all(root.join("acme")).unwrap();
    git(root, &["clone", "--bare", src.to_str().unwrap(), "acme/site.git"]);
}

fn config(projects: &Path, base: &str, git_root: &Path) -> Config {
    Config {
        projects_dir: projects.to_path_buf(),
        no_auth: true,
        github: GithubConfig {
            client_id: Some("cid".into()),
            api_url: base.into(),
            web_url: base.into(),
            git_url: format!("file://{}", git_root.display()),
        },
        ..Default::default()
    }
}

fn req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let b = Request::builder().method(method).uri(uri).header(header::HOST, "localhost");
    match body {
        Some(v) => b.header(header::CONTENT_TYPE, "application/json").body(Body::from(v.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    }
}

async fn call(app: &Router, r: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(r).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn sign_in_open_repo_and_author_commits() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let git_root = tmp.path().join("git");
    bare_repo(&git_root);
    let base = fake_github().await;
    let app = http::router(AppState::with_scaffold_install(config(&projects, &base, &git_root), false));

    let (s, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, json!({ "configured": true, "user": null }));

    // Opening a repo needs a session.
    let (s, body) = call(&app, req("POST", "/api/projects/clone", Some(json!({ "repo": "acme/site" })))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "GITHUB_ERR");

    let (s, body) = call(&app, req("POST", "/api/github/device", None)).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["userCode"], "ABCD-1234");
    assert_eq!(body["interval"], 5);

    let (_, body) = call(&app, req("POST", "/api/github/device/poll", None)).await;
    assert_eq!(body, json!({ "status": "pending" }));
    let (_, body) = call(&app, req("POST", "/api/github/device/poll", None)).await;
    assert_eq!(body["status"], "signedIn");
    assert_eq!(body["user"]["login"], "ada");
    assert_eq!(body["user"]["avatarUrl"], "https://avatars.example/ada.png");

    let session = projects.join(".workbench/github.json");
    let mode = std::fs::metadata(&session).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    let (_, body) = call(&app, req("GET", "/api/github/repos", None)).await;
    assert_eq!(body["repos"], json!([{
        "fullName": "acme/site", "private": true, "defaultBranch": "main", "canPush": true
    }]));

    let (s, info) = call(&app, req("POST", "/api/projects/clone", Some(json!({ "repo": "acme/site" })))).await;
    assert_eq!(s, StatusCode::CREATED, "{info}");
    assert_eq!(info["id"], "site");
    assert_eq!(info["repo"], "acme/site");
    let dir = projects.join("site");
    assert!(dir.join("package.json").exists());
    assert_eq!(git(&dir, &["config", "--local", "user.name"]), "Ada Lovelace");
    assert_eq!(git(&dir, &["config", "--local", "user.email"]), "42+ada@users.noreply.github.com");
    assert!(git(&dir, &["remote", "get-url", "origin"]).ends_with("acme/site.git"));
    // The token was only in the environment of the clone.
    assert!(!std::fs::read_to_string(dir.join(".git/config")).unwrap().contains("extraheader"));

    // New template projects are authored by the signed-in user too.
    let (_, made) = call(&app, req("POST", "/api/projects", Some(json!({ "name": "Fresh" })))).await;
    let fresh = projects.join(made["id"].as_str().unwrap());
    assert_eq!(git(&fresh, &["log", "-1", "--format=%an"]), "Ada Lovelace");

    // The session survives a daemon restart.
    let again = http::router(AppState::with_scaffold_install(config(&projects, &base, &git_root), false));
    let (_, body) = call(&again, req("GET", "/api/github/status", None)).await;
    assert_eq!(body["user"]["login"], "ada");

    let (s, _) = call(&again, req("POST", "/api/github/signout", None)).await;
    assert_eq!(s, StatusCode::OK);
    let (_, body) = call(&again, req("GET", "/api/github/status", None)).await;
    assert_eq!(body["user"], Value::Null);
    assert!(!session.exists());
    assert_eq!(git(&dir, &["config", "--local", "user.name"]), "Workbench");
}

#[tokio::test]
async fn sign_in_is_unavailable_without_a_client_id() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = Config { projects_dir: tmp.path().to_path_buf(), no_auth: true, ..Default::default() };
    let app = http::router(AppState::with_scaffold_install(cfg, false));
    let (_, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(body, json!({ "configured": false, "user": null }));
    let (s, body) = call(&app, req("POST", "/api/github/device", None)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "GITHUB_ERR");
}
```

- [ ] **Step 2: Run it**

Run: `cargo test --test github`
Expected: FAIL (404s / compile errors: no `/api/github/*`, no `clone`).

- [ ] **Step 3: State**

In `src/state.rs` add `use crate::github::GithubAuth;`, the field

```rust
    /// GitHub session (device-flow sign-in) and API access.
    pub github: Arc<GithubAuth>,
```

and in `with_options`, before `let config = Arc::new(config);`:

```rust
        let github = Arc::new(GithubAuth::open(
            config.github.clone(),
            &config.projects_dir.join(crate::projects::STATE_DIR),
        ));
```

then add `github,` to the `Self { … }` literal.

- [ ] **Step 4: GitHub handlers**

Create `src/http/github.rs`:

```rust
//! `/api/github/*`: sign-in status, device flow, sign-out, repositories.

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::git::Identity;
use crate::github::{DeviceStart, PollOutcome, Status};
use crate::state::AppState;

/// `GET /api/github/status` → `{ configured, user | null }`.
pub async fn status(State(state): State<AppState>) -> Json<Status> {
    Json(state.github.status().await)
}

/// `POST /api/github/device` → `{ userCode, verificationUri, expiresIn, interval }`.
pub async fn device_start(State(state): State<AppState>) -> Result<Json<DeviceStart>, ApiError> {
    Ok(Json(state.github.start_device().await?))
}

/// `POST /api/github/device/poll` → `{ status, user?, interval? }`. On success every
/// project's future commits are authored by the new user.
pub async fn device_poll(State(state): State<AppState>) -> Result<Json<PollOutcome>, ApiError> {
    let outcome = state.github.poll_device().await?;
    if let PollOutcome::SignedIn { user } = &outcome {
        state.projects.apply_identity(&user.identity()).await;
    }
    Ok(Json(outcome))
}

/// `POST /api/github/signout`. Projects go back to the `Workbench` identity.
pub async fn sign_out(State(state): State<AppState>) -> Json<Value> {
    state.github.sign_out().await;
    state.projects.apply_identity(&Identity::workbench()).await;
    Json(json!({ "signedOut": true }))
}

/// `GET /api/github/repos` → `{ repos: [{ fullName, private, defaultBranch, canPush }] }`.
pub async fn repos(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({ "repos": state.github.repos().await? })))
}
```

In `src/http/mod.rs` add `pub mod github;` and these routes next to `/api/projects`:

```rust
        .route("/api/projects/clone", post(projects::clone))
        .route("/api/github/status", get(github::status))
        .route("/api/github/device", post(github::device_start))
        .route("/api/github/device/poll", post(github::device_poll))
        .route("/api/github/signout", post(github::sign_out))
        .route("/api/github/repos", get(github::repos))
```

(`/api/projects/clone` must be registered so it doesn't collide with `/api/projects/{id}`; axum prefers the static segment.)

- [ ] **Step 5: Project handlers**

In `src/http/projects.rs`, make `create` use the signed-in identity and share its tail with `clone`:

```rust
use crate::github;
use crate::projects::{ProjectInfo, Source};

pub async fn create(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<(StatusCode, Json<ProjectInfo>), ApiError> {
    let req: CreateRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::badreq(format!("expected JSON body {{\"name\": string}}: {e}")))?;
    let identity = state.github.identity().await;
    let info = state
        .projects
        .create_from(&req.name, Source::Template, &identity)
        .await?;
    Ok((StatusCode::CREATED, Json(started(&state, info).await)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CloneRequest {
    repo: String,
    name: Option<String>,
}

/// `POST /api/projects/clone` `{ repo: "owner/name", name? }` → the new project (201).
pub async fn clone(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<(StatusCode, Json<ProjectInfo>), ApiError> {
    let req: CloneRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!("expected JSON body {{\"repo\": \"owner/name\", \"name\"?: string}}: {e}"))
    })?;
    let repo = github::validate_repo(&req.repo)?;
    let env = state.github.git_env().await?;
    let name = req
        .name
        .unwrap_or_else(|| repo.rsplit('/').next().unwrap_or(&repo).to_string());
    let source = Source::Clone {
        url: state.github.clone_url(&repo),
        repo,
        env,
    };
    let identity = state.github.identity().await;
    let info = state.projects.create_from(&name, source, &identity).await?;
    Ok((StatusCode::CREATED, Json(started(&state, info).await)))
}

/// Watch a freshly created project and supervise its dev server; return its current info.
async fn started(state: &AppState, info: ProjectInfo) -> ProjectInfo {
    state.watch_project(&info.id).await;
    // `start` marks the project `crashed` (and publishes it) if it cannot supervise, so
    // the response reflects the real state either way; the restart endpoint retries.
    if let Err(e) = state.devservers.start(&info.id).await {
        eprintln!("workbench: cannot start dev server for {}: {e}", info.id);
    }
    match state.projects.get(&info.id).await {
        Some(p) => ProjectInfo::from(&p),
        None => info,
    }
}
```

(Delete the old body of `create`; `started` replaces its tail.)

- [ ] **Step 6: Startup identity**

In `src/lib.rs::serve_on`, replace the Task 4 line with:

```rust
    let identity = state.github.identity().await;
    state.projects.ensure_identities(&identity).await;
```

- [ ] **Step 7: Run tests**

Run: `cargo test`
Expected: PASS, including both tests in `tests/github.rs`.

- [ ] **Step 8: Commit**

```bash
git add src tests/github.rs
git commit -m "feat: GitHub sign-in endpoints and Open from GitHub (clone)"
```

---

### Task 8: Web API client and GitHub store

**Files:**
- Modify: `web/src/core/types.ts`
- Modify: `web/src/core/api.ts`
- Create: `web/src/modules/github/store.ts`
- Test: `web/src/modules/github/store.test.ts`
- Modify: `web/src/modules/projects/store.ts` (`cloneRepo`)

- [ ] **Step 1: Types**

Append to `web/src/core/types.ts`:

```ts
export interface GithubUser {
  login: string;
  id: number;
  name: string | null;
  avatarUrl: string;
}

export interface GithubStatus {
  /** The daemon was started with a GitHub OAuth client id. */
  configured: boolean;
  user: GithubUser | null;
}

export interface DeviceStart {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  /** Seconds between polls. */
  interval: number;
}

export type DevicePoll =
  | { status: "pending" }
  | { status: "slowDown"; interval: number }
  | { status: "signedIn"; user: GithubUser }
  | { status: "expired" }
  | { status: "denied" };

export interface GithubRepo {
  fullName: string;
  private: boolean;
  defaultBranch: string;
  canPush: boolean;
}
```

- [ ] **Step 2: API calls**

In `web/src/core/api.ts` import the new types and add to the object `createApi` returns, after `deleteProject`:

```ts
    cloneProject: (repo: string, name?: string) =>
      json<Project>("POST", "/api/projects/clone", { body: name ? { repo, name } : { repo } }),
    githubStatus: () => json<GithubStatus>("GET", "/api/github/status"),
    githubDeviceStart: () => json<DeviceStart>("POST", "/api/github/device"),
    githubDevicePoll: () => json<DevicePoll>("POST", "/api/github/device/poll"),
    githubSignOut: () => json<{ signedOut: true }>("POST", "/api/github/signout"),
    githubRepos: async (): Promise<GithubRepo[]> =>
      (await json<{ repos: GithubRepo[] }>("GET", "/api/github/repos")).repos,
```

- [ ] **Step 3: Write the store test**

Create `web/src/modules/github/store.test.ts`:

```ts
// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useGithubStore } from "./store";

const ada = { login: "ada", id: 42, name: "Ada", avatarUrl: "a.png" };

function setup(api: Record<string, unknown>) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return useGithubStore();
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("github store", () => {
  it("loads status", async () => {
    const gh = setup({ githubStatus: vi.fn(async () => ({ configured: true, user: ada })) });
    await gh.load();
    expect(gh.configured).toBe(true);
    expect(gh.user).toEqual(ada);
  });

  it("polls the device flow at the given interval until signed in", async () => {
    const poll = vi
      .fn()
      .mockResolvedValueOnce({ status: "pending" })
      .mockResolvedValueOnce({ status: "slowDown", interval: 10 })
      .mockResolvedValueOnce({ status: "signedIn", user: ada });
    const gh = setup({
      githubDeviceStart: vi.fn(async () => ({
        userCode: "ABCD-1234", verificationUri: "https://github.com/login/device",
        expiresIn: 900, interval: 5,
      })),
      githubDevicePoll: poll,
    });
    await gh.signIn();
    expect(gh.device?.userCode).toBe("ABCD-1234");

    await vi.advanceTimersByTimeAsync(4999);
    expect(poll).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1); // 5s: pending
    expect(poll).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(5000); // 10s: slowDown → next in 10s
    expect(poll).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(9999);
    expect(poll).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1);
    await flushPromises();
    expect(poll).toHaveBeenCalledTimes(3);
    expect(gh.user).toEqual(ada);
    expect(gh.device).toBeNull();
  });

  it("reports an expired code and stops polling", async () => {
    const poll = vi.fn(async () => ({ status: "expired" }));
    const gh = setup({
      githubDeviceStart: vi.fn(async () => ({ userCode: "X", verificationUri: "u", expiresIn: 1, interval: 1 })),
      githubDevicePoll: poll,
    });
    await gh.signIn();
    await vi.advanceTimersByTimeAsync(5000);
    expect(poll).toHaveBeenCalledTimes(1);
    expect(gh.device).toBeNull();
    expect(gh.error).toMatch(/expired/);
  });

  it("cancel stops polling", async () => {
    const poll = vi.fn(async () => ({ status: "pending" }));
    const gh = setup({
      githubDeviceStart: vi.fn(async () => ({ userCode: "X", verificationUri: "u", expiresIn: 900, interval: 1 })),
      githubDevicePoll: poll,
    });
    await gh.signIn();
    gh.cancel();
    await vi.advanceTimersByTimeAsync(5000);
    expect(poll).not.toHaveBeenCalled();
  });

  it("signs out", async () => {
    const signOut = vi.fn(async () => ({ signedOut: true }));
    const gh = setup({ githubSignOut: signOut });
    gh.user = ada;
    await gh.signOut();
    expect(signOut).toHaveBeenCalled();
    expect(gh.user).toBeNull();
  });
});
```

- [ ] **Step 4: Run it**

Run: `pnpm -C web exec vitest run src/modules/github/store.test.ts`
Expected: FAIL (`./store` not found).

- [ ] **Step 5: Implement the store**

Create `web/src/modules/github/store.ts`:

```ts
import { defineStore } from "pinia";
import { ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { DeviceStart, GithubRepo, GithubUser } from "@/core/types";
import { errorMessage } from "@/lib/errors";

export const useGithubStore = defineStore("github", () => {
  const daemon = useDaemon();
  const configured = ref(false);
  const user = ref<GithubUser | null>(null);
  /** The code being shown while the user approves it on github.com. */
  const device = ref<DeviceStart | null>(null);
  const error = ref<string | null>(null);
  const repos = ref<GithubRepo[]>([]);
  const reposLoading = ref(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  async function load() {
    const s = await daemon.api.githubStatus();
    configured.value = s.configured;
    user.value = s.user;
  }

  function stopTimer() {
    if (timer) clearTimeout(timer);
    timer = null;
  }

  function schedule(seconds: number) {
    stopTimer();
    timer = setTimeout(() => void poll(seconds), seconds * 1000);
  }

  async function signIn() {
    cancel();
    error.value = null;
    try {
      device.value = await daemon.api.githubDeviceStart();
      schedule(device.value.interval);
    } catch (e) {
      error.value = errorMessage(e);
    }
  }

  async function poll(interval: number) {
    timer = null;
    if (!device.value) return;
    try {
      const r = await daemon.api.githubDevicePoll();
      if (!device.value) return; // cancelled while the request was in flight
      switch (r.status) {
        case "pending":
          schedule(interval);
          break;
        case "slowDown":
          schedule(r.interval);
          break;
        case "signedIn":
          user.value = r.user;
          device.value = null;
          break;
        case "expired":
          error.value = "The sign-in code expired. Try again.";
          device.value = null;
          break;
        case "denied":
          error.value = "Sign-in was cancelled on GitHub.";
          device.value = null;
          break;
      }
    } catch (e) {
      error.value = errorMessage(e);
      device.value = null;
    }
  }

  function cancel() {
    stopTimer();
    device.value = null;
  }

  async function signOut() {
    await daemon.api.githubSignOut();
    user.value = null;
    repos.value = [];
  }

  async function loadRepos() {
    reposLoading.value = true;
    try {
      repos.value = await daemon.api.githubRepos();
    } finally {
      reposLoading.value = false;
    }
  }

  return { configured, user, device, error, repos, reposLoading, load, signIn, cancel, signOut, loadRepos };
});
```

- [ ] **Step 6: `cloneRepo` in the projects store**

In `web/src/modules/projects/store.ts`, after `create`:

```ts
  /** Open a GitHub repository as a project (the daemon clones it) and select it. */
  async function cloneRepo(repo: string): Promise<Project> {
    creating.value = true;
    try {
      const project = await daemon.api.cloneProject(repo);
      projects.value = [...projects.value.filter((p) => p.id !== project.id), project];
      select(project.id);
      return project;
    } finally {
      creating.value = false;
    }
  }
```

and add `cloneRepo` to the returned object.

- [ ] **Step 7: Run tests and type-check**

Run: `pnpm -C web exec vitest run src/modules/github && pnpm -C web exec vue-tsc -b`
Expected: 5 passed; no type errors.

- [ ] **Step 8: Commit**

```bash
git add web/src/core web/src/modules/github web/src/modules/projects/store.ts
git commit -m "feat(web): GitHub API client and sign-in store"
```

---

### Task 9: Header account control and device-code dialog

**Files:**
- Create: `web/src/modules/github/DeviceCodeDialog.vue`
- Create: `web/src/modules/github/GithubAccount.vue`
- Modify: `web/src/app/Shell.vue` (an `account` slot)
- Modify: `web/src/App.vue`
- Test: `web/src/modules/github/GithubAccount.test.ts`

- [ ] **Step 1: Write the component test**

Create `web/src/modules/github/GithubAccount.test.ts`:

```ts
// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import GithubAccount from "./GithubAccount.vue";

function mountWith(status: unknown, api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({ githubStatus: vi.fn(async () => status), ...api });
  setActivePinia(createPinia());
  return mount(GithubAccount, { attachTo: document.body });
}

describe("GithubAccount", () => {
  it("renders nothing when sign-in is not configured", async () => {
    const w = mountWith({ configured: false, user: null });
    await flushPromises();
    expect(w.find("[data-testid='github-sign-in']").exists()).toBe(false);
    w.unmount();
  });

  it("starts the device flow and shows the code", async () => {
    const start = vi.fn(async () => ({
      userCode: "ABCD-1234", verificationUri: "https://github.com/login/device", expiresIn: 900, interval: 5,
    }));
    const w = mountWith({ configured: true, user: null }, { githubDeviceStart: start, githubDevicePoll: vi.fn() });
    await flushPromises();
    await w.find("[data-testid='github-sign-in']").trigger("click");
    await flushPromises();
    expect(start).toHaveBeenCalled();
    expect(document.body.querySelector("[data-testid='device-code']")?.textContent?.trim()).toBe("ABCD-1234");
    w.unmount();
  });

  it("shows the signed-in account", async () => {
    const w = mountWith({ configured: true, user: { login: "ada", id: 1, name: "Ada", avatarUrl: "a.png" } });
    await flushPromises();
    expect(w.find("[data-testid='github-account']").text()).toContain("Ada");
    w.unmount();
  });
});
```

- [ ] **Step 2: Run it**

Run: `pnpm -C web exec vitest run src/modules/github/GithubAccount.test.ts`
Expected: FAIL (component missing).

- [ ] **Step 3: Device-code dialog**

Create `web/src/modules/github/DeviceCodeDialog.vue`:

```vue
<script setup lang="ts">
import { Loader2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useGithubStore } from "./store";

const gh = useGithubStore();

function copyCode() {
  if (gh.device) void navigator.clipboard?.writeText(gh.device.userCode).catch(() => {});
}
</script>

<template>
  <Dialog :open="!!gh.device" @update:open="(open) => !open && gh.cancel()">
    <DialogContent class="sm:max-w-sm">
      <DialogHeader>
        <DialogTitle>Sign in with GitHub</DialogTitle>
        <DialogDescription>
          Enter this code on GitHub to connect your account. This window closes once you approve.
        </DialogDescription>
      </DialogHeader>
      <div class="flex flex-col items-center gap-3 py-2">
        <code class="text-2xl font-semibold tracking-widest" data-testid="device-code">
          {{ gh.device?.userCode }}
        </code>
        <Button as="a" :href="gh.device?.verificationUri" target="_blank" rel="noopener" @click="copyCode">
          Copy code and open GitHub
        </Button>
        <p class="text-muted-foreground flex items-center gap-2 text-xs">
          <Loader2 class="size-3 animate-spin" /> Waiting for approval…
        </p>
      </div>
    </DialogContent>
  </Dialog>
</template>
```

- [ ] **Step 4: Account control**

Create `web/src/modules/github/GithubAccount.vue`:

```vue
<script setup lang="ts">
import { onMounted, watch } from "vue";
import { LogIn, LogOut } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { errorMessage } from "@/lib/errors";
import DeviceCodeDialog from "./DeviceCodeDialog.vue";
import { useGithubStore } from "./store";

const gh = useGithubStore();

onMounted(() => {
  gh.load().catch(() => {
    // Older daemons have no /api/github: keep the control hidden.
  });
});

watch(
  () => gh.error,
  (e) => {
    if (e) toast.error(e);
  },
);

async function signOut() {
  try {
    await gh.signOut();
  } catch (e) {
    toast.error(`Sign-out failed: ${errorMessage(e)}`);
  }
}
</script>

<template>
  <template v-if="gh.configured">
    <DropdownMenu v-if="gh.user">
      <DropdownMenuTrigger as-child>
        <Button variant="ghost" size="sm" class="gap-1.5" data-testid="github-account">
          <img :src="gh.user.avatarUrl" alt="" class="size-5 rounded-full" />
          {{ gh.user.name || gh.user.login }}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem @select="signOut"><LogOut /> Sign out</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
    <Button v-else variant="outline" size="sm" data-testid="github-sign-in" @click="gh.signIn()">
      <LogIn /> Sign in with GitHub
    </Button>
    <DeviceCodeDialog />
  </template>
</template>
```

- [ ] **Step 5: Put it in the header**

In `web/src/app/Shell.vue`, inside `<div class="flex items-center gap-2 pr-1">` and before the status `<span>`, add:

```vue
        <slot name="account" />
```

In `web/src/App.vue`, import `GithubAccount from "@/modules/github/GithubAccount.vue"` and add inside `<Shell>`:

```vue
    <template #account>
      <GithubAccount />
    </template>
```

- [ ] **Step 6: Run tests and type-check**

Run: `pnpm -C web exec vitest run src/modules/github && pnpm -C web exec vue-tsc -b`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add web/src/modules/github web/src/app/Shell.vue web/src/App.vue
git commit -m "feat(web): Sign in with GitHub in the header"
```

---

### Task 10: "Open from GitHub" dialog

**Files:**
- Create: `web/src/modules/projects/OpenRepoDialog.vue`
- Modify: `web/src/modules/projects/ProjectsGrid.vue`
- Test: `web/src/modules/projects/OpenRepoDialog.test.ts`

- [ ] **Step 1: Write the test**

Create `web/src/modules/projects/OpenRepoDialog.test.ts`:

```ts
// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useGithubStore } from "@/modules/github/store";
import OpenRepoDialog from "./OpenRepoDialog.vue";

const repos = [
  { fullName: "acme/site", private: true, defaultBranch: "main", canPush: true },
  { fullName: "acme/docs", private: false, defaultBranch: "main", canPush: false },
];

describe("OpenRepoDialog", () => {
  it("lists, filters and opens a repository", async () => {
    const cloneProject = vi.fn(async () => ({
      id: "site", name: "site", port: 4000, devserver: "starting", autoApprove: false, repo: "acme/site",
    }));
    h.daemon = createFakeDaemon({ githubRepos: vi.fn(async () => repos), cloneProject });
    setActivePinia(createPinia());
    useGithubStore().user = { login: "ada", id: 1, name: "Ada", avatarUrl: "" };

    const w = mount(OpenRepoDialog, { props: { open: false }, attachTo: document.body });
    await w.setProps({ open: true });
    await flushPromises();
    const items = () => [...document.body.querySelectorAll<HTMLElement>("[data-testid='repo-item']")];
    expect(items().map((i) => i.textContent)).toEqual([
      expect.stringContaining("acme/site"),
      expect.stringContaining("acme/docs"),
    ]);

    const filter = document.body.querySelector<HTMLInputElement>("[aria-label='Filter repositories']")!;
    filter.value = "site";
    filter.dispatchEvent(new Event("input"));
    await flushPromises();
    expect(items()).toHaveLength(1);

    items()[0]!.click();
    await flushPromises();
    expect(cloneProject).toHaveBeenCalledWith("acme/site");
    expect(w.emitted("update:open")?.at(-1)).toEqual([false]);
    w.unmount();
  });
});
```

- [ ] **Step 2: Run it**

Run: `pnpm -C web exec vitest run src/modules/projects/OpenRepoDialog.test.ts`
Expected: FAIL (component missing).

- [ ] **Step 3: Implement the dialog**

Create `web/src/modules/projects/OpenRepoDialog.vue`:

```vue
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Loader2 } from "@lucide/vue";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemTitle,
} from "@/components/ui/item";
import { ScrollArea } from "@/components/ui/scroll-area";
import { errorMessage } from "@/lib/errors";
import { useGithubStore } from "@/modules/github/store";
import { useProjectsStore } from "./store";

const open = defineModel<boolean>("open", { default: false });
const gh = useGithubStore();
const projects = useProjectsStore();

const filter = ref("");
const opening = ref<string | null>(null);
const error = ref<string | null>(null);

watch(open, (isOpen) => {
  if (!isOpen) return;
  filter.value = "";
  error.value = null;
  if (gh.user) gh.loadRepos().catch((e) => (error.value = errorMessage(e)));
});

const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return gh.repos.filter((r) => !q || r.fullName.toLowerCase().includes(q));
});

async function openRepo(fullName: string) {
  if (opening.value) return;
  opening.value = fullName;
  error.value = null;
  try {
    await projects.cloneRepo(fullName);
    open.value = false;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    opening.value = null;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>Open from GitHub</DialogTitle>
        <DialogDescription>
          Pick a site repository. It's downloaded to this computer so you can edit and preview it.
        </DialogDescription>
      </DialogHeader>
      <p v-if="!gh.user" class="text-sm">Sign in with GitHub first (top right).</p>
      <template v-else>
        <Input v-model="filter" placeholder="Filter repositories…" aria-label="Filter repositories" />
        <ScrollArea class="max-h-72">
          <p v-if="gh.reposLoading" class="text-muted-foreground p-2 text-sm">Loading…</p>
          <ItemGroup v-else>
            <Item
              v-for="r in shown"
              :key="r.fullName"
              as="button"
              type="button"
              size="sm"
              class="hover:bg-accent/60 w-full text-left"
              :disabled="!!opening"
              data-testid="repo-item"
              @click="openRepo(r.fullName)"
            >
              <ItemContent class="min-w-0">
                <ItemTitle class="w-full"><span class="truncate">{{ r.fullName }}</span></ItemTitle>
                <ItemDescription class="text-xs">
                  {{ r.private ? "Private" : "Public" }}{{ r.canPush ? "" : " · read-only" }}
                </ItemDescription>
              </ItemContent>
              <ItemActions>
                <Loader2 v-if="opening === r.fullName" class="size-4 animate-spin" />
              </ItemActions>
            </Item>
            <p v-if="shown.length === 0" class="text-muted-foreground p-2 text-sm">
              No repositories match.
            </p>
          </ItemGroup>
        </ScrollArea>
      </template>
      <p v-if="error" class="text-destructive text-sm" role="alert">{{ error }}</p>
    </DialogContent>
  </Dialog>
</template>
```

- [ ] **Step 4: Button in the projects grid**

In `web/src/modules/projects/ProjectsGrid.vue`: import `OpenRepoDialog from "./OpenRepoDialog.vue"`, `useGithubStore`, and the `FolderGit2` icon from `@lucide/vue` (Lucide no longer ships brand logos); add `const showOpen = ref(false); const gh = useGithubStore();`. Next to `<Button @click="showNew = true"><Plus /> New project</Button>` add:

```vue
        <Button v-if="gh.configured" variant="outline" @click="showOpen = true">
          <FolderGit2 /> Open from GitHub
        </Button>
```

and next to `<NewProjectDialog v-model:open="showNew" />`:

```vue
  <OpenRepoDialog v-model:open="showOpen" />
```

(Wrap the two header buttons in `<div class="flex gap-2">` if they aren't already in a flex row.)

- [ ] **Step 5: Run all web tests and type-check**

Run: `pnpm -C web test && pnpm -C web exec vue-tsc -b`
Expected: PASS. `ProjectsGrid.test.ts` may need `githubStatus` in its fake daemon if the grid now calls the store on mount — it doesn't (only `GithubAccount` calls `load()`), so no change is expected.

- [ ] **Step 6: Commit**

```bash
git add web/src/modules/projects
git commit -m "feat(web): Open from GitHub"
```

---

### Task 11: Setup documentation

**Files:**
- Create: `docs/github-setup.md`

- [ ] **Step 1: Write it**

````markdown
# GitHub sign-in setup

Done once per organisation by an admin.

1. GitHub → Settings → Developer settings → **OAuth Apps** → *New OAuth App*
   (for an organisation: Organisation settings → Developer settings).
   - Application name: `Workbench`
   - Homepage URL: your site or repo URL
   - Authorization callback URL: any URL you control (the device flow does not use it)
2. Open the app and tick **Enable Device Flow**. Copy the **Client ID** (no secret is needed).
3. If the organisation restricts OAuth apps, approve Workbench under
   Organisation settings → Third-party access.

Everyone then starts the daemon with the client id:

```sh
WORKBENCH_GITHUB_CLIENT_ID=Iv1.xxxxxxxx workbench serve
# or: workbench serve --github-client-id Iv1.xxxxxxxx
```

**Sign in with GitHub** appears in the top-right. Workbench asks for the `repo` scope
(clone and push the site repositories) and `read:user` (your name on commits). The token is
stored in `~/workbench-projects/.workbench/github.json`, readable only by you; **Sign out**
deletes it.
````

- [ ] **Step 2: Commit**

```bash
git add docs/github-setup.md
git commit -m "docs: GitHub sign-in setup"
```

---

### Task 12: Full verification

- [ ] **Step 1: Everything**

Run: `just test`
Expected: web build, `cargo test`, web tests and the template build all pass. (`tests/devserver.rs` has a 10 s timeout that can trip on a heavily loaded machine; rerun `cargo test --test devserver` alone before investigating.)

- [ ] **Step 2: Manual check with a real OAuth App**

1. `WORKBENCH_GITHUB_CLIENT_ID=<id> just dev`, open the printed link.
2. Click **Sign in with GitHub**, enter the code on github.com, approve. The header shows your avatar and name.
3. **Open from GitHub** → pick a site repo → it appears and its preview starts.
4. Ask the agent for a small change; in **History** the new checkpoint shows your name.
5. Restart the daemon: still signed in. **Sign out**: the header shows the sign-in button again.

- [ ] **Step 3: Commit any fixes, then open a PR**

```bash
git push -u origin HEAD
gh pr create --title "Phase 1: GitHub sign-in, Open from GitHub, real authors" --body-file docs/superpowers/plans/2026-10-02-phase1-github-signin.md
```
