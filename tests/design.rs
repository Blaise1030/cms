//! Read and save of the project's `DESIGN.md` over HTTP.

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
    // `.gitignore` has `node_modules/`, which does not match a symlink.
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
        self.dir.join("DESIGN.md")
    }
}

/// A template project, optionally on a change branch.
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
        changes::create(&dir, "Edit design").await.unwrap();
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
        &format!("/api/design?project={}", e.id),
        None,
    )
    .await
}

async fn put(e: &Env, mut body: Value) -> (StatusCode, Value) {
    body["project"] = json!(e.id);
    call(&e.app, "PUT", "/api/design", Some(body)).await
}

fn branch(e: &Env) -> String {
    git_cli(&e.dir, &["branch", "--show-current"])
}

fn keys(v: &Value) -> Vec<&str> {
    v.as_object().unwrap().keys().map(String::as_str).collect()
}

#[tokio::test]
async fn template_project_has_design() {
    let e = env(false).await;
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["exists"], true);
    assert_eq!(v["tokens"]["version"], "alpha");
    assert!(v["hash"].as_str().is_some_and(|h| h.len() == 64), "{v}");
    assert!(v.get("starter").is_none(), "{v}");
}

#[tokio::test]
async fn missing_file_returns_starter() {
    let e = env(false).await;
    std::fs::remove_file(e.file()).unwrap();
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["exists"], false);
    assert_eq!(v["tokens"], json!({}));
    assert_eq!(v["body"], "");
    assert!(v.get("hash").is_none(), "{v}");
    assert_eq!(v["starter"]["tokens"]["version"], "alpha");
    assert!(v["starter"]["body"]
        .as_str()
        .unwrap()
        .contains("## Overview"));
}

#[tokio::test]
async fn create_writes_file_and_checkpoints() {
    let e = env(true).await;
    std::fs::remove_file(e.file()).unwrap();
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "tokens": {"version": "alpha", "name": "Site"}, "body": "## Overview\n\nHi\n"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["exists"], true);
    assert_eq!(v["tokens"]["name"], "Site");
    assert!(v["hash"].is_string());
    let text = std::fs::read_to_string(e.file()).unwrap();
    assert!(text.starts_with("---\n"), "{text}");
    assert!(text.ends_with("---\n## Overview\n\nHi\n"), "{text}");
    assert_eq!(
        git_cli(&e.dir, &["log", "-1", "--format=%s"]),
        "checkpoint: Updated design"
    );
    assert_eq!(git_cli(&e.dir, &["status", "--porcelain", "DESIGN.md"]), "");
}

#[tokio::test]
async fn create_when_present_is_stale() {
    let e = env(true).await;
    let (s, v) = put(&e, json!({"branch": branch(&e), "tokens": {}, "body": "x"})).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "stale_content");
}

#[tokio::test]
async fn update_with_old_hash_is_stale() {
    let e = env(true).await;
    let (_, v) = get(&e).await;
    let old = v["hash"].as_str().unwrap().to_string();
    let b = branch(&e);
    let (s, v) = put(
        &e,
        json!({"branch": b, "hash": old, "tokens": {"name": "A"}, "body": "one\n"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, v) = put(
        &e,
        json!({"branch": b, "hash": old, "tokens": {"name": "B"}, "body": "two\n"}),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "stale_content");
    assert!(std::fs::read_to_string(e.file())
        .unwrap()
        .contains("name: A"));
}

#[tokio::test]
async fn unknown_keys_and_order_survive_update() {
    let e = env(true).await;
    let source = "---\nzeta: 1\ncolors:\n  b: '#222222'\n  a: '#111111'\ncomponents:\n  button:\n    rounded: 4px\n---\n## Iconography\n\nLine icons.\n";
    std::fs::write(e.file(), source).unwrap();
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(keys(&v["tokens"]), ["zeta", "colors", "components"]);
    assert_eq!(keys(&v["tokens"]["colors"]), ["b", "a"]);
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": v["hash"], "tokens": v["tokens"], "body": v["body"]}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(keys(&v["tokens"]), ["zeta", "colors", "components"]);
    assert_eq!(keys(&v["tokens"]["colors"]), ["b", "a"]);
    let text = std::fs::read_to_string(e.file()).unwrap();
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle} in {text}"))
    };
    assert!(
        at("zeta") < at("colors:") && at("colors:") < at("components:"),
        "{text}"
    );
    assert!(at("  b:") < at("  a:"), "{text}");
    assert!(
        text.ends_with("---\n## Iconography\n\nLine icons.\n"),
        "{text}"
    );
}

