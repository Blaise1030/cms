//! `fs.change` events from the per-project watcher.

use std::path::Path;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::broadcast::error::TryRecvError;
use tower::ServiceExt;
use workbench::config::Config;
use workbench::http;
use workbench::state::AppState;

const TIMEOUT: Duration = Duration::from_secs(10);

fn config(dir: &Path) -> Config {
    Config {
        port: 0,
        origin: None,
        projects_dir: dir.to_path_buf(),
        no_auth: true,
        token: None,
        agents_config: None,
    }
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
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
    let res = app.clone().oneshot(b.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn next_fs_change(
    rx: &mut tokio::sync::broadcast::Receiver<std::sync::Arc<str>>,
    want_path: &str,
) -> Value {
    tokio::time::timeout(TIMEOUT, async {
        loop {
            let text = rx.recv().await.expect("event bus closed");
            let v: Value = serde_json::from_str(&text).unwrap();
            if v["type"] == "fs.change" && v["path"] == want_path {
                return v;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no fs.change for {want_path}"))
}

#[tokio::test]
async fn edits_emit_fs_change_and_ignored_dirs_stay_quiet() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(config(tmp.path()), false);
    let app = http::router(state.clone());
    let (s, p) = call(
        &app,
        "POST",
        "/api/projects",
        Some(json!({ "name": "Watch Me" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    let id = p["id"].as_str().unwrap();
    let dir = state.projects.get(id).await.unwrap().dir;
    let mut rx = state.events.subscribe();

    // Ignored dirs first: must never surface. A sentinel edit afterwards proves the
    // pipeline was live while they were written.
    std::fs::create_dir_all(dir.join("node_modules/x")).unwrap();
    std::fs::write(dir.join("node_modules/x/a.js"), "1").unwrap();
    std::fs::write(dir.join(".git/probe"), "1").unwrap();
    std::fs::write(dir.join("src/watched.txt"), "hello").unwrap();

    let ev = next_fs_change(&mut rx, "src/watched.txt").await;
    assert_eq!(ev["projectId"], id);
    assert!(
        matches!(ev["kind"].as_str(), Some("create" | "modify")),
        "{ev}"
    );

    tokio::time::sleep(Duration::from_millis(300)).await;
    loop {
        match rx.try_recv() {
            Ok(text) => {
                let v: Value = serde_json::from_str(&text).unwrap();
                let path = v["path"].as_str().unwrap_or("");
                assert!(
                    !path.starts_with("node_modules") && !path.starts_with(".git"),
                    "ignored path leaked: {v}"
                );
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Lagged(_)) => continue,
            Err(TryRecvError::Closed) => break,
        }
    }

    // Removal is reported too.
    std::fs::remove_file(dir.join("src/watched.txt")).unwrap();
    let ev = next_fs_change(&mut rx, "src/watched.txt").await;
    assert_eq!(ev["kind"], "remove");

    // Delete stops the watcher: no panics, project gone.
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/api/projects/{id}?confirm=true"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}
