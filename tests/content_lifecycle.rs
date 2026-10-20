//! Task 7: rename and delete content entries with references, owned assets, and checkpoints.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use orso::config::Config;
use orso::state::AppState;
use orso::{changes, http};

const WELCOME: &str = "src/content/blog/en/welcome.md";
const HELLO: &str = "src/content/blog/en/hello.md";
const LAUNCH: &str = "src/content/blog/en/launch.md";
const COVER: &str = "src/assets/blog/welcome/cover.png";
const DIAGRAM: &str = "src/assets/blog/welcome/diagram.png";

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

/// Each test spawns many content-helper (node) processes; running all of them at once
/// starves the helper of CPU past its 10 s timeout, so cap how many run together.
static HELPER_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(3);

struct Env {
    _slot: tokio::sync::SemaphorePermit<'static>,
    _tmp: tempfile::TempDir,
    state: AppState,
    app: axum::Router,
    id: String,
    dir: PathBuf,
    branch: String,
}

const WELCOME_SOURCE: &str = "---
title: Welcome to the blog
description: News, ideas, and updates from the Acme team.
pubDate: \"2026-10-11\"
coverImage: ../../../assets/blog/welcome/cover.png
coverAlt: A cover
tags: [news]
---

## Welcome

![Diagram](../../../assets/blog/welcome/diagram.png)
";

const LAUNCH_SOURCE: &str = "---
title: Launch
description: Launch notes
pubDate: \"2026-10-12\"
# Related reading stays commented.
relatedPosts: [welcome]
---
Read [the welcome post](/blog/welcome) first.
";

