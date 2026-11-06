//! Leave an open change for the live site: check out `main`, keep the change branch.

use std::path::Path;
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::changes;
use orso::config::Config;
use orso::error::ErrorCode;
use orso::http;
use orso::state::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;

fn git_cli(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Ada",
            "-c",
            "user.email=ada@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
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
    std::fs::write(dir.join("README.md"), "hello\n").unwrap();
    git_cli(dir, &["add", "-A"]);
    git_cli(dir, &["commit", "-m", "init"]);
}

fn commit_file(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
    git_cli(dir, &["add", name]);
    git_cli(dir, &["commit", "-m", &format!("checkpoint: {name}")]);
}

fn local_branch_exists(dir: &Path, branch: &str) -> bool {
    !git_cli(dir, &["branch", "--list", branch]).is_empty()
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

#[tokio::test]
async fn leave_checks_out_main_and_keeps_the_change_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("site");
    seed_repo(&dir);
    let change = changes::create(&dir, "New post: Draft").await.unwrap();
    commit_file(&dir, "draft.md", "draft\n");
    assert_eq!(git_cli(&dir, &["branch", "--show-current"]), change.branch);

    changes::leave_to_main(&dir).await.unwrap();

    assert_eq!(git_cli(&dir, &["branch", "--show-current"]), "main");
    assert!(local_branch_exists(&dir, &change.branch));
}

#[tokio::test]
async fn leave_is_a_no_op_when_already_on_main() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("site");
    seed_repo(&dir);

    changes::leave_to_main(&dir).await.unwrap();

    assert_eq!(git_cli(&dir, &["branch", "--show-current"]), "main");
}

#[tokio::test]
async fn leave_refuses_a_dirty_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("site");
    seed_repo(&dir);
    let change = changes::create(&dir, "New post: Dirty").await.unwrap();
    std::fs::write(dir.join("README.md"), "dirty\n").unwrap();

    let err = changes::leave_to_main(&dir).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(err.message.contains("Uncommitted"), "{err:?}");
    assert_eq!(git_cli(&dir, &["branch", "--show-current"]), change.branch);
}

async fn post(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let r = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::HOST, "localhost")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(r).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn http_leave_route() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let st = AppState::with_scaffold_install(config(&projects), false);
    let info = st.projects.create("Site").await.unwrap();
    let dir = projects.join(&info.id);
    let app = http::router(st);

    let change = changes::create(&dir, "New post: Leave").await.unwrap();
    commit_file(&dir, "leave.md", "leave\n");

    let (s, body) = post(&app, "/api/changes/leave", json!({ "project": info.id })).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "branch": "main" }));
    assert_eq!(git_cli(&dir, &["branch", "--show-current"]), "main");
    assert!(local_branch_exists(&dir, &change.branch));

    let (s, _) = post(&app, "/api/changes/leave", json!({ "project": "nope" })).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}
