//! A sync chat: its worktree, record, intro, and ending.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::changes::sync_chat;
use orso::chat::chunk::UiChunk;
use orso::chat::history::{self, RecordKind, SyncPhase};
use orso::error::ErrorCode;
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

/// The fake ACP agent, spelled the way `providers::acp_argv` expects.
fn fake_argv() -> Vec<String> {
    let (cmd, args) = orso::chat::acp::fake_agent();
    let mut argv = vec![cmd];
    argv.extend(args);
    argv
}

const FORBIDDEN: &[&str] = &[
    "branch", "commit", "merge", "conflict", "worktree", "checkout", "clone", "git", "origin",
    "main", "markers", "npm", "<<<<<<<",
];

/// Every message an HTTP response can carry must avoid git vocabulary.
fn assert_plain(text: &str) {
    let lower = text.to_ascii_lowercase();
    for word in FORBIDDEN {
        assert!(!lower.contains(word), "{word:?} in writer-visible {text:?}");
    }
}

/// A project whose `content/page` draft conflicts with `main` in `page.md`,
/// with a conflict session opened by Get latest.
struct SyncSetup {
    _tmp: tempfile::TempDir,
    state: AppState,
    project: String,
    dir: PathBuf,
    projects: PathBuf,
    session: String,
}

impl SyncSetup {
    async fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("projects");
        std::fs::create_dir(&projects).unwrap();
        providers::set_test_acp_argv("fake-ok", fake_argv());
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
            dir,
            projects,
            session,
        }
    }

    fn state_dir(&self) -> PathBuf {
        self.projects.join(".orso")
    }

    fn site(&self, chat: &str) -> PathBuf {
        self.state_dir().join("sync").join(chat).join("site")
    }

    fn chunks(&self, chat: &str) -> Vec<UiChunk> {
        self.state.chats.history(&self.project, chat)
    }

    async fn start(&self) -> String {
        sync_chat::start(&self.state, &self.project, &self.session, "fake-ok")
            .await
            .unwrap()
    }
}

/// The last event of `kind`, if any were emitted since the subscription.
fn event_after(mut rx: tokio::sync::broadcast::Receiver<std::sync::Arc<str>>, kind: &str) -> Value {
    let mut last = None;
    while let Ok(frame) = rx.try_recv() {
        let event: Value = serde_json::from_str(&frame).unwrap();
        if event["type"] == kind {
            last = Some(event);
        }
    }
    last.unwrap_or_else(|| panic!("no {kind} event"))
}

#[tokio::test]
async fn start_creates_worktree_record_and_intro() {
    let h = SyncSetup::new().await;
    let events = h.state.events.subscribe();
    let chat = h.start().await;

    let rec = history::get(&h.state_dir(), &h.project, &chat).unwrap();
    assert_eq!(rec.kind, RecordKind::Sync);
    assert_eq!(rec.provider, "fake-ok");
    assert!(
        rec.title.as_deref().unwrap_or("").starts_with("Combining "),
        "{:?}",
        rec.title
    );
    let st = rec.sync.clone().unwrap();
    let site = h.site(&chat);
    assert_eq!(rec.cwd.as_deref(), Some(site.as_path()));
    assert_eq!(st.phase, SyncPhase::Working);
    assert_eq!(st.conflict_session_id, h.session);
    assert_eq!(st.branch, "content/page");
    assert_eq!(st.files, vec!["page.md".to_string()]);
    assert!(st.ended_at.is_none() && !st.repair_attempted);

    // The worktree is the draft tip mid-merge with the live site.
    assert_eq!(git(&site, &["rev-parse", "HEAD"]), st.tip);
    assert_eq!(git(&site, &["rev-parse", "MERGE_HEAD"]), st.incoming);
    assert!(std::fs::read_to_string(site.join("page.md"))
        .unwrap()
        .contains("<<<<<<< "));

    // Neither the draft nor its checkout moved.
    assert_eq!(git(&h.dir, &["rev-parse", "content/page"]), st.tip);
    assert_eq!(git(&h.dir, &["show", "content/page:page.md"]), "mine");

    let first_user = h
        .chunks(&chat)
        .into_iter()
        .find_map(|chunk| match chunk {
            UiChunk::UserText { text, .. } => Some(text),
            _ => None,
        })
        .unwrap();
    assert!(
        first_user.starts_with("You and a teammate both changed"),
        "{first_user}"
    );
    assert!(!first_user.contains("git"), "{first_user}");

    let event = event_after(events, "sync.changed");
    assert_eq!(event["projectId"].as_str(), Some(h.project.as_str()));
    assert_eq!(event["chatId"].as_str(), Some(chat.as_str()));
    assert_eq!(
        event["conflictSessionId"].as_str(),
        Some(h.session.as_str())
    );
    assert_eq!(event["phase"].as_str(), Some("working"));

    h.state.chats.shutdown().await;
}

