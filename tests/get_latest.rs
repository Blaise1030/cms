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
    git_cli(
        tmp,
        &["clone", "--bare", src.to_str().unwrap(), "remote.git"],
    );
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

fn branches(list: &[changes::Change]) -> Vec<String> {
    list.iter().map(|c| c.branch.clone()).collect()
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

    let update = changes::get_latest(&work, &[]).await.unwrap();

    assert!(update.main_updated);
    assert!(update.updated.is_empty());
    assert!(update.conflicts.is_empty());
    assert_eq!(rev(&work, "main"), remote_main);
    // On main, the working tree follows.
    assert_eq!(git_cli(&work, &["branch", "--show-current"]), "main");
    assert!(work.join("news.md").exists());
}

#[tokio::test]
async fn leaving_checkout_skips_the_active_change_and_tolerates_dirty_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let first = change_with_edit(&work, "First", "first.md", "one\n").await;
    let active = change_with_edit(&work, "Second", "second.md", "two\n").await;
    let active_before = rev(&work, &active);
    std::fs::write(work.join("second.md"), "agent mid-edit\n").unwrap();
    let remote_main = push_to_remote_main(tmp.path(), "news.md", "fresh\n");

    let update = changes::get_latest_leaving_checkout(&work, &[])
        .await
        .unwrap();

    assert!(update.main_updated);
    assert_eq!(branches(&update.updated), vec![first.clone()]);
    assert_eq!(update.skipped.map(|c| c.branch), Some(active.clone()));
    assert_eq!(rev(&work, "main"), remote_main);
    assert!(is_ancestor(&work, &remote_main, &first));
    assert_eq!(rev(&work, &active), active_before);
    assert!(!work.join("news.md").exists());
    assert_eq!(
        std::fs::read_to_string(work.join("second.md")).unwrap(),
        "agent mid-edit\n"
    );
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

    let update = changes::get_latest(&work, &[]).await.unwrap();

    assert!(update.main_updated);
    let mut updated = branches(&update.updated);
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
            git_cli(
                &work,
                &["log", "-1", "--format=%an <%ae>|%cn <%ce>", branch]
            ),
            "Ada Writer <ada@example.com>|Ada Writer <ada@example.com>"
        );
        assert_eq!(
            git_cli(&work, &["show", &format!("{branch}:news.md")]),
            "fresh"
        );
    }
    assert_eq!(
        git_cli(&work, &["show", &format!("{first}:first.md")]),
        "one"
    );

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

    let update = changes::get_latest(&work, &[]).await.unwrap();

    assert!(!update.main_updated);
    assert!(update.updated.is_empty());
    assert!(update.conflicts.is_empty());
    assert_eq!(rev(&work, "main"), main_before);
    assert_eq!(rev(&work, &branch), tip_before);

    // Second run after an update is also a no-op.
    push_to_remote_main(tmp.path(), "news.md", "fresh\n");
    changes::get_latest(&work, &[]).await.unwrap();
    let tip = rev(&work, &branch);
    let again = changes::get_latest(&work, &[]).await.unwrap();
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

    let err = changes::get_latest(&work, &[]).await.unwrap_err();

    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(
        err.message.to_lowercase().contains("checkpoint"),
        "expected checkpoint-first message, got: {}",
        err.message
    );
    assert_eq!(rev(&work, "main"), main_before);
    assert_eq!(rev(&work, &branch), tip_before);
    assert_eq!(
        std::fs::read_to_string(work.join("first.md")).unwrap(),
        "unsaved\n"
    );
}

