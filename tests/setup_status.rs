use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use orso::config::Config;
use orso::http;
use orso::state::AppState;

const TOKEN: &str = "abc123";

fn fake_exe(dir: &Path, name: &str) {
    let p = dir.join(name);
    std::fs::write(&p, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn app(projects: &Path, search_path: OsString) -> axum::Router {
    http::router(AppState::new(Config {
        projects_dir: projects.to_path_buf(),
        token: Some(TOKEN.into()),
        search_path: Some(search_path),
        ..Default::default()
    }))
}

async fn status(app: axum::Router) -> Value {
    let req = Request::builder()
        .uri("/api/setup/status")
        .header(header::HOST, "localhost")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn reports_git_and_gh_found_and_no_agents_on_search_path() {
    let projects = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    fake_exe(bin.path(), "git");
    fake_exe(bin.path(), "gh");

    let body = status(app(projects.path(), bin.path().into())).await;

    assert_eq!(body["git"], true);
    assert_eq!(body["gh"], true);
    assert_eq!(body["githubSignedIn"], false);
    assert_eq!(body["projectsDir"], projects.path().to_str().unwrap());
    let providers = body["providers"].as_array().unwrap();
    let ids: Vec<&str> = providers
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["cursor", "claude", "codex", "opencode"]);
    for p in providers {
        assert_eq!(p["available"], false, "{p}");
        assert!(!p["name"].as_str().unwrap().is_empty(), "{p}");
    }
}

#[tokio::test]
async fn reports_everything_missing_on_an_empty_search_path() {
    let projects = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();

    let body = status(app(projects.path(), bin.path().into())).await;

    assert_eq!(body["git"], false);
    assert_eq!(body["gh"], false);
    assert!(body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["available"] == false));
}

#[tokio::test]
async fn an_agent_on_the_search_path_is_available() {
    let projects = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    fake_exe(bin.path(), "claude");
    // Not executable: not a usable CLI.
    std::fs::write(bin.path().join("codex"), "").unwrap();

    let body = status(app(projects.path(), bin.path().into())).await;

    let available: Vec<(&str, bool)> = body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p["available"].as_bool().unwrap()))
        .collect();
    assert_eq!(
        available,
        [
            ("cursor", false),
            ("claude", true),
            ("codex", false),
            ("opencode", false)
        ]
    );
}

#[tokio::test]
async fn a_saved_github_session_counts_as_signed_in() {
    let projects = tempfile::tempdir().unwrap();
    let state_dir = projects.path().join(".orso");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::fs::write(
        state_dir.join("github.json"),
        json!({
            "token": "tok",
            "user": { "login": "ada", "id": 42, "name": null, "avatarUrl": "" }
        })
        .to_string(),
    )
    .unwrap();
    let bin = tempfile::tempdir().unwrap();

    let body = status(app(projects.path(), bin.path().into())).await;

    assert_eq!(body["githubSignedIn"], true);
}

#[tokio::test]
async fn requires_the_token() {
    let projects = tempfile::tempdir().unwrap();
    let req = Request::builder()
        .uri("/api/setup/status")
        .header(header::HOST, "localhost")
        .body(Body::empty())
        .unwrap();
    let res = app(projects.path(), OsString::new())
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
