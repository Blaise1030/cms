use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use workbench::changes::auto_resolve::{
    AutoResolveJobs, JobContext, JobState, JobStatus, Timeouts,
};
use workbench::changes::conflict_sessions::ConflictSessions;
use workbench::conflict_preview::ConflictPreviewManager;
use workbench::events::EventBus;
use workbench::git::Identity;
use workbench::providers::Provider;
use workbench::{config::Config, http, state::AppState};

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn raw_git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

/// A repo whose `content/page` draft conflicts with `main` in `page.md`.
fn fixture(build: &str) -> (tempfile::TempDir, PathBuf, String, String) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("site");
    std::fs::create_dir(&dir).unwrap();
    git(&dir, &["init", "-b", "main"]);
    git(&dir, &["config", "user.name", "Ada"]);
    git(&dir, &["config", "user.email", "ada@example.com"]);
    std::fs::write(dir.join("page.md"), "old\n").unwrap();
    std::fs::write(
        dir.join("package.json"),
        format!(r#"{{"name":"site","private":true,"scripts":{{"build":"{build}"}}}}"#),
    )
    .unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    git(&dir, &["checkout", "-b", "content/page"]);
    std::fs::write(dir.join("page.md"), "mine\n").unwrap();
    git(&dir, &["commit", "-am", "mine"]);
    let tip = git(&dir, &["rev-parse", "HEAD"]);
    git(&dir, &["checkout", "main"]);
    std::fs::write(dir.join("page.md"), "latest\n").unwrap();
    std::fs::write(dir.join("other.md"), "clean addition\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "latest"]);
    let incoming = git(&dir, &["rev-parse", "HEAD"]);
    git(&dir, &["checkout", "content/page"]);
    (tmp, dir, tip, incoming)
}

struct Setup {
    tmp: tempfile::TempDir,
    dir: PathBuf,
    tip: String,
    incoming: String,
    session_id: String,
    sessions: Arc<ConflictSessions>,
    previews: Arc<ConflictPreviewManager>,
    events: EventBus,
    jobs: AutoResolveJobs,
}

impl Setup {
    async fn new(build: &str, agent: Duration) -> Self {
        let (tmp, dir, tip, incoming) = fixture(build);
        let sessions = Arc::new(ConflictSessions::new());
        let session = sessions
            .start(&dir, "site", "content/page", &incoming)
            .await
            .unwrap();
        Self {
            tmp,
            dir,
            tip,
            incoming,
            session_id: session.id,
            sessions,
            previews: Arc::new(ConflictPreviewManager::new()),
            events: EventBus::new(),
            jobs: AutoResolveJobs::new(Timeouts {
                agent,
                build: Duration::from_secs(30),
            }),
        }
    }

    fn ctx(&self, command: Vec<String>) -> JobContext {
        JobContext {
            project_id: "site".into(),
            project_dir: self.dir.clone(),
            session_id: self.session_id.clone(),
            provider: Provider {
                id: "fake".into(),
                command,
            },
            author: Identity {
                name: "Grace".into(),
                email: "grace@example.com".into(),
            },
            state_dir: self.tmp.path().join("state"),
            events: self.events.clone(),
            sessions: self.sessions.clone(),
            previews: self.previews.clone(),
        }
    }

    async fn start(&self, mode: &str) {
        self.jobs.start(self.ctx(fake(mode))).await.unwrap();
    }

    async fn wait_for(&self, done: impl Fn(&JobStatus) -> bool) -> JobStatus {
        for _ in 0..600 {
            if let Some(status) = self.jobs.status(&self.session_id).await {
                if done(&status) {
                    return status;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!(
            "job never reached the expected state: {:?}",
            self.jobs.status(&self.session_id).await
        );
    }

    async fn finished(&self) -> JobStatus {
        self.wait_for(|s| matches!(s.state, JobState::Done | JobState::Failed))
            .await
    }

    fn branch(&self) -> String {
        git(&self.dir, &["rev-parse", "content/page"])
    }

    fn job_dir(&self) -> PathBuf {
        std::env::temp_dir().join(format!("wb-sync-{}", self.session_id))
    }
}

fn fake(mode: &str) -> Vec<String> {
    vec![
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/fake_resolver.sh"
        )
        .into(),
        mode.into(),
    ]
}

async fn failed_with(setup: &Setup, message: &str) {
    let status = setup.finished().await;
    assert_eq!(status.state, JobState::Failed, "{status:?}");
    assert_eq!(status.error.as_deref(), Some(message));
    assert_eq!(status.stage, "");
    assert!(!setup.job_dir().exists());
}

#[tokio::test]
async fn resolved_advances_branch() {
    let setup = Setup::new("true", Duration::from_secs(5)).await;
    let mut events = setup.events.subscribe();
    setup.start("resolved").await;
    let status = setup.finished().await;
    assert_eq!(status.state, JobState::Done, "{status:?}");
    assert_eq!(status.stage, "Done");
    assert_eq!(status.summary.as_deref(), Some("Kept your version"));
    let merged = setup.branch();
    assert_eq!(
        git(&setup.dir, &["rev-list", "--parents", "-n", "1", &merged]),
        format!("{merged} {} {}", setup.tip, setup.incoming)
    );
    assert_eq!(
        git(&setup.dir, &["show", &format!("{merged}:page.md")]),
        "mine"
    );
    assert_eq!(
        git(&setup.dir, &["show", &format!("{merged}:other.md")]),
        "clean addition"
    );
    assert_eq!(
        git(&setup.dir, &["show", "-s", "--format=%an %s", &merged]),
        "Grace Sync with live site"
    );
    assert!(setup.sessions.get(&setup.session_id).await.is_err());
    assert!(!setup.jobs.is_running(&setup.session_id).await);
    assert!(!setup.job_dir().exists());

    let mut states = Vec::new();
    while let Ok(event) = events.try_recv() {
        let event: serde_json::Value = serde_json::from_str(&event).unwrap();
        assert_eq!(event["type"], "changes.auto_resolve");
        assert_eq!(event["projectId"], "site");
        assert_eq!(event["sessionId"], setup.session_id.as_str());
        states.push(event["state"].as_str().unwrap().to_string());
    }
    assert_eq!(states.first().map(String::as_str), Some("preparing"));
    assert!(states.iter().any(|s| s == "resolving"), "{states:?}");
    assert!(states.iter().any(|s| s == "verifying"), "{states:?}");
    assert_eq!(states.last().map(String::as_str), Some("done"));
}

#[tokio::test]
async fn needs_input_then_answer() {
    let setup = Setup::new("true", Duration::from_secs(5)).await;
    setup.start("needs-input").await;
    let status = setup
        .wait_for(|s| s.state != JobState::Preparing && s.state != JobState::Resolving)
        .await;
    assert_eq!(status.state, JobState::NeedsInput, "{status:?}");
    assert_eq!(status.stage, "Waiting for your answer");
    assert_eq!(status.questions.len(), 1);
    assert_eq!(status.questions[0].id, "q1");
    assert_eq!(status.questions[0].text, "Which headline should stay?");
    assert!(setup.jobs.is_running(&setup.session_id).await);

    let err = setup
        .jobs
        .answer(&setup.session_id, BTreeMap::new())
        .await
        .unwrap_err();
    assert_eq!(err.code.status(), 400);

    setup
        .jobs
        .answer(
            &setup.session_id,
            BTreeMap::from([("q1".to_string(), "Theirs".to_string())]),
        )
        .await
        .unwrap();
    let status = setup.finished().await;
    assert_eq!(status.state, JobState::Done, "{status:?}");
    assert_eq!(
        raw_git(&setup.dir, &["show", "content/page:page.md"]),
        "Theirs\n"
    );
}

#[tokio::test]
async fn answer_requires_needs_input() {
    let setup = Setup::new("true", Duration::from_secs(5)).await;
    setup.start("sleep").await;
    setup.wait_for(|s| s.state == JobState::Resolving).await;
    let err = setup
        .jobs
        .answer(
            &setup.session_id,
            BTreeMap::from([("q1".to_string(), "Theirs".to_string())]),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code.status(), 409);
    setup.jobs.cancel(&setup.session_id).await.unwrap();
    setup.finished().await;
}

#[tokio::test]
async fn garbage_fails_agent() {
    let setup = Setup::new("true", Duration::from_secs(5)).await;
    setup.start("garbage").await;
    failed_with(
        &setup,
        "Your AI assistant couldn't finish combining the changes.",
    )
    .await;
    assert_eq!(setup.branch(), setup.tip);
    assert!(setup.sessions.get(&setup.session_id).await.is_ok());
}

#[tokio::test]
async fn agent_commit_fails_agent() {
    let setup = Setup::new("true", Duration::from_secs(5)).await;
    setup.start("commit").await;
    failed_with(
        &setup,
        "Your AI assistant couldn't finish combining the changes.",
    )
    .await;
    assert_eq!(setup.branch(), setup.tip);
}

#[tokio::test]
async fn missing_cli_is_unavailable() {
    let setup = Setup::new("true", Duration::from_secs(5)).await;
    setup
        .jobs
        .start(setup.ctx(vec!["/nonexistent/cli".into()]))
        .await
        .unwrap();
    failed_with(&setup, "Your AI assistant isn't available.").await;
}

#[tokio::test]
async fn build_failure() {
    let setup = Setup::new("false", Duration::from_secs(5)).await;
    setup.start("resolved").await;
    failed_with(&setup, "The site didn't build after combining.").await;
    assert_eq!(setup.branch(), setup.tip);
}

#[tokio::test]
async fn agent_timeout() {
    let setup = Setup::new("true", Duration::from_secs(1)).await;
    setup.start("sleep").await;
    failed_with(&setup, "This took too long.").await;
}

#[tokio::test]
async fn cancel_stops() {
    let setup = Setup::new("true", Duration::from_secs(5)).await;
    setup.start("sleep").await;
    let status = setup.wait_for(|s| s.state == JobState::Resolving).await;
    assert_eq!(status.stage, "Combining Page…");
    setup.jobs.cancel(&setup.session_id).await.unwrap();
    failed_with(&setup, "You stopped combining.").await;
    assert_eq!(setup.branch(), setup.tip);
    assert!(setup.sessions.get(&setup.session_id).await.is_ok());
}

#[tokio::test]
async fn draft_moved_keeps_new_commit() {
    let setup = Setup::new("true", Duration::from_secs(5)).await;
    setup.start("needs-input").await;
    setup.wait_for(|s| s.state == JobState::NeedsInput).await;
    std::fs::write(setup.dir.join("new.md"), "new\n").unwrap();
    git(&setup.dir, &["add", "new.md"]);
    git(&setup.dir, &["commit", "-m", "new"]);
    let moved = setup.branch();
    setup
        .jobs
        .answer(
            &setup.session_id,
            BTreeMap::from([("q1".to_string(), "Theirs".to_string())]),
        )
        .await
        .unwrap();
    failed_with(
        &setup,
        "Your draft changed while combining. Please sync again.",
    )
    .await;
    assert_eq!(setup.branch(), moved);
}

#[tokio::test]
async fn merge_commit_excludes_node_modules_link() {
    let setup = Setup::new("true", Duration::from_secs(5)).await;
    std::fs::create_dir(setup.dir.join("node_modules")).unwrap();
    std::fs::write(setup.dir.join("node_modules/x.js"), "x\n").unwrap();
    setup.start("resolved").await;
    let status = setup.finished().await;
    assert_eq!(status.state, JobState::Done, "{status:?}");
    let tree = git(
        &setup.dir,
        &["ls-tree", "-r", "--name-only", &setup.branch()],
    );
    assert!(!tree.contains("node_modules"), "{tree}");
}

#[tokio::test]
async fn second_job_same_project_409() {
    let setup = Setup::new("true", Duration::from_secs(5)).await;
    setup.start("sleep").await;
    let err = setup
        .jobs
        .start(setup.ctx(fake("sleep")))
        .await
        .unwrap_err();
    assert_eq!(err.code.status(), 409);
    setup.jobs.cancel(&setup.session_id).await.unwrap();
    setup.finished().await;
    setup.jobs.start(setup.ctx(fake("resolved"))).await.unwrap();
    let status = setup.finished().await;
    assert_eq!(status.state, JobState::Done, "{status:?}");
}

// --- HTTP ---

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "localhost");
    let body = if let Some(v) = body {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
        Body::from(v.to_string())
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

struct HttpSetup {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    project: String,
    dir: PathBuf,
    session_id: String,
}

impl HttpSetup {
    /// A project whose `content/page` draft conflicts with `main`, after Get latest opened a
    /// conflict session; `fake-resolver` runs the fixture in `mode`.
    async fn new(mode: &str) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("projects");
        std::fs::create_dir(&projects).unwrap();
        let agents = tmp.path().join("agents.json");
        std::fs::write(
            &agents,
            json!({ "fake-resolver": ["sh", &fake(mode)[0], mode] }).to_string(),
        )
        .unwrap();
        let state = AppState::with_scaffold_install(
            Config {
                port: 0,
                projects_dir: projects.clone(),
                no_auth: true,
                agents_config: Some(agents),
                ..Default::default()
            },
            false,
        );
        let info = state.projects.create("Site").await.unwrap();
        let dir = projects.join(&info.id);
        git(&dir, &["config", "user.name", "Ada"]);
        git(&dir, &["config", "user.email", "ada@example.com"]);
        std::fs::write(dir.join("page.md"), "old\n").unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"name":"site","private":true,"scripts":{"build":"true"}}"#,
        )
        .unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-m", "base"]);
        git(&dir, &["checkout", "-b", "content/page"]);
        std::fs::write(dir.join("page.md"), "mine\n").unwrap();
        git(&dir, &["commit", "-am", "mine"]);
        git(&dir, &["checkout", "main"]);
        std::fs::write(dir.join("page.md"), "latest\n").unwrap();
        git(&dir, &["commit", "-am", "latest"]);
        git(&dir, &["checkout", "content/page"]);
        let app = http::router(state);
        let (status, update) = call(
            &app,
            "POST",
            "/api/changes/update",
            Some(json!({ "project": info.id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{update}");
        let session_id = update["conflictSessions"]["content/page"]
            .as_str()
            .unwrap_or_else(|| panic!("no conflict session: {update}"))
            .to_string();
        Self {
            _tmp: tmp,
            app,
            project: info.id,
            dir,
            session_id,
        }
    }

    fn url(&self, suffix: &str) -> String {
        format!("/api/changes/conflicts/{}{suffix}", self.session_id)
    }

    async fn post(&self, suffix: &str, body: Value) -> (StatusCode, Value) {
        call(&self.app, "POST", &self.url(suffix), Some(body)).await
    }

    async fn status(&self) -> Value {
        let uri = self.url(&format!("/auto?project={}", self.project));
        let (status, body) = call(&self.app, "GET", &uri, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn poll(&self, done: impl Fn(&Value) -> bool) -> Value {
        for _ in 0..600 {
            let body = self.status().await;
            if done(&body) {
                return body;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!(
            "job never reached the expected state: {}",
            self.status().await
        );
    }

    async fn wait_for(&self, state: &str) -> Value {
        let body = self
            .poll(|b| b["state"] == state || b["state"] == "done" || b["state"] == "failed")
            .await;
        assert_eq!(body["state"], state, "{body}");
        body
    }

    async fn wait_for_end(&self) -> Value {
        self.poll(|b| b["state"] == "done" || b["state"] == "failed")
            .await
    }

    async fn start(&self) {
        let (status, body) = self
            .post(
                "/auto",
                json!({ "project": self.project, "provider": "fake-resolver" }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body, json!({ "started": true }));
    }
}

#[tokio::test]
async fn http_auto_resolved_flow() {
    let setup = HttpSetup::new("resolved").await;
    setup.start().await;
    let status = setup.wait_for("done").await;
    assert_eq!(status["sessionId"], setup.session_id.as_str());
    assert_eq!(status["summary"], "Kept your version");
    assert_eq!(git(&setup.dir, &["show", "content/page:page.md"]), "mine");
}

#[tokio::test]
async fn http_auto_answers() {
    let setup = HttpSetup::new("needs-input").await;
    setup.start().await;
    let status = setup.wait_for("needs_input").await;
    assert_eq!(status["questions"][0]["id"], "q1");
    let (status, body) = setup
        .post(
            "/auto/answers",
            json!({ "project": setup.project, "answers": { "q1": "Theirs" } }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "ok": true }));
    setup.wait_for("done").await;
    assert_eq!(git(&setup.dir, &["show", "content/page:page.md"]), "Theirs");
}

#[tokio::test]
async fn http_auto_unknown_provider_400() {
    let setup = HttpSetup::new("resolved").await;
    let (status, body) = setup
        .post(
            "/auto",
            json!({ "project": setup.project, "provider": "nope" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn http_get_auto_idle() {
    let setup = HttpSetup::new("resolved").await;
    assert_eq!(
        setup.status().await,
        json!({
            "sessionId": setup.session_id,
            "state": "idle",
            "stage": "",
            "questions": [],
            "summary": null,
            "error": null,
        })
    );
}

#[tokio::test]
async fn http_finish_blocked_while_running() {
    let setup = HttpSetup::new("sleep").await;
    setup.start().await;
    setup.wait_for("resolving").await;
    let project = json!({ "project": setup.project });
    let (status, body) = setup.post("/finish", project.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["message"], "Combining is in progress.");
    let (status, body) = setup
        .post(
            "/choice",
            json!({ "project": setup.project, "path": "page.md", "choice": "yours" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, body) = setup.post("/abandon", project.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, body) = setup
        .post(
            "/auto",
            json!({ "project": setup.project, "provider": "fake-resolver" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, body) = setup.post("/auto/cancel", project).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "ok": true }));
    let status = setup.wait_for_end().await;
    assert_eq!(status["error"], "You stopped combining.");
}
