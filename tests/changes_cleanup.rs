//! Clean up a merged change: fast-forward `main`, then delete only the local branch.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use workbench::changes::{self, PrMerge};
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

fn merged(head: &str) -> PrMerge {
    PrMerge::Merged {
        head_sha: head.to_string(),
    }
}

fn tip(dir: &Path, rev: &str) -> String {
    git_cli(dir, &["rev-parse", rev])
}

fn local_branch_exists(dir: &Path, branch: &str) -> bool {
    !git_cli(dir, &["branch", "--list", branch]).is_empty()
}

#[tokio::test]
async fn cleanup_refuses_unmerged_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Draft", "draft.md").await;

    let err = changes::cleanup(&work, &branch, PrMerge::NotMerged, &[])
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

    changes::cleanup(&work, &merged, PrMerge::NotMerged, &[]).await.unwrap();

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
    let err = changes::cleanup(&work, &branch, PrMerge::NotMerged, &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);

    let head = tip(&work, &branch);
    let done = changes::cleanup(&work, &branch, merged(&head), &[]).await.unwrap();
    assert!(done.main_updated);
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

    let local_main = tip(&work, "main");
    let head = tip(&work, &branch);
    let err = changes::cleanup(&work, &branch, merged(&head), &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict, "{}", err.message);
    assert!(
        local_branch_exists(&work, &branch),
        "nothing is deleted before main is up to date"
    );
    assert_eq!(
        git_cli(&work, &["branch", "--show-current"]),
        branch,
        "the change stays checked out"
    );
    assert_eq!(tip(&work, "main"), local_main);
}

#[tokio::test]
async fn cleanup_refuses_dirty_tree_and_non_change_branches() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Merged", "merged.md").await;
    merge_on_remote(tmp.path(), &branch, false);

    std::fs::write(work.join("merged.md"), "unsaved\n").unwrap();
    let head = tip(&work, &branch);
    let err = changes::cleanup(&work, &branch, merged(&head), &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(local_branch_exists(&work, &branch));

    let err = changes::cleanup(&work, "main", merged(&head), &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Badreq);
}

#[tokio::test]
async fn cleanup_refuses_edits_made_after_the_pr_merged() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Squashed", "squashed.md").await;
    let pr_head = tip(&work, &branch);
    merge_on_remote(tmp.path(), &branch, true);
    // Checkpointed after the merge, never pushed.
    commit_file(&work, "later.md", "later\n");

    let err = changes::cleanup(&work, &branch, merged(&pr_head), &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(
        err.message.contains("after its pull request was merged"),
        "{}",
        err.message
    );
    assert!(local_branch_exists(&work, &branch));
    assert_eq!(git_cli(&work, &["branch", "--show-current"]), branch);
}

#[tokio::test]
async fn cleanup_ignores_a_merged_pr_from_an_earlier_change_with_the_same_name() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Squashed", "squashed.md").await;
    let old_head = tip(&work, &branch);
    merge_on_remote(tmp.path(), &branch, true);
    changes::cleanup(&work, &branch, merged(&old_head), &[])
        .await
        .unwrap();

    // GitHub deleted the merged head branch, so a fresh name check can't see it remotely.
    git_cli(&work, &["branch", "-D", "-r", &format!("origin/{branch}")]);
    let again = changes::create(&work, "New post: Squashed").await.unwrap();
    assert_eq!(again.branch, branch, "name is free again");
    commit_file(&work, "new.md", "new work\n");

    assert!(!changes::pr_matches(&work, &branch, &old_head).await);
    let err = changes::cleanup(&work, &branch, merged(&old_head), &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(local_branch_exists(&work, &branch));
}

#[tokio::test]
async fn create_avoids_names_still_on_origin() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Launch", "launch.md").await;
    let head = tip(&work, &branch);
    merge_on_remote(tmp.path(), &branch, false);
    changes::cleanup(&work, &branch, merged(&head), &[])
        .await
        .unwrap();

    let again = changes::create(&work, "New post: Launch").await.unwrap();
    assert_eq!(again.branch, "content/launch-2");
}

#[tokio::test]
async fn pr_matches_the_tip_or_its_ancestors_only() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Launch", "launch.md").await;
    let pushed = tip(&work, &branch);
    commit_file(&work, "more.md", "more\n");
    assert!(changes::pr_matches(&work, &branch, &pushed).await);
    assert!(changes::pr_matches(&work, &branch, &tip(&work, &branch)).await);
    assert!(!changes::pr_matches(&work, &branch, "").await);
    assert!(!changes::pr_matches(&work, &branch, &"0".repeat(40)).await);
    git_cli(&work, &["checkout", "main"]);
    commit_file(&work, "main.md", "main\n");
    assert!(!changes::pr_matches(&work, &branch, &tip(&work, "main")).await);
}