#[tokio::test]
async fn a_second_sync_for_the_same_project_is_refused() {
    let h = SyncSetup::new().await;
    let _chat = h.start().await;
    let err = sync_chat::start(&h.state, &h.project, &h.session, "fake-ok")
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(err.message, "Combining is in progress.");
    assert_plain(&err.message);
    let syncs = history::list(&h.state_dir(), &h.project)
        .into_iter()
        .filter(|r| r.kind == RecordKind::Sync)
        .count();
    assert_eq!(syncs, 1);
    assert!(sync_chat::active_for_project(&h.state, &h.project).is_some());
    h.state.chats.shutdown().await;
}

#[tokio::test]
async fn an_unknown_provider_is_a_bad_request() {
    let h = SyncSetup::new().await;
    let err = sync_chat::start(&h.state, &h.project, &h.session, "nope")
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Badreq);
    assert_eq!(err.message, "unknown provider: nope");
    assert_plain(&err.message);
    assert!(history::list(&h.state_dir(), &h.project).is_empty());
    assert!(sync_chat::active_for_project(&h.state, &h.project).is_none());
}

/// An expired/unknown session used to surface "conflict session expired or not
/// found". `start` must hand the writer the plain line instead.
#[tokio::test]
async fn a_missing_session_returns_plain_copy() {
    let h = SyncSetup::new().await;
    let err = sync_chat::start(&h.state, &h.project, "no-such-session", "fake-ok")
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Enoent);
    assert_eq!(err.message, sync_chat::UNEXPECTED);
    assert_plain(&err.message);
    assert!(history::list(&h.state_dir(), &h.project).is_empty());
    assert!(!h.state_dir().join("sync").exists());
}

#[tokio::test]
async fn end_marks_the_chat_and_removes_the_worktree() {
    let h = SyncSetup::new().await;
    let chat = h.start().await;
    let site = h.site(&chat);
    assert!(site.is_dir());
    assert!(h.state.chats.is_live(&h.project, &chat));
    let events = h.state.events.subscribe();

    sync_chat::end(
        &h.state,
        &h.project,
        &chat,
        SyncPhase::Ended,
        sync_chat::UNAVAILABLE,
    )
    .await
    .unwrap();

    let rec = history::get(&h.state_dir(), &h.project, &chat).unwrap();
    let st = rec.sync.unwrap();
    assert_eq!(st.phase, SyncPhase::Ended);
    assert!(st.ended_at.is_some());
    assert!(!h.state.chats.is_live(&h.project, &chat));
    assert!(!site.exists());
    assert!(!h.state_dir().join("sync").join(&chat).exists());
    // The conflict session survives, so the writer can pick changes themselves.
    assert!(h.state.conflicts.get(&h.session).await.is_ok());

    let chunks = h.chunks(&chat);
    assert!(
        chunks
            .iter()
            .any(|c| matches!(c, UiChunk::SyncStatus { phase, text }
            if phase == "ended" && text == sync_chat::UNAVAILABLE)),
        "{chunks:?}"
    );

    let event = event_after(events, "sync.changed");
    assert_eq!(event["projectId"].as_str(), Some(h.project.as_str()));
    assert_eq!(event["chatId"].as_str(), Some(chat.as_str()));
    assert_eq!(event["phase"].as_str(), Some("ended"));

    // Ending twice is a no-op: one status line, phase unchanged.
    sync_chat::end(
        &h.state,
        &h.project,
        &chat,
        SyncPhase::Ended,
        sync_chat::UNAVAILABLE,
    )
    .await
    .unwrap();
    let statuses = h
        .chunks(&chat)
        .into_iter()
        .filter(|c| matches!(c, UiChunk::SyncStatus { .. }))
        .count();
    assert_eq!(statuses, 1);
}
