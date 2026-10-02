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
    assert_eq!(body, json!({ "configured": true, "user": null }));

    // Opening a repo needs a session.
    let (s, body) = call(
        &app,
        req(
            "POST",
            "/api/projects/clone",
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
            "/api/projects/clone",
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
            "/api/projects/clone",
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
    assert!(!std::fs::read_to_string(dir.join(".git/config"))
        .unwrap()
        .contains("extraheader"));

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
    assert_eq!(body, json!({ "configured": false, "user": null }));
    let (s, body) = call(&app, req("POST", "/api/github/device", None)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "GITHUB_ERR");
}