#[tokio::test]
async fn cleanup_refuses_a_change_without_commits_of_its_own() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let change = changes::create(&work, "New post: Empty").await.unwrap();

    let err = changes::cleanup(&work, &change.branch, PrMerge::NotMerged, &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(local_branch_exists(&work, &change.branch));
}

#[tokio::test]
async fn cleanup_needs_github_only_when_git_cannot_tell() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let squashed = shared_change(&work, "New post: Squashed", "squashed.md").await;
    merge_on_remote(tmp.path(), &squashed, true);
    let unknown = || PrMerge::Unknown(workbench::error::ApiError::github("GitHub is down"));

    // Squash merge: only GitHub can say, so its error is the answer.
    let err = changes::cleanup(&work, &squashed, unknown(), &[])
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::GithubErr);
    assert!(err.message.contains("GitHub is down"), "{}", err.message);
    assert!(local_branch_exists(&work, &squashed));

    // A merge commit: git alone knows.
    git_cli(&work, &["checkout", "main"]);
    let merged_branch = shared_change(&work, "New post: Merged", "merged.md").await;
    merge_on_remote(tmp.path(), &merged_branch, false);
    changes::cleanup(&work, &merged_branch, unknown(), &[])
        .await
        .unwrap();
    assert!(!local_branch_exists(&work, &merged_branch));
}

