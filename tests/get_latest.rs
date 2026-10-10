//! Get latest: fast-forward `main` from `origin`, merge it into every open change branch.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use workbench::config::Config;
use workbench::error::ErrorCode;
use workbench::http;
use workbench::state::AppState;
use workbench::{changes, git};

fn git_cli(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
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

fn commit_file(dir: &Path, name: &str, content: &str, msg: &str) {
    std::fs::write(dir.join(name), content).unwrap();
    git_cli(dir, &["add", "-A"]);
    git_cli(dir, &["commit", "-m", msg]);
}

/// `tmp/work` cloned from `tmp/remote.git` (seeded with README.md), authored as the writer.
fn clone_with_origin(tmp: &Path) -> PathBuf {
    let src = tmp.join("src");
    std::fs::create_dir_all(&src).unwrap();
    git_cli(&src, &["init", "-b", "main"]);
    git_cli(&src, &["config", "user.name", "Grace"]);
    git_cli(&src, &["config", "user.email", "grace@example.com"]);
    commit_file(&src, "README.md", "hello\n", "init");
    git_cli(tmp, &["clone", "--bare", src.to_str().unwrap(), "remote.git"]);
    let work = tmp.join("work");
    git_cli(tmp, &["clone", "remote.git", "work"]);
    git_cli(&work, &["config", "user.name", "Ada Writer"]);
    git_cli(&work, &["config", "user.email", "ada@example.com"]);
    work
}

/// Another person lands `name` on the remote `main`; returns the new remote tip.
fn push_to_remote_main(tmp: &Path, name: &str, content: &str) -> String {
    let push = tmp.join("push");
    if !push.exists() {
        git_cli(tmp, &["clone", "remote.git", "push"]);
        git_cli(&push, &["config", "user.name", "Grace"]);
        git_cli(&push, &["config", "user.email", "grace@example.com"]);
    }
    git_cli(&push, &["pull", "--ff-only", "origin", "main"]);
    commit_file(&push, name, content, &format!("add {name}"));
    git_cli(&push, &["push", "origin", "main"]);
    git_cli(&tmp.join("remote.git"), &["rev-parse", "refs/heads/main"])
}

/// Start a change and commit `name` on it.
async fn change_with_edit(work: &Path, title: &str, name: &str, content: &str) -> String {
    let change = changes::create(work, title).await.unwrap();
    commit_file(work, name, content, &format!("edit {name}"));
    change.branch
}

fn rev(dir: &Path, r: &str) -> String {
    git_cli(dir, &["rev-parse", r])
}

fn is_ancestor(dir: &Path, a: &str, b: &str) -> bool {
    Command::new("git")
        .args(["merge-base", "--is-ancestor", a, b])
        .current_dir(dir)
        .status()
        .unwrap()
        .success()
}

#[tokio::test]
async fn fast_forwards_main() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let remote_main = push_to_remote_main(tmp.path(), "news.md", "fresh\n");

    let update = changes::get_latest(&work).await.unwrap();

    assert!(update.main_updated);
    assert!(update.updated.is_empty());
    assert!(update.conflicts.is_empty());
    assert_eq!(rev(&work, "main"), remote_main);
    // On main, the working tree follows.
    assert_eq!(git_cli(&work, &["branch", "--show-current"]), "main");
    assert!(work.join("news.md").exists());
}

#[tokio::test]
async fn merges_main_into_each_change_as_the_user() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let first = change_with_edit(&work, "First", "first.md", "one\n").await;
    let second = change_with_edit(&work, "Second", "second.md", "two\n").await;
    let first_before = rev(&work, &first);
    let second_before = rev(&work, &second);
    let remote_main = push_to_remote_main(tmp.path(), "news.md", "fresh\n");

    let update = changes::get_latest(&work).await.unwrap();

    assert!(update.main_updated);
    let mut updated = update.updated.clone();
    updated.sort();
    assert_eq!(updated, vec![first.clone(), second.clone()]);
    assert!(update.conflicts.is_empty());
    assert_eq!(rev(&work, "main"), remote_main);

    for (branch, before) in [(&first, &first_before), (&second, &second_before)] {
        // A merge commit: the branch's own tip and main as parents (no rebase).
        let parents = git_cli(&work, &["rev-list", "--parents", "-n", "1", branch]);
        let parents: Vec<&str> = parents.split(' ').collect();
        assert_eq!(parents.len(), 3, "{branch}: {parents:?}");
        assert_eq!(parents[1], before.as_str());
        assert_eq!(parents[2], remote_main.as_str());
        assert_eq!(
            git_cli(&work, &["log", "-1", "--format=%an <%ae>|%cn <%ce>", branch]),
            "Ada Writer <ada@example.com>|Ada Writer <ada@example.com>"
        );
        assert_eq!(git_cli(&work, &["show", &format!("{branch}:news.md")]), "fresh");
    }
    assert_eq!(git_cli(&work, &["show", &format!("{first}:first.md")]), "one");

    // Active change stays checked out, and its working tree has the new file.
    assert_eq!(git_cli(&work, &["branch", "--show-current"]), second);
    assert!(work.join("news.md").exists());
    assert!(work.join("second.md").exists());
    assert_eq!(
        git_cli(&work, &["status", "--porcelain", "--untracked-files=no"]),
        ""
    );
}

