mod common;

use std::path::Path;
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::config::Config;
use orso::state::AppState;
use orso::{changes, git, http};
use serde_json::{json, Value};
use tower::ServiceExt;

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

fn state(dir: &Path) -> AppState {
    AppState::with_scaffold_install(config(dir, true), false)
}

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

/// A scaffolded project (git-initialised) plus router over the same state.
async fn project(tmp: &Path) -> (axum::Router, std::path::PathBuf) {
    let st = state(tmp);
    let info = st.projects.create("Site").await.unwrap();
    let dir = tmp.join(&info.id);
    (http::router(st), dir)
}

const WELCOME: &str = "src/content/blog/en/welcome.md";
const WELCOME2: &str = "src/content/blog/en/welcome2.md";

fn append(dir: &Path, path: &str, text: &str) {
    let file = dir.join(path);
    let mut body = std::fs::read_to_string(&file).unwrap_or_default();
    body.push_str(text);
    std::fs::write(file, body).unwrap();
}

#[tokio::test]
async fn file_log_lists_commits_touching_path_newest_first() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    git_cli(&dir, &["checkout", "-b", "content/x"]);
    append(&dir, WELCOME, "\nOne.\n");
    git::checkpoint(&dir, "one").await.unwrap().unwrap();
    append(&dir, "README.md", "\nOther.\n");
    git::checkpoint(&dir, "other").await.unwrap().unwrap();
    append(&dir, WELCOME, "\nTwo.\n");
    git::checkpoint(&dir, "two").await.unwrap().unwrap();

    let log = git::file_log(&dir, WELCOME).await.unwrap();
    assert_eq!(log[0].summary, "two");
    assert_eq!(log[1].summary, "one");
    assert!(log.iter().all(|v| v.summary != "other"));
    assert!(log.iter().all(|v| v.path == WELCOME));
}

#[tokio::test]
async fn file_log_marks_main_commits_published() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    let scaffold = git_cli(&dir, &["rev-parse", "main"]);
    git_cli(&dir, &["checkout", "-b", "content/x"]);
    append(&dir, WELCOME, "\nDraft.\n");
    let draft = git::checkpoint(&dir, "draft").await.unwrap().unwrap();

    let log = git::file_log(&dir, WELCOME).await.unwrap();
    let find = |sha: &str| log.iter().find(|v| v.sha == sha).unwrap();
    assert!(!find(&draft.sha).published);
    assert!(find(&scaffold).published);
}

#[tokio::test]
async fn file_log_follows_renames_and_file_at_reads_old_path() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    append(&dir, WELCOME, "\nBefore the move.\n");
    git::checkpoint(&dir, "before").await.unwrap().unwrap();
    git_cli(&dir, &["mv", WELCOME, WELCOME2]);
    git::checkpoint(&dir, "moved").await.unwrap().unwrap();

    let log = git::file_log(&dir, WELCOME2).await.unwrap();
    assert_eq!(log[0].summary, "moved");
    assert_eq!(log[0].path, WELCOME2);
    let old = log.iter().find(|v| v.summary == "before").unwrap();
    assert_eq!(old.path, WELCOME);
    assert!(git::file_at(&dir, &old.sha, &old.path)
        .await
        .unwrap()
        .contains("Before the move."));
}

#[tokio::test]
async fn file_log_is_empty_for_untracked_file() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    std::fs::write(dir.join("src/content/blog/en/new.md"), "new\n").unwrap();
    assert_eq!(
        git::file_log(&dir, "src/content/blog/en/new.md")
            .await
            .unwrap(),
        vec![]
    );
}

#[tokio::test]
async fn file_log_skips_deletion_commits() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    append(&dir, WELCOME, "\nFirst life.\n");
    git::checkpoint(&dir, "first").await.unwrap().unwrap();
    std::fs::remove_file(dir.join(WELCOME)).unwrap();
    git::checkpoint(&dir, "deleted").await.unwrap().unwrap();
    append(&dir, WELCOME, "Second life.\n");
    git::checkpoint(&dir, "recreated").await.unwrap().unwrap();

    let log = git::file_log(&dir, WELCOME).await.unwrap();
    assert_eq!(log[0].summary, "recreated");
    assert!(log.iter().all(|v| v.summary != "deleted"));
    for v in &log {
        assert!(
            git::file_at(&dir, &v.sha, &v.path).await.is_ok(),
            "{}",
            v.summary
        );
    }
}

#[tokio::test]
async fn file_at_rejects_option_and_parent_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    let head = git_cli(&dir, &["rev-parse", "HEAD"]);
    for path in ["-x", "../x", "src/../x"] {
        let err = git::file_at(&dir, &head, path).await.unwrap_err();
        assert_eq!(err.http_status(), 400, "{path}");
    }
    if let Err(err) = git::file_at(&dir, &head, "src/a..b.md").await {
        assert_ne!(err.http_status(), 400);
    }
}