/// A template project on a change branch with a blog post that owns assets and a post
/// that references it, all committed.
async fn env(change: bool) -> Env {
    let slot = HELPER_SLOTS.acquire().await.unwrap();
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
        changes::create(&dir, "Edit content").await.unwrap();
    }
    std::fs::create_dir_all(dir.join("src/assets/blog/welcome")).unwrap();
    std::fs::write(dir.join(COVER), b"cover").unwrap();
    std::fs::write(dir.join(DIAGRAM), b"diagram").unwrap();
    std::fs::write(dir.join(WELCOME), WELCOME_SOURCE).unwrap();
    std::fs::write(dir.join(LAUNCH), LAUNCH_SOURCE).unwrap();
    git_cli(&dir, &["add", "-A", "src"]);
    git_cli(&dir, &["commit", "-qm", "fixture"]);
    let branch = git_cli(&dir, &["branch", "--show-current"]);
    Env {
        _slot: slot,
        app: http::router(state.clone()),
        _tmp: tmp,
        state,
        id: info.id,
        dir,
        branch,
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

async fn read(e: &Env, collection: &str, id: &str) -> Value {
    let (s, v) = call(
        &e.app,
        "GET",
        &format!(
            "/api/content/entry?project={}&collection={collection}&id={id}",
            e.id
        ),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v
}

async fn rename(e: &Env, collection: &str, id: &str, slug: &str) -> (StatusCode, Value) {
    let hash = read(e, collection, id).await["hash"].clone();
    call(
        &e.app,
        "POST",
        "/api/content/entry/rename",
        Some(json!({"project":e.id,"collection":collection,"id":id,"slug":slug,"branch":e.branch,"hash":hash})),
    )
    .await
}

async fn delete(e: &Env, collection: &str, id: &str) -> (StatusCode, Value) {
    let hash = read(e, collection, id).await["hash"].clone();
    call(
        &e.app,
        "DELETE",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":collection,"id":id,"branch":e.branch,"hash":hash})),
    )
    .await
}

async fn impact(e: &Env, collection: &str, id: &str) -> Value {
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry/impact",
        Some(json!({"project":e.id,"collection":collection,"id":id})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v
}

fn head(dir: &Path) -> String {
    git_cli(dir, &["rev-parse", "HEAD"])
}

fn committed(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = git_cli(
        dir,
        &["show", "--format=", "--name-only", "--no-renames", "HEAD"],
    )
    .lines()
    .map(str::to_string)
    .collect();
    v.sort();
    v
}

/// Every file under the project's `src` tree with its bytes.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if meta.file_type().is_symlink() {
                out.push((rel, b"<symlink>".to_vec()));
            } else if meta.is_dir() {
                walk(root, &path, out);
            } else {
                out.push((rel, std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = vec![];
    walk(dir, &dir.join("src"), &mut out);
    out.sort();
    out
}

#[tokio::test]
async fn rename_and_delete_require_an_active_change() {
    let e = env(false).await;
    let before = snapshot(&e.dir);
    let (s, v) = rename(&e, "blog", "launch", "renamed").await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["code"], "CHANGE_REQUIRED");
    let (s, v) = delete(&e, "blog", "launch").await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["code"], "CHANGE_REQUIRED");
    assert_eq!(snapshot(&e.dir), before);
}

#[tokio::test]
async fn impact_reports_schema_references_owned_assets_and_prose_links() {
    let e = env(true).await;
    let v = impact(&e, "blog", "welcome").await;
    assert_eq!(
        v["references"],
        json!([{"collection":"blog","id":"launch","path":LAUNCH,"field":"relatedPosts"}])
    );
    assert_eq!(v["assets"], json!([COVER, DIAGRAM]));
    assert_eq!(
        v["proseLinks"],
        json!([{"collection":"blog","id":"launch","path":LAUNCH,"url":"/blog/welcome","line":8}])
    );
}

#[tokio::test]
async fn rename_rewrites_references_and_owned_assets_in_one_reversible_checkpoint() {
    let e = env(true).await;
    // Unrelated staged and unstaged work must survive untouched.
    std::fs::write(e.dir.join("README.md"), "staged\n").unwrap();
    git_cli(&e.dir, &["add", "README.md"]);
    std::fs::write(e.dir.join("README.md"), "staged then unstaged\n").unwrap();
    std::fs::write(e.dir.join("astro.config.mjs"), "// unstaged\n").unwrap();
    let staged = git_cli(&e.dir, &["diff", "--cached", "--binary"]);
    let unstaged = git_cli(&e.dir, &["diff", "--binary"]);
    let base = head(&e.dir);
    let mut events = e.state.events.subscribe();

    let (s, v) = rename(&e, "blog", "welcome", "hello").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["entry"]["id"], "hello");
    assert_eq!(v["entry"]["path"], HELLO);
    assert_eq!(v["entry"]["branch"], e.branch.as_str());
    let ids: Vec<_> = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        ids.contains(&"hello".into()) && !ids.contains(&"welcome".into()),
        "{ids:?}"
    );

    assert!(!e.dir.join(WELCOME).exists());
    assert_eq!(
        std::fs::read_to_string(e.dir.join(HELLO)).unwrap(),
        WELCOME_SOURCE.replace("assets/blog/welcome/", "assets/blog/hello/")
    );
    assert!(!e.dir.join("src/assets/blog/welcome").exists());
    assert_eq!(
        std::fs::read(e.dir.join("src/assets/blog/hello/cover.png")).unwrap(),
        b"cover"
    );
    // The reference is rewritten in place; the comment and prose link are preserved.
    assert_eq!(
        std::fs::read_to_string(e.dir.join(LAUNCH)).unwrap(),
        LAUNCH_SOURCE.replace("[welcome]", "[hello]")
    );

    assert_eq!(git_cli(&e.dir, &["rev-parse", "HEAD^"]), base);
    assert_eq!(
        committed(&e.dir),
        vec![
            "src/assets/blog/hello/cover.png",
            "src/assets/blog/hello/diagram.png",
            COVER,
            DIAGRAM,
            HELLO,
            LAUNCH,
            WELCOME,
        ]
    );
    assert_eq!(git_cli(&e.dir, &["diff", "--cached", "--binary"]), staged);
    assert_eq!(git_cli(&e.dir, &["diff", "--binary"]), unstaged);
    let mut kinds = vec![];
    while let Ok(text) = events.try_recv() {
        let v: Value = serde_json::from_str(&text).unwrap();
        kinds.push(v["type"].as_str().unwrap().to_string());
    }
    assert!(kinds.contains(&"fs.change".into()), "{kinds:?}");
    assert!(kinds.contains(&"checkpoint.created".into()), "{kinds:?}");

    // History restore of the previous checkpoint reverses the rename.
    let (s, v) = call(
        &e.app,
        "POST",
        &format!("/api/checkpoints/{base}/restore"),
        Some(json!({"project":e.id})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        std::fs::read_to_string(e.dir.join(WELCOME)).unwrap(),
        WELCOME_SOURCE
    );
    assert_eq!(
        std::fs::read_to_string(e.dir.join(LAUNCH)).unwrap(),
        LAUNCH_SOURCE
    );
    assert!(e.dir.join(COVER).exists());
    assert!(!e.dir.join(HELLO).exists());
}

#[tokio::test]
async fn delete_refuses_referenced_entries_and_removes_entry_and_assets_reversibly() {
    let e = env(true).await;
    let before = snapshot(&e.dir);
    let base = head(&e.dir);
    let (s, v) = delete(&e, "blog", "welcome").await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "entry_referenced");
    assert_eq!(v["error"]["details"][0]["id"], "launch");
    assert_eq!(snapshot(&e.dir), before);
    assert_eq!(head(&e.dir), base);

    // Once unreferenced, delete removes the entry and its owned assets in one checkpoint.
    let (s, v) = delete(&e, "blog", "launch").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, v) = delete(&e, "blog", "welcome").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let ids: Vec<_> = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].clone())
        .collect();
    assert!(!ids.contains(&json!("welcome")), "{ids:?}");
    assert!(!e.dir.join(WELCOME).exists());
    assert!(!e.dir.join("src/assets/blog/welcome").exists());
    assert_eq!(committed(&e.dir), vec![COVER, DIAGRAM, WELCOME]);
    assert_eq!(git_cli(&e.dir, &["status", "--porcelain", "--", "src"]), "");
    let leftovers: Vec<_> = std::fs::read_dir(e.dir.join("src/assets/blog"))
        .unwrap()
        .map(|x| x.unwrap().file_name())
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    let previous = git_cli(&e.dir, &["rev-parse", "HEAD^"]);
    let (s, v) = call(
        &e.app,
        "POST",
        &format!("/api/checkpoints/{previous}/restore"),
        Some(json!({"project":e.id})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        std::fs::read_to_string(e.dir.join(WELCOME)).unwrap(),
        WELCOME_SOURCE
    );
    assert_eq!(std::fs::read(e.dir.join(COVER)).unwrap(), b"cover");
}

