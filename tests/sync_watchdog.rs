//! A sync chat's per-turn watchdog. A separate test binary because the turn
//! limit is read from a process-global environment variable.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::changes::sync_chat;
use orso::chat::chunk::UiChunk;
use orso::chat::history::{self, SyncPhase};
use orso::{config::Config, http, providers, state::AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

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

struct Setup {
    _tmp: tempfile::TempDir,
    state: AppState,
    project: String,
    state_dir: PathBuf,
    session: String,
}

impl Setup {
    async fn new(build: &str) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("projects");
        std::fs::create_dir(&projects).unwrap();
        let state = AppState::with_scaffold_install(
            Config {
                port: 0,
                projects_dir: projects.clone(),
                no_auth: true,
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
            json!({"name": "site", "private": true, "scripts": {"build": build}}).to_string(),
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
        let app = http::router(state.clone());
        let (status, update) = call(
            &app,
            "POST",
            "/api/changes/update",
            Some(json!({ "project": info.id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{update}");
        let session = update["conflictSessions"]["content/page"]
            .as_str()
            .unwrap_or_else(|| panic!("no conflict session: {update}"))
            .to_string();
        Self {
            _tmp: tmp,
            state,
            project: info.id,
            state_dir: projects.join(".orso"),
            session,
        }
    }

    fn chunks(&self, chat: &str) -> Vec<UiChunk> {
        self.state.chats.history(&self.project, chat)
    }
}

/// The test seam, set once: both tests in this binary share one process.
fn short_limit() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("ORSO_SYNC_TURN_LIMIT_SECS", "1"));
}

/// A turn that outlives the limit is cancelled and the writer gets one plain
/// line; the chat stays open so they can ask it to continue.
#[tokio::test]
async fn a_long_turn_is_cancelled_and_noted() {
    short_limit();
    let h = Setup::new("true").await;

    let script = h._tmp.path().join("slow.sh");
    std::fs::write(&script, "sleep 5\n").unwrap();
    let (cmd, args) = orso::chat::acp::fake_agent_sync(&script);
    let mut argv = vec![cmd];
    argv.extend(args);
    providers::set_test_acp_argv("fake-slow", argv);

    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-slow")
        .await
        .unwrap();

    // The limit is one second; give the watchdog room to fire and record.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let noted = h.chunks(&chat).iter().any(
            |chunk| matches!(chunk, UiChunk::SyncStatus { text, .. } if text == sync_chat::TOO_LONG),
        );
        if noted {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no watchdog note; transcript: {:?}",
            h.chunks(&chat)
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let st = history::get(&h.state_dir, &h.project, &chat)
        .unwrap()
        .sync
        .unwrap();
    assert_eq!(st.phase, SyncPhase::Working);
    h.state.chats.shutdown().await;
}

/// A timer belongs to the turn that armed it. The second turn here is sent
/// straight through the manager, so the opening prompt's timer is the only one
/// in flight; past its deadline the newer turn must still be running and un-noted.
#[tokio::test]
async fn a_stale_watchdog_does_not_cancel_a_newer_turn() {
    short_limit();
    let h = Setup::new("true").await;

    let script = h._tmp.path().join("stale.sh");
    std::fs::write(&script, "sleep 5\n").unwrap();
    let (cmd, args) = orso::chat::acp::fake_agent_sync(&script);
    let mut argv = vec![cmd];
    argv.extend(args);
    providers::set_test_acp_argv("fake-stale", argv);

    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-stale")
        .await
        .unwrap();
    assert!(h.state.chats.is_running(&h.project, &chat));

    // A newer turn, with no watchdog armed for it.
    h.state
        .chats
        .send_message(&h.project, &chat, "keep going")
        .await
        .unwrap();

    // Past the opening turn's deadline, while the newer turn is still running.
    tokio::time::sleep(Duration::from_millis(1800)).await;
    assert!(
        h.state.chats.is_running(&h.project, &chat),
        "the older turn's timer stopped the newer turn"
    );
    let noted = h.chunks(&chat).iter().any(
        |chunk| matches!(chunk, UiChunk::SyncStatus { text, .. } if text == sync_chat::TOO_LONG),
    );
    assert!(
        !noted,
        "the older turn's timer noted the newer turn: {:?}",
        h.chunks(&chat)
    );
    h.state.chats.shutdown().await;
}