#[tokio::test]
async fn no_frontmatter_round_trips() {
    let e = env(true).await;
    std::fs::write(e.file(), "# Hi\n\nText\n").unwrap();
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["tokens"], json!({}));
    assert_eq!(v["body"], "# Hi\n\nText\n");
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": v["hash"], "tokens": {"name": "X"}, "body": v["body"]}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        std::fs::read_to_string(e.file()).unwrap(),
        "---\nname: X\n---\n# Hi\n\nText\n"
    );
}

#[tokio::test]
async fn empty_tokens_with_dashed_body_round_trip() {
    let e = env(true).await;
    let body = "---\nnot: tokens\n---\nrest\n";
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": v["hash"], "tokens": {}, "body": body}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["tokens"], json!({}));
    assert_eq!(v["body"], body);
    assert_eq!(
        std::fs::read_to_string(e.file()).unwrap(),
        format!("---\n---\n{body}")
    );
}

#[tokio::test]
async fn empty_tokens_with_plain_body_write_no_frontmatter() {
    let e = env(true).await;
    let (_, v) = get(&e).await;
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": v["hash"], "tokens": {}, "body": "# Hi\n"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(std::fs::read_to_string(e.file()).unwrap(), "# Hi\n");
}

#[tokio::test]
async fn body_with_rule_is_kept() {
    let e = env(false).await;
    std::fs::write(e.file(), "---\nname: X\n---\nA\n\n---\n\nB\n").unwrap();
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["tokens"], json!({"name": "X"}));
    assert_eq!(v["body"], "A\n\n---\n\nB\n");
}

#[tokio::test]
async fn save_without_change_requires_change() {
    let e = env(false).await;
    let (_, v) = get(&e).await;
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": v["hash"], "tokens": {}, "body": "x"}),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["code"], "CHANGE_REQUIRED");
}

#[tokio::test]
async fn invalid_frontmatter_is_reported() {
    let e = env(true).await;
    let source = "---\n: [\n---\n";
    std::fs::write(e.file(), source).unwrap();
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"]["reason"], "invalid_design");
    // The raw source rides along so the tab can show it read-only.
    assert_eq!(v["error"]["details"][0]["path"], json!(["source"]), "{v}");
    assert_eq!(v["error"]["details"][0]["message"], source, "{v}");
    // A save with a stale hash must not touch the broken file.
    let (s, _) = put(
        &e,
        json!({"branch": branch(&e), "hash": "any", "tokens": {}, "body": ""}),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(std::fs::read_to_string(e.file()).unwrap(), source);
}

#[tokio::test]
async fn tokens_must_be_object() {
    let e = env(true).await;
    let (_, v) = get(&e).await;
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": v["hash"], "tokens": [], "body": "x"}),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
}

fn sha256(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

#[tokio::test]
async fn correct_hash_of_invalid_file_is_refused() {
    let e = env(true).await;
    let source = "---\n: [\n---\nbody\n";
    std::fs::write(e.file(), source).unwrap();
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": sha256(source), "tokens": {"name": "X"}, "body": "new\n"}),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"]["reason"], "invalid_design");
    assert_eq!(std::fs::read_to_string(e.file()).unwrap(), source);
}

#[tokio::test]
async fn crlf_frontmatter_round_trips() {
    let e = env(true).await;
    std::fs::write(
        e.file(),
        "---\r\nname: X\r\ncolors:\r\n  a: '#111111'\r\n---\r\n# Body\r\n",
    )
    .unwrap();
    let (s, v) = get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["tokens"],
        json!({"name": "X", "colors": {"a": "#111111"}})
    );
    assert_eq!(v["body"], "# Body\r\n");
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": v["hash"], "tokens": v["tokens"], "body": v["body"]}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["tokens"],
        json!({"name": "X", "colors": {"a": "#111111"}})
    );
    assert_eq!(v["body"], "# Body\r\n");
}

#[tokio::test]
async fn failed_checkpoint_restores_previous_file() {
    let e = env(true).await;
    let (_, v) = get(&e).await;
    let before = std::fs::read_to_string(e.file()).unwrap();
    // Git's index lock makes the checkpoint fail after the file was written.
    std::fs::write(e.dir.join(".git/index.lock"), "").unwrap();
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "hash": v["hash"], "tokens": {"name": "Z"}, "body": "z\n"}),
    )
    .await;
    assert!(!s.is_success(), "{v}");
    assert_eq!(std::fs::read_to_string(e.file()).unwrap(), before);

    // A failed create removes the new file again.
    std::fs::remove_file(e.file()).unwrap();
    let (s, v) = put(
        &e,
        json!({"branch": branch(&e), "tokens": {"name": "Z"}, "body": "z\n"}),
    )
    .await;
    assert!(!s.is_success(), "{v}");
    assert!(!e.file().exists());
}
