//! Change branches: create from latest main, active change, HTTP routes.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use workbench::config::Config;
use workbench::http;
use workbench::state::AppState;
use workbench::{changes, git};

fn git_cli(dir: &Path, args: &[&str]) -> String {
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

fn seed_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git_cli(dir, &["init", "-b", "main"]);
    git_cli(dir, &["config", "user.name", "Ada"]);
    git_cli(dir, &["config", "user.email", "ada@example.com"]);
    std::fs::write(dir.join("README.md"), "hello\n").unwrap();
    git_cli(dir, &["add", "-A"]);
    git_cli(dir, &["commit", "-m", "init"]);
}

/// Local clone with `origin` pointing at a bare remote (so `fetch` works).
fn clone_with_origin(tmp: &Path) -> PathBuf {
    let src = tmp.join("src");
    seed_repo(&src);
    git_cli(tmp, &["clone", "--bare", src.to_str().unwrap(), "remote.git"]);
    let work = tmp.join("work");
    git_cli(tmp, &["clone", "remote.git", "work"]);
    git_cli(&work, &["config", "user.name", "Ada"]);
    git_cli(&work, &["config", "user.email", "ada@example.com"]);
    work
}

fn config(dir: &Path) -> Config {
    Config {
        port: 0,
        origin: None,
        projects_dir: dir.to_path_buf(),
        no_auth: true,
        token: None,
        agents_config: None,
        ..Default::default()
    }
}

fn req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "localhost");
    let body = match body {
        Some(v) => {
            b = b.header(header::CONTENT_TYPE, "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    b.body(body).unwrap()
}

async fn call(app: &axum::Router, r: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(r).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn create_suffixes_on_collision() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());

    let first = changes::create(&work, "New post: Launch week")
        .await
        .unwrap();
    assert_eq!(first.branch, "content/launch-week");
    assert_eq!(first.title, "Launch week");
    assert_eq!(
        git_cli(&work, &["branch", "--show-current"]),
        "content/launch-week"
    );
    assert_eq!(
        git_cli(
            &work,
            &["config", "--get", "branch.content/launch-week.description"]
        ),
        "Launch week"
    );

    let second = changes::create(&work, "New post: Launch week")
        .await
        .unwrap();
    assert_eq!(second.branch, "content/launch-week-2");
    assert_eq!(
        git_cli(&work, &["branch", "--show-current"]),
        "content/launch-week-2"
    );
}

#[tokio::test]
async fn create_fetches_main_first() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let remote = tmp.path().join("remote.git");

    // Advance origin/main after the clone so a fetch is required to see the new commit.
    let push = tmp.path().join("push");
    git_cli(tmp.path(), &["clone", remote.to_str().unwrap(), "push"]);
    git_cli(&push, &["config", "user.name", "Ada"]);
    git_cli(&push, &["config", "user.email", "ada@example.com"]);
    std::fs::write(push.join("from-main.md"), "new\n").unwrap();
    git_cli(&push, &["add", "-A"]);
    git_cli(&push, &["commit", "-m", "advance main"]);
    git_cli(&push, &["push", "origin", "main"]);
    let remote_main = git_cli(&remote, &["rev-parse", "refs/heads/main"]);

    let change = changes::create(&work, "Fresh").await.unwrap();
    assert_eq!(change.branch, "content/fresh");
    let tip = git_cli(&work, &["rev-parse", "HEAD"]);
    assert_eq!(tip, remote_main);
    assert!(
        work.join("from-main.md").exists(),
        "branch should include fetched main tip"
    );
}

#[tokio::test]
async fn active_returns_branch_and_title() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());

    assert_eq!(changes::active(&work).await.unwrap(), None);

    changes::create(&work, "New post: Launch week")
        .await
        .unwrap();
    let active = changes::active(&work).await.unwrap().unwrap();
    assert_eq!(active.branch, "content/launch-week");
    assert_eq!(active.title, "Launch week");
}

#[tokio::test]
async fn http_create_and_active() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let st = AppState::with_scaffold_install(config(&projects), false);
    let info = st.projects.create("Site").await.unwrap();
    let dir = projects.join(&info.id);

    // Attach a bare origin so create can fetch main.
    let bare = tmp.path().join("origin.git");
    git_cli(tmp.path(), &["clone", "--bare", dir.to_str().unwrap(), "origin.git"]);
    git_cli(&dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git_cli(&dir, &["fetch", "origin"]);
    let _ = git::set_identity(
        &dir,
        &git::Identity {
            name: "Ada".into(),
            email: "ada@example.com".into(),
        },
    )
    .await;

    let app = http::router(st);

    let (s, body) = call(
        &app,
        req(
            "POST",
            "/api/changes",
            Some(json!({ "project": info.id, "title": "New post: Launch week" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    assert_eq!(body["branch"], "content/launch-week");
    assert_eq!(body["title"], "Launch week");

    let (s, body) = call(
        &app,
        req(
            "GET",
            &format!("/api/changes/active?project={}", info.id),
            None,
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["branch"], "content/launch-week");
    assert_eq!(body["title"], "Launch week");
}