#[tokio::test]
async fn cleanup_without_reaching_origin() {
    let tmp = tempfile::tempdir().unwrap();
    let work = clone_with_origin(tmp.path());
    let branch = shared_change(&work, "New post: Squashed", "squashed.md").await;
    let head = tip(&work, &branch);
    merge_on_remote(tmp.path(), &branch, true);
    let main_before = tip(&work, "main");
    git_cli(&work, &["remote", "set-url", "origin", "/nonexistent/remote.git"]);

    // Merge unknown and the remote unreachable: refuse, keep everything.
    let err = changes::cleanup(&work, &branch, PrMerge::NotMerged, &[])
        .await
        .unwrap_err();
    assert!(
        err.message.contains("Couldn't get the latest main"),
        "{}",
        err.message
    );
    assert!(local_branch_exists(&work, &branch));

    // Merge known from the PR and the tip is exactly what merged: clean up, main as it was.
    let done = changes::cleanup(&work, &branch, merged(&head), &[])
        .await
        .unwrap();
    assert!(!done.main_updated);
    assert_eq!(git_cli(&work, &["branch", "--show-current"]), "main");
    assert_eq!(tip(&work, "main"), main_before);
    assert!(!local_branch_exists(&work, &branch));
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
    assert_eq!(body, json!({ "branch": "main", "mainUpdated": true }));
    assert_eq!(git_cli(&dir, &["branch", "--show-current"]), "main");
    assert!(!local_branch_exists(&dir, &change.branch));

    let (s, _) = post(&app, uri, json!({ "project": "nope" })).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

/// What the fake GitHub's `GET /repos/acme/site/pulls` answers.
#[derive(Clone)]
enum Pulls {
    /// One PR for the queried head in this state, with this head SHA.
    Pr { merged: bool, head_sha: String },
    Fail,
    Hang,
}

struct FakeGithub {
    base: String,
    pulls: std::sync::Arc<std::sync::Mutex<Pulls>>,
    hits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl FakeGithub {
    fn set(&self, p: Pulls) {
        *self.pulls.lock().unwrap() = p;
    }
    fn hits(&self) -> usize {
        self.hits.load(std::sync::atomic::Ordering::SeqCst)
    }
}

async fn fake_github() -> FakeGithub {
    use axum::extract::Query;
    use axum::routing::get;
    use std::collections::HashMap;

    let pulls = std::sync::Arc::new(std::sync::Mutex::new(Pulls::Fail));
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (p, h) = (pulls.clone(), hits.clone());
    let app = axum::Router::new().route(
        "/repos/acme/site/pulls",
        get(move |Query(q): Query<HashMap<String, String>>| {
            let (p, h) = (p.clone(), h.clone());
            async move {
                h.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let current = p.lock().unwrap().clone();
                match current {
                    Pulls::Fail => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json(json!({ "message": "boom" })),
                    ),
                    Pulls::Hang => {
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                        (StatusCode::OK, axum::Json(json!([])))
                    }
                    Pulls::Pr { merged, head_sha } => {
                        let head = q.get("head").cloned().unwrap_or_default();
                        let r = head.split_once(':').map(|(_, r)| r.to_string()).unwrap_or_default();
                        (
                            StatusCode::OK,
                            axum::Json(json!([{
                                "number": 7,
                                "title": "t",
                                "html_url": "https://github.com/acme/site/pull/7",
                                "draft": false,
                                "state": if merged { "closed" } else { "open" },
                                "merged_at": if merged { json!("2026-10-04T00:00:00Z") } else { Value::Null },
                                "head": { "ref": r, "sha": head_sha },
                            }])),
                        )
                    }
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    FakeGithub {
        base: format!("http://{addr}"),
        pulls,
        hits,
    }
}

struct GithubProject {
    app: axum::Router,
    id: String,
    dir: PathBuf,
    gh: FakeGithub,
    _tmp: tempfile::TempDir,
}

/// A signed-in daemon with `acme/site` cloned from a local bare repo, on a shared change
/// `content/launch` with one commit (pushed).
async fn github_project() -> GithubProject {
    use workbench::config::GithubConfig;
    use workbench::git::Identity;
    use workbench::projects::Source;

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
    let session = projects.join(".workbench");
    std::fs::create_dir_all(&session).unwrap();
    std::fs::write(
        session.join("github.json"),
        json!({ "token": "tok", "user": { "login": "ada", "id": 42, "name": "Ada", "avatarUrl": "" } })
            .to_string(),
    )
    .unwrap();
    let gh = fake_github().await;
    let cfg = Config {
        projects_dir: projects.clone(),
        no_auth: true,
        github: GithubConfig {
            client_id: Some("cid".into()),
            api_url: gh.base.clone(),
            web_url: gh.base.clone(),
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
            &Identity::workbench(),
        )
        .await
        .unwrap();
    let dir = projects.join(&info.id);
    let change = changes::create(&dir, "New post: Launch").await.unwrap();
    commit_file(&dir, "launch.md", "launch\n");
    changes::push(&dir, &change.branch, &[]).await.unwrap();
    GithubProject {
        app: http::router(state),
        id: info.id,
        dir,
        gh,
        _tmp: tmp,
    }
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let r = Request::builder()
        .uri(uri)
        .header(header::HOST, "localhost")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(r).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

const LAUNCH: &str = "/api/changes/content%2Flaunch/cleanup";

#[tokio::test]
async fn http_cleanup_checks_locally_before_asking_github() {
    let p = github_project().await;
    p.gh.set(Pulls::Hang);
    let body = json!({ "project": p.id });

    let (s, v) = post(&p.app, "/api/changes/content%2Fnope/cleanup", body.clone()).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    let (s, v) = post(&p.app, "/api/changes/main/cleanup", body.clone()).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    std::fs::write(p.dir.join("launch.md"), "unsaved\n").unwrap();
    let (s, v) = post(&p.app, LAUNCH, body).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(p.gh.hits(), 0, "no GitHub call for a request that fails locally");
}

#[tokio::test]
async fn http_cleanup_surfaces_github_errors_and_times_out() {
    let p = github_project().await;
    let body = json!({ "project": p.id });

    p.gh.set(Pulls::Fail);
    let (s, v) = post(&p.app, LAUNCH, body.clone()).await;
    assert_eq!(v["error"]["code"], "GITHUB_ERR", "{s} {v}");

    p.gh.set(Pulls::Hang);
    let started = std::time::Instant::now();
    let (s, v) = post(&p.app, LAUNCH, body).await;
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert_eq!(v["error"]["code"], "GITHUB_ERR", "{s} {v}");
    assert!(local_branch_exists(&p.dir, "content/launch"));
}

#[tokio::test]
async fn http_cleanup_deletes_the_branch_its_merged_pr_describes() {
    let p = github_project().await;
    let head = tip(&p.dir, "content/launch");
    p.gh.set(Pulls::Pr {
        merged: true,
        head_sha: head,
    });
    let (s, v) = post(&p.app, LAUNCH, json!({ "project": p.id })).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(!local_branch_exists(&p.dir, "content/launch"));
}

#[tokio::test]
async fn http_status_and_list_ignore_a_pr_for_other_commits() {
    let p = github_project().await;
    let status_uri = format!("/api/changes/content%2Flaunch/status?project={}", p.id);
    let list_uri = format!("/api/changes?project={}", p.id);

    // A merged PR from an earlier change that had this name.
    p.gh.set(Pulls::Pr {
        merged: true,
        head_sha: "1".repeat(40),
    });
    let (s, v) = get_json(&p.app, &status_uri).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["pr"], Value::Null);
    let (_, v) = get_json(&p.app, &list_uri).await;
    assert_eq!(v["changes"][0]["branch"], "content/launch");
    assert!(v["changes"][0].get("prState").is_none(), "{v}");

    // This change's own PR.
    p.gh.set(Pulls::Pr {
        merged: true,
        head_sha: tip(&p.dir, "content/launch"),
    });
    let (_, v) = get_json(&p.app, &status_uri).await;
    assert_eq!(v["pr"]["state"], "merged", "{v}");
    let (_, v) = get_json(&p.app, &list_uri).await;
    assert_eq!(v["changes"][0]["prState"], "merged", "{v}");
}
