//! Slice 3: GitHub projects on `main` require a change before agent prompt / checkpoint.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use workbench::config::Config;
use workbench::git::Identity;
use workbench::http;
use workbench::projects::Source;
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

fn seed_bare(tmp: &Path) -> PathBuf {
    let src = tmp.join("src");
    std::fs::create_dir_all(&src).unwrap();
    git_cli(&src, &["init", "-b", "main"]);
    git_cli(&src, &["config", "user.name", "Ada"]);
    git_cli(&src, &["config", "user.email", "ada@example.com"]);
    std::fs::write(src.join("package.json"), "{}\n").unwrap();
    git_cli(&src, &["add", "-A"]);
    git_cli(&src, &["commit", "-m", "init"]);
    let bare = tmp.join("remote.git");
    git_cli(
        tmp,
        &["clone", "--bare", src.to_str().unwrap(), "remote.git"],
    );
    bare
}

fn config(projects_dir: &Path, agents: Option<&Path>) -> Config {
    Config {
        port: 0,
        origin: None,
        projects_dir: projects_dir.to_path_buf(),
        no_auth: true,
        token: None,
        agents_config: agents.map(|p| p.to_path_buf()),
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn fake_agent() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for profile in ["release", "debug"] {
        let p = path.join(format!("target/{profile}/examples/fake_acp_agent"));
        if p.exists() {
            return p;
        }
    }
    // Worktree may share another target via symlink.
    path = std::fs::canonicalize(&path).unwrap_or(path);
    for profile in ["release", "debug"] {
        let p = path.join(format!("target/{profile}/examples/fake_acp_agent"));
        if p.exists() {
            return p;
        }
    }
    panic!("fake_acp_agent not built; run cargo build --example fake_acp_agent");
}

#[tokio::test]
async fn require_change_blocks_github_on_main_allows_local_and_branches() {
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("work");
    let bare = seed_bare(tmp.path());
    git_cli(tmp.path(), &["clone", bare.to_str().unwrap(), "work"]);
    git_cli(&work, &["config", "user.name", "Ada"]);
    git_cli(&work, &["config", "user.email", "ada@example.com"]);

    let err = changes::require_change(&work, Some("acme/site"))
        .await
        .unwrap_err();
    assert_eq!(err.code.as_str(), "CHANGE_REQUIRED");

    changes::require_change(&work, None).await.unwrap();

    changes::create(&work, "New post: Launch week")
        .await
        .unwrap();
    changes::require_change(&work, Some("acme/site"))
        .await
        .unwrap();
}

#[tokio::test]
async fn agent_prompt_rejects_main_for_github_allows_local() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    std::fs::create_dir_all(&root).unwrap();
    let agents = root.join("agents.json");
    let exe = fake_agent().to_string_lossy().into_owned();
    std::fs::write(&agents, json!({ "fake": [exe] }).to_string()).unwrap();

    let state = AppState::with_scaffold_install(config(&root, Some(&agents)), false);
    state.agents.set_settle_quiet(Duration::from_millis(150));

    // Local-only template project: prompt on main is allowed.
    let local = state.projects.create("Local").await.unwrap();
    let app = http::router(state.clone());
    let (s, v) = call(
        &app,
        req(
            "POST",
            "/api/agent/session",
            Some(json!({ "project": local.id, "provider": "fake" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let sid = v["sessionId"].as_str().unwrap();
    let (s, v) = call(
        &app,
        req(
            "POST",
            "/api/agent/prompt",
            Some(json!({ "project": local.id, "sessionId": sid, "text": "hello" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");

    // GitHub clone on main: prompt is rejected with CHANGE_REQUIRED.
    let bare = seed_bare(&root.join("git"));
    let gh = state
        .projects
        .create_from(
            "gh-site",
            Source::Clone {
                repo: "acme/site".into(),
                url: format!("file://{}", bare.display()),
                env: vec![],
            },
            &Identity::workbench(),
        )
        .await
        .unwrap();
    assert_eq!(gh.repo.as_deref(), Some("acme/site"));
    let dir = root.join(&gh.id);
    assert_eq!(git_cli(&dir, &["branch", "--show-current"]), "main");

    let (s, v) = call(
        &app,
        req(
            "POST",
            "/api/agent/session",
            Some(json!({ "project": gh.id, "provider": "fake" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let sid = v["sessionId"].as_str().unwrap();
    let (s, v) = call(
        &app,
        req(
            "POST",
            "/api/agent/prompt",
            Some(json!({ "project": gh.id, "sessionId": sid, "text": "edit the notes" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["code"], "CHANGE_REQUIRED");

    // After starting a change, prompt (and thus checkpoint) is allowed.
    changes::create(&dir, "New post: Launch week")
        .await
        .unwrap();
    let (s, v) = call(
        &app,
        req(
            "POST",
            "/api/agent/prompt",
            Some(json!({ "project": gh.id, "sessionId": sid, "text": "edit the notes" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");

    // Wait for ACP settle → checkpoint on the change branch.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if git_cli(&dir, &["log", "--oneline", "-5"]).contains("checkpoint:") {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "timed out waiting for checkpoint; log={}",
                git_cli(&dir, &["log", "--oneline", "-5"])
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        git_cli(&dir, &["branch", "--show-current"]),
        "content/launch-week"
    );
    assert!(
        git_cli(&dir, &["log", "-1", "--format=%s", "content/launch-week"])
            .starts_with("checkpoint:")
    );
}

#[tokio::test]
async fn checkpoint_guard_blocks_main_for_github_via_require_change() {
    // The ACP checkpoint path calls the same guard; assert the code it would see.
    let tmp = tempfile::tempdir().unwrap();
    let bare = seed_bare(tmp.path());
    let work = tmp.path().join("work");
    git_cli(tmp.path(), &["clone", bare.to_str().unwrap(), "work"]);
    git_cli(&work, &["config", "user.name", "Ada"]);
    git_cli(&work, &["config", "user.email", "ada@example.com"]);
    std::fs::write(work.join("notes.txt"), "dirty\n").unwrap();

    let err = changes::require_change(&work, Some("acme/site"))
        .await
        .unwrap_err();
    assert_eq!(err.code.as_str(), "CHANGE_REQUIRED");

    // Local-only: checkpoint itself may proceed (guard allows).
    changes::require_change(&work, None).await.unwrap();
    let cp = git::checkpoint(&work, "local edit").await.unwrap();
    assert!(cp.is_some());
}
