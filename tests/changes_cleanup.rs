//! Clean up a merged change: fast-forward `main`, then delete only the local branch.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use workbench::changes;
use workbench::config::Config;
use workbench::error::ErrorCode;
use workbench::http;
use workbench::state::AppState;

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

/// `tmp/work` cloned from the bare `tmp/remote.git`.
fn clone_with_origin(tmp: &Path) -> PathBuf {
    let src = tmp.join("src");
    seed_repo(&src);
    git_cli(tmp, &["clone", "--bare", src.to_str().unwrap(), "remote.git"]);
    git_cli(tmp, &["clone", "remote.git", "work"]);
    tmp.join("work")
}

fn commit_file(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
    git_cli(dir, &["add", name]);
    git_cli(dir, &["commit", "-m", &format!("checkpoint: {name}")]);
}

/// A change branch with `file` committed on it, pushed to `origin`.
async fn shared_change(work: &Path, title: &str, file: &str) -> String {
    let change = changes::create(work, title).await.unwrap();
    commit_file(work, file, &format!("{title}\n"));
    changes::push(work, &change.branch, &[]).await.unwrap();
    change.branch
}

/// What merging the PR on GitHub does to the remote's `main`.
fn merge_on_remote(tmp: &Path, branch: &str, squash: bool) {
    let reviewer = tmp.join("reviewer");
    if !reviewer.exists() {
        git_cli(tmp, &["clone", "remote.git", "reviewer"]);
    }
    git_cli(&reviewer, &["fetch", "origin"]);
    git_cli(&reviewer, &["checkout", "main"]);
    git_cli(&reviewer, &["merge", "--ff-only", "origin/main"]);
    let theirs = format!("origin/{branch}");
    if squash {
        git_cli(&reviewer, &["merge", "--squash", &theirs]);
        git_cli(&reviewer, &["commit", "-m", &format!("{branch} (#1)")]);
    } else {
        git_cli(&reviewer, &["merge", "--no-ff", &theirs, "-m", "merge"]);
    }
    git_cli(&reviewer, &["push", "origin", "main"]);
}

fn local_branch_exists(dir: &Path, branch: &str) -> bool {
    !git_cli(dir, &["branch", "--list", branch]).is_empty()
}

#[tokio::test]
async fn cleanup_refuses_unmerged_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Draft", "draft.md").await;

    let err = changes::cleanup(&work, &branch, false, &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(
        err.message.to_lowercase().contains("not been merged"),
        "{}",
        err.message
    );
    assert!(local_branch_exists(&work, &branch));
    assert_eq!(git_cli(&work, &["branch", "--show-current"]), branch);
}

#[tokio::test]
async fn cleanup_fast_forwards_main_and_deletes_only_the_local_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let remote = tmp.path().join("remote.git");
    let merged = shared_change(&work, "New post: Merged", "merged.md").await;
    git_cli(&work, &["checkout", "main"]);
    let other = shared_change(&work, "New post: Other", "other.md").await;
    git_cli(&work, &["checkout", &merged]);

    merge_on_remote(tmp.path(), &merged, false);

    changes::cleanup(&work, &merged, false, &[]).await.unwrap();

    assert_eq!(git_cli(&work, &["branch", "--show-current"]), "main");
    assert_eq!(
        git_cli(&work, &["rev-parse", "main"]),
        git_cli(&remote, &["rev-parse", "main"]),
        "local main fast-forwarded to the remote's"
    );
    assert_eq!(
        std::fs::read_to_string(work.join("merged.md")).unwrap(),
        "New post: Merged\n"
    );
    assert!(!local_branch_exists(&work, &merged));
    assert!(local_branch_exists(&work, &other), "other changes stay");
    assert!(
        local_branch_exists(&remote, &merged),
        "the remote branch is left alone"
    );
    let open: Vec<String> = changes::list(&work)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.branch)
        .collect();
    assert_eq!(open, vec![other]);
}

#[tokio::test]
async fn cleanup_trusts_a_merged_pr_for_squash_merges() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Squashed", "squashed.md").await;
    merge_on_remote(tmp.path(), &branch, true);

    // The squash commit does not contain the branch tip: only the PR knows it merged.
    let err = changes::cleanup(&work, &branch, false, &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);

    changes::cleanup(&work, &branch, true, &[]).await.unwrap();
    assert_eq!(git_cli(&work, &["branch", "--show-current"]), "main");
    assert!(work.join("squashed.md").exists());
    assert!(!local_branch_exists(&work, &branch));
}

#[tokio::test]
async fn cleanup_keeps_the_branch_when_main_cannot_fast_forward() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Merged", "merged.md").await;
    // Local main has diverged from the remote's.
    git_cli(&work, &["checkout", "main"]);
    commit_file(&work, "local.md", "local only\n");
    git_cli(&work, &["checkout", &branch]);
    merge_on_remote(tmp.path(), &branch, false);

    assert!(changes::cleanup(&work, &branch, true, &[]).await.is_err());
    assert!(
        local_branch_exists(&work, &branch),
        "nothing is deleted before main is up to date"
    );
}

#[tokio::test]
async fn cleanup_refuses_dirty_tree_and_non_change_branches() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Merged", "merged.md").await;
    merge_on_remote(tmp.path(), &branch, false);

    std::fs::write(work.join("merged.md"), "unsaved\n").unwrap();
    let err = changes::cleanup(&work, &branch, true, &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(local_branch_exists(&work, &branch));

    let err = changes::cleanup(&work, "main", true, &[]).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Badreq);
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
async fn http_cleanup_route() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let st = AppState::with_scaffold_install(config(&projects), false);
    let info = st.projects.create("Site").await.unwrap();
    let dir = projects.join(&info.id);
    let app = http::router(st);

    // Local-only project: a change counts as merged once `main` contains it.
    let change = changes::create(&dir, "New post: Launch").await.unwrap();
    commit_file(&dir, "launch.md", "launch\n");
    let uri = "/api/changes/content%2Flaunch/cleanup";

    let (s, body) = post(&app, uri, json!({ "project": info.id })).await;
    assert_eq!(s, StatusCode::CONFLICT, "{body}");

    git_cli(&dir, &["checkout", "main"]);
    git_cli(&dir, &["merge", "--no-ff", &change.branch, "-m", "merge"]);
    git_cli(&dir, &["checkout", &change.branch]);

    let (s, body) = post(&app, uri, json!({ "project": info.id })).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "branch": "main" }));
    assert_eq!(git_cli(&dir, &["branch", "--show-current"]), "main");
    assert!(!local_branch_exists(&dir, &change.branch));

    let (s, _) = post(&app, uri, json!({ "project": "nope" })).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}