#[tokio::test]
async fn already_up_to_date_is_a_no_op() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = change_with_edit(&work, "First", "first.md", "one\n").await;
    let main_before = rev(&work, "main");
    let tip_before = rev(&work, &branch);

    let update = changes::get_latest(&work).await.unwrap();

    assert!(!update.main_updated);
    assert!(update.updated.is_empty());
    assert!(update.conflicts.is_empty());
    assert_eq!(rev(&work, "main"), main_before);
    assert_eq!(rev(&work, &branch), tip_before);

    // Second run after an update is also a no-op.
    push_to_remote_main(tmp.path(), "news.md", "fresh\n");
    changes::get_latest(&work).await.unwrap();
    let tip = rev(&work, &branch);
    let again = changes::get_latest(&work).await.unwrap();
    assert!(!again.main_updated);
    assert!(again.updated.is_empty());
    assert_eq!(rev(&work, &branch), tip);
}

#[tokio::test]
async fn refuses_uncommitted_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = change_with_edit(&work, "First", "first.md", "one\n").await;
    let main_before = rev(&work, "main");
    let tip_before = rev(&work, &branch);
    push_to_remote_main(tmp.path(), "news.md", "fresh\n");
    std::fs::write(work.join("first.md"), "unsaved\n").unwrap();

    let err = changes::get_latest(&work).await.unwrap_err();

    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(
        err.message.to_lowercase().contains("checkpoint"),
        "expected checkpoint-first message, got: {}",
        err.message
    );
    assert_eq!(rev(&work, "main"), main_before);
    assert_eq!(rev(&work, &branch), tip_before);
    assert_eq!(std::fs::read_to_string(work.join("first.md")).unwrap(), "unsaved\n");
}

#[tokio::test]
async fn conflicting_changes_are_reported_and_left_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let clash = change_with_edit(&work, "Clash", "README.md", "mine\n").await;
    let clean = change_with_edit(&work, "Clean", "clean.md", "ok\n").await;
    let clash_before = rev(&work, &clash);
    let remote_main = push_to_remote_main(tmp.path(), "README.md", "theirs\n");

    let update = changes::get_latest(&work).await.unwrap();

    assert!(update.main_updated);
    assert_eq!(update.updated, vec![clean.clone()]);
    assert_eq!(update.conflicts, vec![clash.clone()]);
    assert_eq!(rev(&work, &clash), clash_before);
    assert!(is_ancestor(&work, &remote_main, &clean));
    assert_eq!(git_cli(&work, &["branch", "--show-current"]), clean);
    assert_eq!(
        git_cli(&work, &["status", "--porcelain", "--untracked-files=no"]),
        ""
    );
}

#[tokio::test]
async fn local_only_project_merges_local_main() {
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("local");
    std::fs::create_dir_all(&work).unwrap();
    git_cli(&work, &["init", "-b", "main"]);
    git_cli(&work, &["config", "user.name", "Ada Writer"]);
    git_cli(&work, &["config", "user.email", "ada@example.com"]);
    commit_file(&work, "README.md", "hello\n", "init");
    let branch = change_with_edit(&work, "First", "first.md", "one\n").await;
    git_cli(&work, &["checkout", "main"]);
    commit_file(&work, "news.md", "fresh\n", "news");
    git_cli(&work, &["checkout", &branch]);

    let update = changes::get_latest(&work).await.unwrap();

    assert!(!update.main_updated);
    assert_eq!(update.updated, vec![branch.clone()]);
    assert!(work.join("news.md").exists());
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
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn http_update_route() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let st = AppState::with_scaffold_install(config(&projects), false);
    let info = st.projects.create("Site").await.unwrap();
    let dir = projects.join(&info.id);
    git::set_identity(
        &dir,
        &git::Identity {
            name: "Ada Writer".into(),
            email: "ada@example.com".into(),
        },
    )
    .await
    .unwrap();
    let branch = change_with_edit(&dir, "First", "first.md", "one\n").await;
    git_cli(&dir, &["checkout", "main"]);
    commit_file(&dir, "news.md", "fresh\n", "news");
    git_cli(&dir, &["checkout", &branch]);

    let app = http::router(st);
    let (s, body) = post(&app, "/api/changes/update", json!({ "project": info.id })).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["mainUpdated"], false);
    assert_eq!(body["updated"], json!([branch]));
    assert_eq!(body["conflicts"], json!([]));

    // Dirty tree → 409 with a message.
    std::fs::write(dir.join("first.md"), "unsaved\n").unwrap();
    let (s, body) = post(&app, "/api/changes/update", json!({ "project": info.id })).await;
    assert_eq!(s, StatusCode::CONFLICT, "{body}");

    let (s, _) = post(&app, "/api/changes/update", json!({})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}