#[tokio::test]
async fn file_at_rejects_non_hex_sha() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    let err = git::file_at(&dir, "HEAD~1", WELCOME).await.unwrap_err();
    assert_eq!(err.http_status(), 400);
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Content endpoints run the content helper (node), which needs the template's modules.
fn link_node_modules(dir: &Path) {
    let modules =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/astro-marketing/node_modules");
    std::os::unix::fs::symlink(modules, dir.join("node_modules")).unwrap();
    let exclude = dir.join(".git/info/exclude");
    let mut text = std::fs::read_to_string(&exclude).unwrap_or_default();
    text.push_str("\n/node_modules\n");
    std::fs::write(exclude, text).unwrap();
}

/// A project on a change branch, ready for the content endpoints; returns its ID too.
async fn content_project(tmp: &Path) -> (axum::Router, std::path::PathBuf, String) {
    let (app, dir) = project(tmp).await;
    common::write_content_manifest(&dir, "");
    link_node_modules(&dir);
    changes::create(&dir, "Edit content").await.unwrap();
    let id = dir.file_name().unwrap().to_string_lossy().to_string();
    (app, dir, id)
}

fn set_title(dir: &Path, title: &str) {
    let file = dir.join(WELCOME);
    let text = std::fs::read_to_string(&file).unwrap();
    let text = text
        .lines()
        .map(|l| {
            if l.starts_with("title:") {
                format!("title: {title}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(file, text + "\n").unwrap();
}

fn history_uri(id: &str, entry: &str, sha: Option<&str>) -> String {
    let base = match sha {
        Some(sha) => format!("/api/content/history/{sha}"),
        None => "/api/content/history".to_string(),
    };
    format!("{base}?project={id}&collection=blog&id={entry}")
}

#[tokio::test]
async fn history_endpoint_lists_versions() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir, id) = content_project(tmp.path()).await;
    set_title(&dir, "Edited");
    git::checkpoint(&dir, "edit welcome")
        .await
        .unwrap()
        .unwrap();

    let (s, v) = call(&app, req("GET", &history_uri(&id, "welcome", None), None)).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["versions"][0]["summary"], "edit welcome");
    assert!(v["versions"][0].get("published").is_some(), "{v}");
}

#[tokio::test]
async fn version_endpoint_returns_parsed_old_content() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir, id) = content_project(tmp.path()).await;
    set_title(&dir, "Old");
    let old = git::checkpoint(&dir, "old").await.unwrap().unwrap();
    set_title(&dir, "New");
    git::checkpoint(&dir, "new").await.unwrap().unwrap();

    let short = &old.sha[..10];
    let (s, v) = call(
        &app,
        req("GET", &history_uri(&id, "welcome", Some(short)), None),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["sha"], old.sha);
    assert_eq!(v["path"], WELCOME);
    assert_eq!(v["fields"]["title"], "Old");
    assert_eq!(v["parseable"], true);
    assert!(v["source"].as_str().unwrap().contains("Old"));
    assert!(v["body"]
        .as_str()
        .unwrap()
        .contains("We're glad you're here"));
}

#[tokio::test]
async fn version_endpoint_404s_for_commit_not_touching_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir, id) = content_project(tmp.path()).await;
    append(&dir, "README.md", "\nOther.\n");
    let other = git::checkpoint(&dir, "other").await.unwrap().unwrap();

    let (s, v) = call(
        &app,
        req("GET", &history_uri(&id, "welcome", Some(&other.sha)), None),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
}

#[tokio::test]
async fn version_endpoint_returns_old_content_after_rename() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir, id) = content_project(tmp.path()).await;
    set_title(&dir, "Before rename");
    let before = git::checkpoint(&dir, "before").await.unwrap().unwrap();

    let entry_uri = format!("/api/content/entry?project={id}&collection=blog&id=welcome");
    let (s, entry) = call(&app, req("GET", &entry_uri, None)).await;
    assert_eq!(s, StatusCode::OK, "{entry}");
    let body = json!({"project":id,"collection":"blog","id":"welcome","slug":"welcome2",
        "branch":entry["branch"],"hash":entry["hash"]});
    let (s, v) = call(&app, req("POST", "/api/content/entry/rename", Some(body))).await;
    assert_eq!(s, StatusCode::OK, "{v}");

    let (s, v) = call(
        &app,
        req(
            "GET",
            &history_uri(&id, "welcome2", Some(&before.sha)),
            None,
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["path"], WELCOME);
    assert_eq!(v["fields"]["title"], "Before rename");
}

#[tokio::test]
async fn history_requires_collection_and_id() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, _dir, id) = content_project(tmp.path()).await;
    let uri = format!("/api/content/history?project={id}&collection=blog");
    let (s, v) = call(&app, req("GET", &uri, None)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
}
