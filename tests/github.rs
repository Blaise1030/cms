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
    fake_github_with(false).await
}

async fn fake_github_with(repos_unauthorized: bool) -> String {
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
                let authed = headers
                    .get(header::AUTHORIZATION)
                    .is_some_and(|v| v == "Bearer tok");
                if !authed {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({ "message": "Bad credentials" })),
                    );
                }
                (
                    StatusCode::OK,
                    Json(json!({
                        "login": "ada", "id": 42, "name": "Ada Lovelace",
                        "avatar_url": "https://avatars.example/ada.png"
                    })),
                )
            }),
        )
        .route(
            "/user/repos",
            get(move || async move {
                if repos_unauthorized {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({ "message": "Bad credentials" })),
                    );
                }
                (
                    StatusCode::OK,
                    Json(json!([{
                        "full_name": "acme/site", "private": true,
                        "default_branch": "main", "permissions": { "push": true }
                    }])),
                )
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// `{root}/acme/site.git`: a bare repo with one commit.
fn bare_repo(root: &Path) {
    let src = root.join("src-repo");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("package.json"), "{\"name\":\"site\"}\n").unwrap();
    git(&src, &["init", "-b", "main"]);
    git(
        &src,
        &[
            "-c",
            "user.name=Grace",
            "-c",
            "user.email=g@example.com",
            "add",
            "-A",
        ],
    );
    git(
        &src,
        &[
            "-c",
            "user.name=Grace",
            "-c",
            "user.email=g@example.com",
            "commit",
            "-m",
            "init",
        ],
    );
    std::fs::create_dir_all(root.join("acme")).unwrap();
    git(
        root,
        &["clone", "--bare", src.to_str().unwrap(), "acme/site.git"],
    );
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
            gh_path: no_gh(),
        },
        ..Default::default()
    }
}

/// A path that is never an executable: tests must not run the `gh` on this machine.
fn no_gh() -> std::path::PathBuf {
    "/nonexistent/workbench-test/gh".into()
}

const ADA_JSON: &str = r#"{"login":"ada","id":42,"name":"Ada Lovelace","avatar_url":"https://avatars.example/ada.png"}"#;
const REPOS_JSON: &str = r#"[{"full_name":"acme/site","private":true,"default_branch":"main","permissions":{"push":true}}]"#;

/// A fake `gh` in `dir`. `logged_in == false` behaves like a logged-out gh (exit 4).
fn fake_gh(dir: &Path, logged_in: bool) -> std::path::PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("gh");
    let script = if logged_in {
        format!(
            r#"#!/bin/sh
echo "$@" >> "$0.calls"
case "$1 $2" in
  "api user") printf '%s' '{ADA_JSON}' ;;
  "api /user/repos?per_page=100&sort=pushed") printf '%s' '{REPOS_JSON}' ;;
  "auth git-credential") cat >/dev/null; printf 'username=x-access-token\npassword=tok\n' ;;
  *) echo "unexpected: $*" >&2; exit 2 ;;
esac
"#
        )
    } else {
        "#!/bin/sh\necho 'To get started with GitHub CLI, please run:  gh auth login' >&2\nexit 4\n"
            .to_string()
    };
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn cli_config(projects: &Path, git_root: &Path, gh: &Path) -> Config {
    Config {
        projects_dir: projects.to_path_buf(),
        no_auth: true,
        github: GithubConfig {
            client_id: None,
            // Nothing in these tests may reach a real GitHub.
            api_url: "http://127.0.0.1:1".into(),
            web_url: "http://127.0.0.1:1".into(),
            git_url: format!("file://{}", git_root.display()),
            gh_path: gh.to_path_buf(),
        },
        ..Default::default()
    }
}

fn req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let b = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "localhost");
    match body {
        Some(v) => b
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => b.body(Body::empty()).unwrap(),
    }
}

