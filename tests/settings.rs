//! Read and save of the project's `orso.json` over HTTP.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::config::Config;
use orso::state::AppState;
use orso::{changes, http};
use serde_json::{json, Value};
use tower::ServiceExt;

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

fn link_node_modules(dir: &Path) {
    let modules =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/astro-marketing/node_modules");
    std::os::unix::fs::symlink(modules, dir.join("node_modules")).unwrap();
    let exclude = dir.join(".git/info/exclude");
    let mut text = std::fs::read_to_string(&exclude).unwrap_or_default();
    text.push_str("\n/node_modules\n");
    std::fs::write(exclude, text).unwrap();
}

struct Env {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    id: String,
    dir: PathBuf,
}

impl Env {
    fn file(&self) -> PathBuf {
        self.dir.join("orso.json")
    }
}

async fn env(change: bool) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(
        Config {
            port: 0,
            origin: None,
            projects_dir: tmp.path().to_path_buf(),
            no_auth: true,
            token: None,
            agents_config: None,
            ..Default::default()
        },
        false,
    );
    let info = state.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    link_node_modules(&dir);
    if change {
        changes::create(&dir, "Edit settings").await.unwrap();
    }
    Env {
        app: http::router(state),
        _tmp: tmp,
        id: info.id,
        dir,
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

async fn get(e: &Env) -> (StatusCode, Value) {
    call(
        &e.app,
        "GET",
        &format!("/api/settings?project={}", e.id),
        None,
    )
    .await
}

async fn put(e: &Env, mut body: Value) -> (StatusCode, Value) {
    body["project"] = json!(e.id);
    call(&e.app, "PUT", "/api/settings", Some(body)).await
}

fn branch(e: &Env) -> String {
    git_cli(&e.dir, &["branch", "--show-current"])
}

#[tokio::test]
async fn get_without_file() {
    let e = env(false).await;
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["exists"], false);
    assert_eq!(v["settings"], json!({}));
    assert_eq!(
        v["defaults"]["dev"],
        "npm run dev -- --port {port} --host {host} --ignore-lock"
    );
    assert_eq!(v["defaults"]["root"], ".");
    assert_eq!(v["errors"], json!({}));
    assert!(v.get("hash").is_none() || v["hash"].is_null(), "{v}");
}

#[tokio::test]
async fn get_reports_field_errors() {
    let e = env(false).await;
    std::fs::write(e.file(), r#"{"dev":"x"}"#).unwrap();
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["exists"], true);
    assert!(
        v["errors"]["dev"].as_str().unwrap().contains("{port}"),
        "{v}"
    );

    std::fs::write(e.file(), "nope").unwrap();
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(
        v["errors"]["_file"].as_str().is_some_and(|m| !m.is_empty()),
        "{v}"
    );
}

#[tokio::test]
async fn save_on_change_writes_and_checkpoints() {
    let e = env(true).await;
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "settings": {"root": "", "install": "pnpm install", "custom": 1, "note": ""}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["exists"], true);
    let text = std::fs::read_to_string(e.file()).unwrap();
    let parsed: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed, json!({"install": "pnpm install", "custom": 1, "note": ""}));
    assert!(text.ends_with('\n'), "{text}");
    assert_eq!(
        git_cli(&e.dir, &["log", "-1", "--format=%s"]),
        "checkpoint: Updated project settings"
    );
    assert_eq!(
        git_cli(&e.dir, &["show", "--pretty=format:", "--name-only", "HEAD"]),
        "orso.json"
    );
    assert_eq!(git_cli(&e.dir, &["status", "--porcelain", "orso.json"]), "");
}

#[tokio::test]
async fn save_keeps_unknown_keys() {
    let e = env(true).await;
    std::fs::write(e.file(), r#"{"x":{"y":1},"install":"a"}"#).unwrap();
    let (_, v) = get(&e).await;
    let hash = v["hash"].clone();
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": hash, "settings": {"x": {"y": 1}, "install": "b"}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let parsed: Value = serde_json::from_str(&std::fs::read_to_string(e.file()).unwrap()).unwrap();
    assert_eq!(parsed, json!({"x": {"y": 1}, "install": "b"}));
}

#[tokio::test]
async fn save_on_main_requires_change() {
    let e = env(false).await;
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "settings": {"install": "pnpm install"}}),
    )
    .await;
    assert!(s.is_client_error(), "{s} {v}");
    assert_eq!(v["error"]["code"], "CHANGE_REQUIRED");
    assert!(!e.file().exists());
}

#[tokio::test]
async fn save_with_stale_hash() {
    let e = env(true).await;
    std::fs::write(e.file(), r#"{"install":"a"}"#).unwrap();
    let b = branch(&e);
    let (s, v) = put(
        &e,
        json!({"branch": b, "hash": "wrong", "settings": {"install": "b"}}),
    )
    .await;
    assert_eq!(v["error"]["reason"], "stale_content", "{s} {v}");
    // No hash while the file exists is also stale.
    let (s, v) = put(&e, json!({"branch": b, "settings": {"install": "b"}})).await;
    assert_eq!(v["error"]["reason"], "stale_content", "{s} {v}");
    assert_eq!(
        std::fs::read_to_string(e.file()).unwrap(),
        r#"{"install":"a"}"#
    );
}

#[tokio::test]
async fn save_rejects_invalid() {
    let e = env(true).await;
    std::fs::write(e.file(), r#"{"install":"a"}"#).unwrap();
    let (_, v) = get(&e).await;
    let hash = v["hash"].clone();
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": hash, "settings": {"dev": "x"}}),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"]["reason"], "invalid_settings");
    assert_eq!(v["error"]["details"][0]["path"], json!(["dev"]), "{v}");
    assert!(
        v["error"]["details"][0]["message"]
            .as_str()
            .unwrap()
            .contains("{port}"),
        "{v}"
    );
    assert_eq!(
        std::fs::read_to_string(e.file()).unwrap(),
        r#"{"install":"a"}"#
    );
}

#[tokio::test]
async fn save_repairs_invalid_json() {
    let e = env(true).await;
    std::fs::write(e.file(), "nope").unwrap();
    let (_, v) = get(&e).await;
    let hash = v["hash"].clone();
    assert!(hash.is_string(), "{v}");
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": hash, "settings": {"install": "pnpm install"}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["exists"], true);
    let parsed: Value = serde_json::from_str(&std::fs::read_to_string(e.file()).unwrap()).unwrap();
    assert_eq!(parsed, json!({"install": "pnpm install"}));
}
