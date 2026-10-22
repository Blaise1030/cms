//! Real npm + Astro: a scaffolded project's `orso()` integration writes the content manifest
//! and saves entries through `POST /__orso`. Run with
//! `cargo test --test content_e2e -- --ignored --nocapture`.

use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use futures_util::FutureExt;
use http_body_util::BodyExt;
use orso::config::Config;
use orso::content::catalog::list_collections;
use orso::projects::DevserverStatus;
use orso::state::AppState;
use orso::{changes, http};
use serde_json::{json, Value};
use tower::ServiceExt;

const READY_TIMEOUT: Duration = Duration::from_secs(300);
const WELCOME: &str = "src/content/blog/en/welcome.md";

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

async fn wait_ready(state: &AppState, id: &str) {
    let start = Instant::now();
    loop {
        let status = state.projects.get(id).await.unwrap().devserver;
        match status {
            DevserverStatus::Ready => return,
            DevserverStatus::Crashed => panic!(
                "dev server for {id} crashed: {:?}",
                state.devservers.crash_message(id)
            ),
            _ => {}
        }
        assert!(
            start.elapsed() < READY_TIMEOUT,
            "dev server for {id} not ready ({status:?})"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn save_through_the_integration(state: &AppState, app: &axum::Router) {
    let (s, created) = call(
        app,
        "POST",
        "/api/projects",
        Some(json!({ "name": "Content E2E" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap().to_string();
    let project_dir = state.projects.get(&id).await.unwrap().dir;
    let dir = project_dir.as_path();
    wait_ready(state, &id).await;

    assert!(dir.join("packages/astro/src/index.js").is_file());
    assert!(dir.join(".astro/orso-content.json").is_file());

    let collections = list_collections(dir).await.unwrap();
    for name in ["blog", "helpDocs"] {
        let c = collections
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no {name} collection: {collections:?}"));
        assert!(c.editable, "{name} is not editable: {c:?}");
    }

    let change = changes::create(dir, "Retitle welcome").await.unwrap();
    let branch = git(dir, &["branch", "--show-current"]);
    assert_eq!(branch, change.branch);

    let (s, entry) = call(
        app,
        "GET",
        &format!("/api/content/entry?project={id}&collection=blog&id=welcome"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{entry}");

    let (s, saved) = call(
        app,
        "PUT",
        "/api/content/entry",
        Some(json!({
            "project": id,
            "collection": "blog",
            "id": "welcome",
            "branch": branch,
            "hash": entry["hash"],
            "fields": { "title": "Welcome, end to end" },
            "body": entry["body"],
        })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{saved}");
    let source = std::fs::read_to_string(dir.join(WELCOME)).unwrap();
    assert!(source.contains("Welcome, end to end"), "{source}");
    let subject = git(dir, &["log", "-1", "--format=%s"]);
    assert!(subject.contains("Update blog/welcome"), "{subject}");
}

#[tokio::test]
#[ignore = "needs network + npm"]
async fn real_astro_saves_content_through_the_integration() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(Config {
        port: 0,
        origin: None,
        projects_dir: tmp.path().to_path_buf(),
        no_auth: true,
        token: None,
        agents_config: None,
        ..Default::default()
    });
    let app = http::router(state.clone());

    let result = AssertUnwindSafe(save_through_the_integration(&state, &app))
        .catch_unwind()
        .await;
    state.devservers.shutdown().await;
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