#[tokio::test]
async fn conflicting_changes_are_reported_and_left_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let clash = change_with_edit(&work, "Clash", "README.md", "mine\n").await;
    let clean = change_with_edit(&work, "Clean", "clean.md", "ok\n").await;
    let clash_before = rev(&work, &clash);
    let remote_main = push_to_remote_main(tmp.path(), "README.md", "theirs\n");

    let update = changes::get_latest(&work, &[]).await.unwrap();

    assert!(update.main_updated);
    assert_eq!(branches(&update.updated), vec![clean.clone()]);
    assert_eq!(branches(&update.conflicts), vec![clash.clone()]);
    assert_eq!(update.conflicts[0].title, "Clash");
    assert_eq!(update.updated[0].title, "Clean");
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

    let update = changes::get_latest(&work, &[]).await.unwrap();

    assert!(!update.main_updated);
    assert_eq!(branches(&update.updated), vec![branch.clone()]);
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
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
    assert_eq!(
        body["updated"],
        json!([{ "branch": branch, "title": "First" }])
    );
    assert_eq!(body["conflicts"], json!([]));

    // Leftover edits are checkpointed onto the change first.
    std::fs::write(dir.join("first.md"), "unsaved\n").unwrap();
    let (s, body) = post(&app, "/api/changes/update", json!({ "project": info.id })).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(
        git_cli(&dir, &["log", "-1", "--format=%s"]),
        "checkpoint: Saved edits"
    );

    let (s, _) = post(&app, "/api/changes/update", json!({})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

// ---- Review fixes ----

#[tokio::test]
async fn fetch_uses_the_auth_env() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    // `origin` only resolves through config supplied in the env (stands in for GitHub auth).
    let fake = "file:///nonexistent/acme/site.git";
    git_cli(&work, &["remote", "set-url", "origin", fake]);
    let remote_main = push_to_remote_main(tmp.path(), "news.md", "fresh\n");
    let env = vec![
        ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
        (
            "GIT_CONFIG_KEY_0".to_string(),
            format!("url.{}.insteadOf", tmp.path().join("remote.git").display()),
        ),
        ("GIT_CONFIG_VALUE_0".to_string(), fake.to_string()),
    ];

    assert!(changes::get_latest(&work, &[]).await.is_err());
    let update = changes::get_latest(&work, &env).await.unwrap();

    assert!(update.main_updated);
    assert_eq!(rev(&work, "main"), remote_main);
}

#[tokio::test]
async fn failure_while_applying_leaves_every_ref_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let first = change_with_edit(&work, "First", "first.md", "one\n").await;
    let second = change_with_edit(&work, "Second", "second.md", "two\n").await;
    let main_before = rev(&work, "main");
    let first_before = rev(&work, &first);
    let second_before = rev(&work, &second);
    push_to_remote_main(tmp.path(), "news.md", "fresh\n");
    // Something else holds `content/first`'s ref lock: that branch's update must fail.
    let lock = work.join(".git/refs/heads/content/first.lock");
    std::fs::write(&lock, "").unwrap();

    let err = changes::get_latest(&work, &[]).await;

    assert!(err.is_err(), "expected failure, got {err:?}");
    assert_eq!(rev(&work, "main"), main_before);
    assert_eq!(rev(&work, &first), first_before);
    assert_eq!(rev(&work, &second), second_before);
    assert!(!work.join("news.md").exists());

    // Once the lock is gone, a retry does everything.
    std::fs::remove_file(&lock).unwrap();
    let update = changes::get_latest(&work, &[]).await.unwrap();
    assert!(update.main_updated);
    assert_eq!(update.updated.len(), 2);
}

#[tokio::test]
async fn refuses_when_an_untracked_file_is_in_the_way() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let other = change_with_edit(&work, "Other", "other.md", "o\n").await;
    let active = change_with_edit(&work, "Active", "active.md", "a\n").await;
    let main_before = rev(&work, "main");
    let other_before = rev(&work, &other);
    let active_before = rev(&work, &active);
    push_to_remote_main(tmp.path(), "news.md", "fresh\n");
    std::fs::write(work.join("news.md"), "my local draft\n").unwrap();

    let err = changes::get_latest(&work, &[]).await.unwrap_err();

    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(err.message.contains("news.md"), "{}", err.message);
    assert_eq!(rev(&work, "main"), main_before);
    assert_eq!(rev(&work, &other), other_before);
    assert_eq!(rev(&work, &active), active_before);
    assert_eq!(
        std::fs::read_to_string(work.join("news.md")).unwrap(),
        "my local draft\n"
    );
}

#[tokio::test]
async fn diverged_main_is_kept_and_changes_get_github_main() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    commit_file(&work, "local-only.md", "x\n", "local commit on main");
    let main_before = rev(&work, "main");
    let branch = change_with_edit(&work, "First", "first.md", "one\n").await;
    let remote_main = push_to_remote_main(tmp.path(), "news.md", "fresh\n");

    let update = changes::get_latest(&work, &[]).await.unwrap();

    assert!(update.main_diverged);
    assert!(!update.main_updated);
    assert_eq!(rev(&work, "main"), main_before);
    assert_eq!(branches(&update.updated), vec![branch.clone()]);
    assert!(is_ancestor(&work, &remote_main, &branch));
}

#[test]
fn git_version_check() {
    assert!(changes::check_git_version("git version 2.54.0").is_ok());
    assert!(changes::check_git_version("git version 2.38.0").is_ok());
    assert!(changes::check_git_version("git version 2.39.5 (Apple Git-154)\n").is_ok());
    assert!(changes::check_git_version("git version 3.0.1").is_ok());
    let err = changes::check_git_version("git version 2.37.1").unwrap_err();
    assert!(err.message.contains("2.38"), "{}", err.message);
    assert!(err.message.contains("2.37.1"), "{}", err.message);
    assert!(changes::check_git_version("nonsense").is_err());
}