async fn call(app: &Router, r: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(r).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn sign_in_open_repo_and_author_commits() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let git_root = tmp.path().join("git");
    bare_repo(&git_root);
    let base = fake_github().await;
    let app = http::router(AppState::with_scaffold_install(
        config(&projects, &base, &git_root),
        false,
    ));

    let (s, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["configured"], true);
    assert_eq!(body["deviceFlow"], true);
    assert_eq!(body["source"], Value::Null);
    assert_eq!(body["user"], Value::Null);

    // Opening a repo needs a session.
    let (s, body) = call(
        &app,
        req(
            "POST",
            "/api/github/clone",
            Some(json!({ "repo": "acme/site" })),
        ),
    )
    .await;
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
    assert_eq!(
        body["repos"],
        json!([{
            "fullName": "acme/site", "private": true, "defaultBranch": "main", "canPush": true
        }])
    );

    let (s, body) = call(
        &app,
        req(
            "POST",
            "/api/github/clone",
            Some(json!({ "repo": "../etc" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "BADREQ");

    let (s, info) = call(
        &app,
        req(
            "POST",
            "/api/github/clone",
            Some(json!({ "repo": "acme/site" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{info}");
    assert_eq!(info["id"], "site");
    assert_eq!(info["repo"], "acme/site");
    let dir = projects.join("site");
    assert!(dir.join("package.json").exists());
    assert_eq!(
        git(&dir, &["config", "--local", "user.name"]),
        "Ada Lovelace"
    );
    assert_eq!(
        git(&dir, &["config", "--local", "user.email"]),
        "42+ada@users.noreply.github.com"
    );
    assert!(git(&dir, &["remote", "get-url", "origin"]).ends_with("acme/site.git"));
    // The token was only in the environment of the clone.
    let cfg = std::fs::read_to_string(dir.join(".git/config")).unwrap();
    let origin = git(&dir, &["remote", "get-url", "origin"]);
    assert!(!cfg.contains("extraheader"));
    for leak in ["eC1hY2Nlc3MtdG9rZW46dG9r", "tok@"] {
        assert!(
            !cfg.contains(leak) && !origin.contains(leak),
            "token leaked: {leak}"
        );
    }

    // New template projects are authored by the signed-in user too.
    let (_, made) = call(
        &app,
        req("POST", "/api/projects", Some(json!({ "name": "Fresh" }))),
    )
    .await;
    let fresh = projects.join(made["id"].as_str().unwrap());
    assert_eq!(git(&fresh, &["log", "-1", "--format=%an"]), "Ada Lovelace");

    // The session survives a daemon restart.
    let again = http::router(AppState::with_scaffold_install(
        config(&projects, &base, &git_root),
        false,
    ));
    let (_, body) = call(&again, req("GET", "/api/github/status", None)).await;
    assert_eq!(body["user"]["login"], "ada");
    assert_eq!(body["source"], "oauth");

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
    let cfg = Config {
        projects_dir: tmp.path().to_path_buf(),
        no_auth: true,
        ..Default::default()
    };
    let app = http::router(AppState::with_scaffold_install(cfg, false));
    let (_, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(
        body,
        json!({ "configured": false, "deviceFlow": false, "source": null, "user": null })
    );
    let (s, body) = call(&app, req("POST", "/api/github/device", None)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "GITHUB_ERR");
}

#[tokio::test]
async fn expired_token_signs_out_on_repos() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let git_root = tmp.path().join("git");
    let base = fake_github_with(true).await;
    let app = http::router(AppState::with_scaffold_install(
        config(&projects, &base, &git_root),
        false,
    ));
    call(&app, req("POST", "/api/github/device", None)).await;
    call(&app, req("POST", "/api/github/device/poll", None)).await;
    let (_, body) = call(&app, req("POST", "/api/github/device/poll", None)).await;
    assert_eq!(body["status"], "signedIn");
    let (_, made) = call(
        &app,
        req("POST", "/api/projects", Some(json!({ "name": "Fresh" }))),
    )
    .await;
    let fresh = projects.join(made["id"].as_str().unwrap());
    assert_eq!(
        git(&fresh, &["config", "--local", "user.name"]),
        "Ada Lovelace"
    );

    let (s, body) = call(&app, req("GET", "/api/github/repos", None)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "GITHUB_ERR");
    let (_, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(body["user"], Value::Null);
    assert!(!projects.join(".workbench/github.json").exists());
    assert_eq!(
        git(&fresh, &["config", "--local", "user.name"]),
        "Workbench"
    );
}

#[tokio::test]
async fn github_endpoints_require_auth() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = Config {
        projects_dir: tmp.path().to_path_buf(),
        no_auth: false,
        token: Some("abc123".into()),
        ..Default::default()
    };
    let app = http::router(AppState::with_scaffold_install(cfg, false));
    let (s, _) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let mut r = req("GET", "/api/github/status", None);
    r.headers_mut()
        .insert(header::AUTHORIZATION, "Bearer abc123".parse().unwrap());
    let (s, _) = call(&app, r).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn project_named_clone_can_still_be_deleted() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = Config {
        projects_dir: tmp.path().to_path_buf(),
        no_auth: true,
        ..Default::default()
    };
    let app = http::router(AppState::with_scaffold_install(cfg, false));
    let (s, made) = call(
        &app,
        req("POST", "/api/projects", Some(json!({ "name": "Clone" }))),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    assert_eq!(made["id"], "clone");
    let (s, _) = call(
        &app,
        req("DELETE", "/api/projects/clone?confirm=true", None),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

/// Tests that point `GIT_CONFIG_GLOBAL` somewhere take turns.
async fn lock_global_git_env() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    LOCK.lock().await
}

#[tokio::test]
async fn cli_login_signs_in_lists_repos_and_clones_without_storing_a_token() {
    let _g = lock_global_git_env().await;
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let git_root = tmp.path().join("git");
    bare_repo(&git_root);
    let gh = fake_gh(&tmp.path().join("bin"), true);
    let global = tmp.path().join("gitconfig");
    std::fs::write(
        &global,
        "[user]\n\tname = Ada Global\n\temail = ada@example.com\n",
    )
    .unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &global);
    let app = http::router(AppState::with_scaffold_install(
        cli_config(&projects, &git_root, &gh),
        false,
    ));

    let (_, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(
        body,
        json!({
            "configured": true, "deviceFlow": false, "source": "cli",
            "user": { "login": "ada", "id": 42, "name": "Ada Lovelace",
                      "avatarUrl": "https://avatars.example/ada.png" }
        })
    );

    let (_, body) = call(&app, req("GET", "/api/github/repos", None)).await;
    assert_eq!(
        body["repos"],
        json!([{
            "fullName": "acme/site", "private": true, "defaultBranch": "main", "canPush": true
        }])
    );

    let (s, info) = call(
        &app,
        req(
            "POST",
            "/api/github/clone",
            Some(json!({ "repo": "acme/site" })),
        ),
    )
    .await;
    std::env::remove_var("GIT_CONFIG_GLOBAL");
    assert_eq!(s, StatusCode::CREATED, "{info}");
    let dir = projects.join("site");
    assert!(dir.join("package.json").exists());
    // Authored as the machine's own git identity.
    assert_eq!(git(&dir, &["config", "--local", "user.name"]), "Ada Global");
    assert_eq!(
        git(&dir, &["config", "--local", "user.email"]),
        "ada@example.com"
    );
    let cfg = std::fs::read_to_string(dir.join(".git/config")).unwrap();
    assert!(!cfg.contains("credential") && !cfg.contains("extraheader"));
    assert!(!cfg.contains("tok"));
    // Nothing was persisted: no session file.
    assert!(!projects.join(".workbench/github.json").exists());
}

#[tokio::test]
async fn cli_identity_falls_back_to_the_noreply_address_without_a_global_git_identity() {
    let _g = lock_global_git_env().await;
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let git_root = tmp.path().join("git");
    let gh = fake_gh(&tmp.path().join("bin"), true);
    let global = tmp.path().join("gitconfig");
    std::fs::write(&global, "").unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &global);
    let app = http::router(AppState::with_scaffold_install(
        cli_config(&projects, &git_root, &gh),
        false,
    ));
    let (s, made) = call(
        &app,
        req("POST", "/api/projects", Some(json!({ "name": "Fresh" }))),
    )
    .await;
    std::env::remove_var("GIT_CONFIG_GLOBAL");
    assert_eq!(s, StatusCode::CREATED, "{made}");
    let fresh = projects.join(made["id"].as_str().unwrap());
    assert_eq!(
        git(&fresh, &["config", "--local", "user.name"]),
        "Ada Lovelace"
    );
    assert_eq!(
        git(&fresh, &["config", "--local", "user.email"]),
        "42+ada@users.noreply.github.com"
    );
}

#[tokio::test]
async fn logged_out_cli_is_not_signed_in() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let gh = fake_gh(&tmp.path().join("bin"), false);
    let app = http::router(AppState::with_scaffold_install(
        cli_config(&projects, &tmp.path().join("git"), &gh),
        false,
    ));
    let (_, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(
        body,
        json!({ "configured": false, "deviceFlow": false, "source": null, "user": null })
    );
    let (s, body) = call(&app, req("GET", "/api/github/repos", None)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "GITHUB_ERR");
    let (s, _) = call(
        &app,
        req(
            "POST",
            "/api/github/clone",
            Some(json!({ "repo": "acme/site" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn cli_status_is_cached_between_calls() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let gh = fake_gh(&tmp.path().join("bin"), true);
    let app = http::router(AppState::with_scaffold_install(
        cli_config(&projects, &tmp.path().join("git"), &gh),
        false,
    ));
    for _ in 0..3 {
        call(&app, req("GET", "/api/github/status", None)).await;
    }
    let calls = std::fs::read_to_string(tmp.path().join("bin/gh.calls")).unwrap();
    assert_eq!(
        calls.lines().filter(|l| *l == "api user").count(),
        1,
        "{calls}"
    );
}

#[tokio::test]
async fn gh_not_logged_in_error_on_repos_is_a_400_and_drops_the_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let gh = fake_gh(&tmp.path().join("bin"), true);
    let app = http::router(AppState::with_scaffold_install(
        cli_config(&projects, &tmp.path().join("git"), &gh),
        false,
    ));
    let (_, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(body["source"], "cli");
    // gh gets logged out behind our back.
    let out = fake_gh(&tmp.path().join("bin2"), false);
    std::fs::copy(&out, &gh).unwrap();
    let (s, body) = call(&app, req("GET", "/api/github/repos", None)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("gh auth login"));
    let (_, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(body["user"], Value::Null);
}

#[tokio::test]
async fn oauth_session_takes_priority_over_the_cli() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let git_root = tmp.path().join("git");
    let base = fake_github().await;
    let gh = fake_gh(&tmp.path().join("bin"), true);
    let mut cfg = config(&projects, &base, &git_root);
    cfg.github.gh_path = gh.clone();
    let app = http::router(AppState::with_scaffold_install(cfg, false));
    let (_, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(body["source"], "cli");
    call(&app, req("POST", "/api/github/device", None)).await;
    call(&app, req("POST", "/api/github/device/poll", None)).await;
    call(&app, req("POST", "/api/github/device/poll", None)).await;
    let (_, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(body["source"], "oauth");
    assert_eq!(body["deviceFlow"], true);
    // Signing out drops the oauth session; the CLI login is still there.
    call(&app, req("POST", "/api/github/signout", None)).await;
    let (_, body) = call(&app, req("GET", "/api/github/status", None)).await;
    assert_eq!(body["source"], "cli");
    assert_eq!(body["user"]["login"], "ada");
}

#[tokio::test]
async fn status_applies_the_cli_identity_to_existing_projects() {
    let _g = lock_global_git_env().await;
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let gh = fake_gh(&tmp.path().join("bin"), true);
    let global = tmp.path().join("gitconfig");
    std::fs::write(&global, "").unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &global);
    // The project is made while gh is missing (Workbench identity), then gh appears.
    let app_no_gh = http::router(AppState::with_scaffold_install(
        cli_config(&projects, &tmp.path().join("git"), &no_gh()),
        false,
    ));
    let (_, made) = call(
        &app_no_gh,
        req("POST", "/api/projects", Some(json!({ "name": "Fresh" }))),
    )
    .await;
    let fresh = projects.join(made["id"].as_str().unwrap());
    assert_eq!(
        git(&fresh, &["config", "--local", "user.name"]),
        "Workbench"
    );
    let app = http::router(AppState::with_scaffold_install(
        cli_config(&projects, &tmp.path().join("git"), &gh),
        false,
    ));
    call(&app, req("GET", "/api/github/status", None)).await;
    std::env::remove_var("GIT_CONFIG_GLOBAL");
    assert_eq!(
        git(&fresh, &["config", "--local", "user.name"]),
        "Ada Lovelace"
    );
}