#[tokio::test]
async fn rename_is_all_or_nothing_when_a_reference_fails_validation() {
    let e = env(true).await;
    // An invalid entry (missing description) that references the renamed post.
    std::fs::write(
        e.dir.join("src/content/blog/en/broken.md"),
        "---\ntitle: Broken\npubDate: \"2026-10-12\"\nrelatedPosts: [welcome]\n---\n",
    )
    .unwrap();
    git_cli(&e.dir, &["add", "-A", "src"]);
    git_cli(&e.dir, &["commit", "-qm", "broken"]);
    let before = snapshot(&e.dir);
    let base = head(&e.dir);
    let (s, v) = rename(&e, "blog", "welcome", "hello").await;
    assert_eq!(s.as_u16(), 422, "{v}");
    assert_eq!(v["error"]["reason"], "invalid_schema");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("blog/broken"),
        "{v}"
    );
    assert_eq!(snapshot(&e.dir), before);
    assert_eq!(head(&e.dir), base);
}

#[tokio::test]
async fn failed_checkpoint_rolls_back_rename_and_delete() {
    let e = env(true).await;
    let before = snapshot(&e.dir);
    let base = head(&e.dir);
    // A held Git index lock makes the scoped checkpoint fail after the files moved.
    let lock = e.dir.join(".git/index.lock");
    std::fs::write(&lock, b"").unwrap();
    let (s, v) = rename(&e, "blog", "welcome", "hello").await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR, "{v}");
    assert_eq!(v["error"]["code"], "GIT_ERR", "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("cannot lock project index"),
        "{v}"
    );
    // A clean rollback reports the checkpoint failure itself, not `rollback_failed`.
    assert!(v["error"].get("reason").is_none(), "{v}");
    assert_eq!(snapshot(&e.dir), before);
    let (s, v) = delete(&e, "blog", "launch").await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR, "{v}");
    assert_eq!(v["error"]["code"], "GIT_ERR", "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("cannot lock project index"),
        "{v}"
    );
    // A clean rollback reports the checkpoint failure itself, not `rollback_failed`.
    assert!(v["error"].get("reason").is_none(), "{v}");
    assert_eq!(snapshot(&e.dir), before);
    std::fs::remove_file(&lock).unwrap();
    assert_eq!(head(&e.dir), base);
    assert_eq!(git_cli(&e.dir, &["status", "--porcelain", "--", "src"]), "");
}

