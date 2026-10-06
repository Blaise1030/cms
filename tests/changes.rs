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
async fn create_refuses_dirty_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    // Modify a tracked file (untracked alone must not block create).
    std::fs::write(work.join("README.md"), "dirty tracked\n").unwrap();

    let err = changes::create(&work, "New post: Launch week")
        .await
        .unwrap_err();
    assert_eq!(err.code, workbench::error::ErrorCode::Conflict);
    assert!(
        err.message.to_lowercase().contains("checkpoint"),
        "expected checkpoint-first message, got: {}",
        err.message
    );
    assert_eq!(
        git_cli(&work, &["branch", "--show-current"]),
        "main",
        "must stay on current branch"
    );
    assert!(
        git_cli(&work, &["branch", "--list", "content/launch-week"]).is_empty(),
        "must not create the branch"
    );

    // Untracked files alone are fine.
    std::fs::write(work.join("README.md"), "hello\n").unwrap();
    std::fs::write(work.join("untracked.md"), "only untracked\n").unwrap();
    let change = changes::create(&work, "New post: Launch week")
        .await
        .unwrap();
    assert_eq!(change.branch, "content/launch-week");
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

fn commit_file(dir: &Path, name: &str, content: &str, msg: &str) {
    std::fs::write(dir.join(name), content).unwrap();
    git_cli(dir, &["add", "-A"]);
    git_cli(dir, &["commit", "-m", msg]);
}

/// Commit with a fixed committer date so newest-first ordering is stable in tests.
fn commit_file_at(dir: &Path, name: &str, content: &str, msg: &str, unix: i64) {
    std::fs::write(dir.join(name), content).unwrap();
    git_cli(dir, &["add", "-A"]);
    let date = format!("@{unix}");
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", "commit", "-m", msg])
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git commit: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[tokio::test]
async fn list_open_content_branches_newest_first() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());

    // Empty / tip==main change is treated as merged — not listed.
    changes::create(&work, "New post: Empty").await.unwrap();
    assert!(changes::list(&work).await.unwrap().is_empty());

    // Older diverged change.
    changes::create(&work, "New post: Older").await.unwrap();
    commit_file_at(&work, "older.md", "a\n", "older edit", 1_700_000_000);

    // Newer diverged change.
    changes::create(&work, "New post: Newer").await.unwrap();
    commit_file_at(&work, "newer.md", "b\n", "newer edit", 1_700_000_100);

    // Merged into main → excluded.
    git_cli(&work, &["checkout", "main"]);
    git_cli(&work, &["merge", "--ff-only", "content/older"]);
    let listed = changes::list(&work).await.unwrap();
    assert_eq!(
        listed.iter().map(|c| c.branch.as_str()).collect::<Vec<_>>(),
        vec!["content/newer"]
    );
    assert_eq!(listed[0].title, "Newer");
    assert_eq!(listed[0].last_edited, Some(1_700_000_100));

    // Second open change; tip dates force newest-first order.
    changes::create(&work, "New post: Mid").await.unwrap();
    commit_file_at(&work, "mid.md", "c\n", "mid edit", 1_700_000_050);
    git_cli(&work, &["checkout", "content/newer"]);
    commit_file_at(&work, "newer2.md", "d\n", "newer again", 1_700_000_200);

    let listed = changes::list(&work).await.unwrap();
    let branches: Vec<_> = listed.iter().map(|c| c.branch.as_str()).collect();
    assert_eq!(branches, vec!["content/newer", "content/mid"]);
    assert_eq!(listed[0].last_edited, Some(1_700_000_200));
    assert_eq!(listed[1].last_edited, Some(1_700_000_050));
}

#[tokio::test]
async fn switch_checks_out_and_refuses_dirty() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());

    changes::create(&work, "New post: Alpha").await.unwrap();
    commit_file(&work, "alpha.md", "a\n", "alpha");
    changes::create(&work, "New post: Beta").await.unwrap();
    commit_file(&work, "beta.md", "b\n", "beta");

    let switched = changes::switch(&work, "content/alpha").await.unwrap();
    assert_eq!(switched.branch, "content/alpha");
    assert_eq!(switched.title, "Alpha");
    assert_eq!(
        git_cli(&work, &["branch", "--show-current"]),
        "content/alpha"
    );
    assert!(work.join("alpha.md").exists());
    assert!(!work.join("beta.md").exists());

    std::fs::write(work.join("alpha.md"), "dirty\n").unwrap();
    let err = changes::switch(&work, "content/beta")
        .await
        .unwrap_err();
    assert_eq!(err.code, workbench::error::ErrorCode::Conflict);
    assert!(
        err.message.to_lowercase().contains("checkpoint"),
        "expected checkpoint-first message, got: {}",
        err.message
    );
    assert_eq!(
        git_cli(&work, &["branch", "--show-current"]),
        "content/alpha",
        "must stay on current branch"
    );
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

    // Diverged tip so list includes it.
    std::fs::write(dir.join("post.md"), "hi\n").unwrap();
    git_cli(&dir, &["add", "-A"]);
    git_cli(&dir, &["commit", "-m", "post"]);

    let (s, body) = call(
        &app,
        req(
            "GET",
            &format!("/api/changes?project={}", info.id),
            None,
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["changes"][0]["branch"], "content/launch-week");
    assert_eq!(body["changes"][0]["title"], "Launch week");
    assert!(body["changes"][0]["lastEdited"].as_i64().unwrap() > 0);

    // Second change, then switch back.
    let (s, body) = call(
        &app,
        req(
            "POST",
            "/api/changes",
            Some(json!({ "project": info.id, "title": "Other" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    std::fs::write(dir.join("other.md"), "x\n").unwrap();
    git_cli(&dir, &["add", "-A"]);
    git_cli(&dir, &["commit", "-m", "other"]);

    let (s, body) = call(
        &app,
        req(
            "POST",
            "/api/changes/content%2Flaunch-week/switch",
            Some(json!({ "project": info.id })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["branch"], "content/launch-week");
    assert_eq!(
        git_cli(&dir, &["branch", "--show-current"]),
        "content/launch-week"
    );
}
