//! Pushing a shared change: combine newer commits from its remote copy first, and hand a
//! collision to the writer's assistant instead of failing the push.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::changes;
use orso::config::{Config, GithubConfig};
use orso::http;
use orso::projects::Source;
use orso::state::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;

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

struct SharedProject {
    app: axum::Router,
    id: String,
    dir: PathBuf,
    branch: String,
    git_root: PathBuf,
    _tmp: tempfile::TempDir,
}

/// A signed-in daemon with `acme/site` cloned from a local bare repo, on a shared change
/// whose remote copy already has the writer's first commit.
async fn shared_project() -> SharedProject {
    use orso::git::Identity;

    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    let git_root = tmp.path().join("git");
    std::fs::create_dir_all(git_root.join("acme")).unwrap();
    let src = git_root.join("src");
    seed_repo(&src);
    git_cli(
        &git_root,
        &["clone", "--bare", src.to_str().unwrap(), "acme/site.git"],
    );
    let session = projects.join(".orso");
    std::fs::create_dir_all(&session).unwrap();
    std::fs::write(
        session.join("github.json"),
        json!({ "token": "tok", "user": { "login": "ada", "id": 42, "name": "Ada", "avatarUrl": "" } })
            .to_string(),
    )
    .unwrap();
    let cfg = Config {
        projects_dir: projects.clone(),
        no_auth: true,
        github: GithubConfig {
            client_id: Some("cid".into()),
            api_url: "http://127.0.0.1:9".into(),
            web_url: "http://127.0.0.1:9".into(),
            git_url: format!("file://{}", git_root.display()),
            gh_path: tmp.path().join("no-gh"),
            git_global_config: None,
        },
        ..Default::default()
    };
    let state = AppState::with_scaffold_install(cfg, false);
    let info = state
        .projects
        .create_from(
            "site",
            Source::Clone {
                repo: "acme/site".into(),
                url: format!("file://{}", git_root.join("acme/site.git").display()),
                env: vec![],
            },
            &Identity::orso(),
        )
        .await
        .unwrap();
    let dir = projects.join(&info.id);
    let change = changes::create(&dir, "New post: Launch").await.unwrap();
    changes::push(&dir, &change.branch, &[]).await.unwrap();
    SharedProject {
        app: http::router(state),
        id: info.id,
        dir,
        branch: change.branch,
        git_root,
        _tmp: tmp,
    }
}

/// Another writer commits `content` to the same file on the shared change.
fn teammate_edits(p: &SharedProject, file: &str, content: &str) {
    let mate = p._tmp.path().join("mate");
    if !mate.exists() {
        let bare = p.git_root.join("acme/site.git");
        git_cli(p._tmp.path(), &["clone", bare.to_str().unwrap(), "mate"]);
    }
    git_cli(&mate, &["fetch", "origin"]);
    git_cli(
        &mate,
        &["checkout", "-B", &p.branch, &format!("origin/{}", p.branch)],
    );
    std::fs::write(mate.join(file), content).unwrap();
    git_cli(&mate, &["add", "-A"]);
    git_cli(&mate, &["commit", "-m", "teammate edit"]);
    git_cli(&mate, &["push", "origin", &p.branch]);
}

fn branch_path(branch: &str) -> String {
    branch.replace('/', "%2F")
}

#[tokio::test]
async fn share_opens_a_combine_session_for_a_colliding_remote_change() {
    let p = shared_project().await;
    teammate_edits(&p, "README.md", "theirs\n");
    std::fs::write(p.dir.join("README.md"), "mine\n").unwrap();
    git_cli(&p.dir, &["add", "-A"]);
    git_cli(&p.dir, &["commit", "-m", "mine"]);
    let tip = git_cli(&p.dir, &["rev-parse", &format!("refs/heads/{}", p.branch)]);

    let (status, body) = call(
        &p.app,
        req(
            "POST",
            &format!("/api/changes/{}/share", branch_path(&p.branch)),
            Some(json!({ "project": p.id })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["branch"], p.branch);
    assert_eq!(body["conflict"]["branch"], p.branch);
    assert!(body["conflict"]["sessionId"].as_str().is_some(), "{body}");
    assert_eq!(
        git_cli(&p.dir, &["rev-parse", &format!("refs/heads/{}", p.branch)]),
        tip,
        "a collision must leave the change alone"
    );

    let (status, body) = call(
        &p.app,
        req(
            "GET",
            &format!("/api/changes/conflicts?project={}", p.id),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sessions"][0]["branch"], p.branch);
}

#[tokio::test]
async fn publish_opens_a_combine_session_instead_of_failing_the_push() {
    let p = shared_project().await;
    teammate_edits(&p, "README.md", "theirs\n");
    std::fs::write(p.dir.join("README.md"), "mine\n").unwrap();
    git_cli(&p.dir, &["add", "-A"]);
    git_cli(&p.dir, &["commit", "-m", "mine"]);

    let (status, body) = call(
        &p.app,
        req(
            "POST",
            &format!("/api/changes/{}/publish", branch_path(&p.branch)),
            Some(json!({ "project": p.id })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["pr"], Value::Null);
    assert_eq!(body["conflict"]["branch"], p.branch);
    assert!(body["conflict"]["sessionId"].as_str().is_some(), "{body}");
}

#[tokio::test]
async fn share_pushes_a_clean_remote_merge_without_a_session() {
    let p = shared_project().await;
    teammate_edits(&p, "theirs.md", "theirs\n");
    std::fs::write(p.dir.join("ours.md"), "ours\n").unwrap();
    git_cli(&p.dir, &["add", "-A"]);
    git_cli(&p.dir, &["commit", "-m", "ours"]);

    // A clean combination continues into share, which then needs GitHub. The push itself
    // must succeed even though the PR lookup fails.
    let (_, _) = call(
        &p.app,
        req(
            "POST",
            &format!("/api/changes/{}/share", branch_path(&p.branch)),
            Some(json!({ "project": p.id })),
        ),
    )
    .await;

    let remote = rev(&p.git_root.join("acme/site.git"), &p.branch);
    let local = rev(&p.dir, &p.branch);
    assert_eq!(remote, local, "the combined change reached GitHub");
}

fn rev(dir: &Path, branch: &str) -> String {
    git_cli(dir, &["rev-parse", &format!("refs/heads/{branch}")])
}