#[tokio::test]
async fn rename_rejects_stale_branch_duplicate_and_invalid_slugs() {
    let e = env(true).await;
    let before = snapshot(&e.dir);
    let entry = read(&e, "blog", "welcome").await;
    let mut body = json!({"project":e.id,"collection":"blog","id":"welcome","slug":"hello","branch":e.branch,"hash":"0"});
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry/rename",
        Some(body.clone()),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "stale_content");
    body["hash"] = entry["hash"].clone();
    body["branch"] = json!("content/other");
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry/rename",
        Some(body.clone()),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "branch_changed");
    let mut delete_body = body.clone();
    delete_body.as_object_mut().unwrap().remove("slug");
    let (s, v) = call(&e.app, "DELETE", "/api/content/entry", Some(delete_body)).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "branch_changed");

    let (s, v) = rename(&e, "blog", "welcome", "launch").await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "duplicate_slug");
    for slug in ["Bad Slug", "../escape", "welcome"] {
        let (s, v) = rename(&e, "blog", "welcome", slug).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{slug}: {v}");
        assert_eq!(v["error"]["reason"], "invalid_slug", "{slug}: {v}");
    }
    assert_eq!(snapshot(&e.dir), before);
}

#[tokio::test]
async fn symlinked_owned_asset_directory_is_rejected_without_touching_outside() {
    let e = env(true).await;
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.png"), b"secret").unwrap();
    let owned = e.dir.join("src/assets/blog/launch");
    std::os::unix::fs::symlink(outside.path(), &owned).unwrap();
    let before = snapshot(&e.dir);
    let base = head(&e.dir);

    let (s, v) = rename(&e, "blog", "launch", "renamed").await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["error"]["reason"], "unsafe_path");
    let (s, v) = delete(&e, "blog", "launch").await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["error"]["reason"], "unsafe_path");
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry/impact",
        Some(json!({"project":e.id,"collection":"blog","id":"launch"})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");

    // A symlink inside an owned asset directory is refused too.
    std::fs::remove_file(&owned).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.png"),
        e.dir.join("src/assets/blog/welcome/link.png"),
    )
    .unwrap();
    let (s, v) = rename(&e, "blog", "welcome", "hello").await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["error"]["reason"], "unsafe_path");
    std::fs::remove_file(e.dir.join("src/assets/blog/welcome/link.png")).unwrap();
    std::os::unix::fs::symlink(outside.path(), &owned).unwrap();

    assert_eq!(snapshot(&e.dir), before);
    assert_eq!(head(&e.dir), base);
    let names: Vec<_> = std::fs::read_dir(outside.path())
        .unwrap()
        .map(|x| x.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("secret.png")]);
    assert_eq!(
        std::fs::read(outside.path().join("secret.png")).unwrap(),
        b"secret"
    );
}
