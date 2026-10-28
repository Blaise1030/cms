//! A sync chat: its worktree, record, intro, and ending.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::changes::sync_chat;
use orso::chat::chunk::UiChunk;
use orso::chat::history::{self, RecordKind, SessionRecord, SyncPhase, SyncState};
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

/// A fake agent that runs `script` in its working directory before replying.
fn sync_provider(id: &str, script: &Path) {
    let (cmd, args) = orso::chat::acp::fake_agent_sync(script);
    let mut argv = vec![cmd];
    argv.extend(args);
    providers::set_test_acp_argv(id, argv);
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
        Self::with_build("true").await
    }

    /// A sync fixture whose site runs `build_script` for `npm run build`.
    async fn with_build(build_script: &str) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("projects");
        std::fs::create_dir(&projects).unwrap();
        providers::set_test_acp_argv("fake-ok", fake_argv());
        let state = AppState::with_scaffold_install(
            Config {
                port: 0,
                projects_dir: projects.clone(),
                no_auth: true,
                // The git launcher agents get must reach the real `git-shim`, not this
                // test binary (which would parse `git-shim` as a test filter).
                hook_exe: Some(PathBuf::from(env!("CARGO_BIN_EXE_orso"))),
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
            json!({"name": "site", "private": true, "scripts": {"build": build_script}})
                .to_string(),
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

    /// A fake provider whose agent runs `body` in the worktree before replying.
    fn provider(&self, id: &str, body: &str) {
        let script = self._tmp.path().join(format!("{id}.sh"));
        std::fs::write(&script, body).unwrap();
        sync_provider(id, &script);
    }

    /// The status lines the writer sees, in order.
    fn notes(&self, chat: &str) -> Vec<String> {
        self.chunks(chat)
            .into_iter()
            .filter_map(|chunk| match chunk {
                UiChunk::SyncStatus { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    /// Everything the writer typed or was shown as their own message.
    fn user_texts(&self, chat: &str) -> Vec<String> {
        self.chunks(chat)
            .into_iter()
            .filter_map(|chunk| match chunk {
                UiChunk::UserText { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    /// Poll until the sync's worktree directory is gone.
    async fn wait_for_gone(&self, chat: &str) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        while self.site(chat).exists() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the worktree {} is still there",
                self.site(chat).display()
            );
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    /// Poll the transcript until a sync note reads exactly `text`.
    async fn wait_for_note(&self, chat: &str, text: &str) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let has_note = self.chunks(chat).iter().any(
                |chunk| matches!(chunk, UiChunk::SyncStatus { text: note, .. } if note == text),
            );
            if has_note {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "no note {text:?}; transcript: {:?}",
                self.chunks(chat)
            );
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    /// Poll the record until the sync reaches `phase`.
    async fn wait_for_phase(&self, chat: &str, phase: SyncPhase) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let current = history::get(&self.state_dir(), &self.project, chat)
                .and_then(|record| record.sync)
                .map(|st| st.phase);
            if current == Some(phase) {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "phase stayed {current:?}, wanted {phase:?}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
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

/// A sync record written without a real agent, for the guard and sweep tests.
fn sync_record(id: &str, started_at: u64, phase: SyncPhase, cwd: Option<PathBuf>) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        provider: "fake-ok".into(),
        model: None,
        agent_session_id: None,
        title: None,
        created_at: started_at,
        updated_at: started_at,
        kind: RecordKind::Sync,
        cwd,
        sync: Some(SyncState {
            conflict_session_id: "session".into(),
            branch: "content/page".into(),
            tip: "tip".into(),
            incoming: "incoming".into(),
            files: vec!["page.md".into()],
            phase,
            started_at,
            ended_at: phase.is_terminal().then_some(started_at),
            repair_attempted: false,
        }),
    }
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

/// The plan's ending contract: a terminal phase, the end time, no live agent,
/// no worktree, and the writer's copy in the transcript.
#[tokio::test]
async fn end_cancels_forgets_and_removes() {
    let h = SyncSetup::new().await;
    h.provider("fake-slow-end", "sleep 5\n");
    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-slow-end")
        .await
        .unwrap();
    assert!(h.site(&chat).is_dir());
    assert!(h.state.chats.is_live(&h.project, &chat));
    assert!(h.state.chats.is_running(&h.project, &chat));

    sync_chat::end(
        &h.state,
        &h.project,
        &chat,
        SyncPhase::Ended,
        sync_chat::ENDED,
    )
    .await
    .unwrap();

    let st = history::get(&h.state_dir(), &h.project, &chat)
        .unwrap()
        .sync
        .unwrap();
    assert_eq!(st.phase, SyncPhase::Ended);
    assert!(st.ended_at.is_some());
    assert!(!h.state.chats.is_live(&h.project, &chat));
    assert!(!h.site(&chat).exists());
    assert!(!h.state_dir().join("sync").join(&chat).exists());
    h.wait_for_note(&chat, sync_chat::ENDED).await;
    assert_plain(sync_chat::ENDED);
    h.state.chats.shutdown().await;
}

/// Deleting a running sync chat through the normal route ends it first, so
/// neither the record nor the worktree survives the delete.
#[tokio::test]
async fn deleting_a_sync_chat_ends_it() {
    let h = SyncSetup::new().await;
    h.provider("fake-slow-delete", "sleep 5\n");
    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-slow-delete")
        .await
        .unwrap();
    assert!(h.state.chats.is_running(&h.project, &chat));

    let app = http::router(h.state.clone());
    let (status, body) = call(
        &app,
        "DELETE",
        &format!("/api/chat/{}/{chat}", h.project),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    assert!(
        history::get(&h.state_dir(), &h.project, &chat).is_none(),
        "the record is gone"
    );
    assert!(!h.state.chats.is_live(&h.project, &chat));
    assert!(
        !h.state_dir().join("sync").join(&chat).exists(),
        "the worktree is gone"
    );
    h.state.chats.shutdown().await;
}

/// The record id names a directory that `end` deletes. An id that is not a safe
/// path segment must never reach the filesystem, even if a record carries one.
#[tokio::test]
async fn end_refuses_an_unsafe_chat_id() {
    let h = SyncSetup::new().await;
    let victim = h.state_dir().join("victim");
    std::fs::create_dir_all(&victim).unwrap();
    std::fs::write(victim.join("keep"), "keep").unwrap();
    history::insert(
        &h.state_dir(),
        &h.project,
        sync_record("../../victim", history::now_ms(), SyncPhase::Working, None),
    )
    .unwrap();

    let err = sync_chat::end(
        &h.state,
        &h.project,
        "../../victim",
        SyncPhase::Ended,
        sync_chat::ENDED,
    )
    .await
    .unwrap_err();

    assert_eq!(err.code, ErrorCode::Badreq);
    assert_plain(&err.message);
    assert!(
        victim.join("keep").is_file(),
        "nothing outside the sync dir is deleted"
    );
    assert_eq!(
        history::get(&h.state_dir(), &h.project, "../../victim")
            .unwrap()
            .sync
            .unwrap()
            .phase,
        SyncPhase::Working,
        "the refused record is not marked"
    );
}

/// A non-terminal sync past its one-hour TTL expires: no live agent, no
/// worktree, and the expiry copy in the transcript.
#[tokio::test]
async fn sweep_expires_after_an_hour() {
    let h = SyncSetup::new().await;
    h.provider("fake-slow-sweep", "sleep 5\n");
    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-slow-sweep")
        .await
        .unwrap();
    let started = history::now_ms() - 3_601_000;
    history::update(&h.state_dir(), &h.project, &chat, |record| {
        if let Some(st) = record.sync.as_mut() {
            st.started_at = started;
        }
    })
    .unwrap();

    sync_chat::sweep(&h.state).await;

    let st = history::get(&h.state_dir(), &h.project, &chat)
        .unwrap()
        .sync
        .unwrap();
    assert_eq!(st.phase, SyncPhase::Expired);
    assert!(st.ended_at.is_some());
    assert!(!h.state.chats.is_live(&h.project, &chat));
    assert!(!h.state_dir().join("sync").join(&chat).exists());
    assert!(h.notes(&chat).iter().any(|note| note == sync_chat::EXPIRED));
    assert_plain(sync_chat::EXPIRED);
    h.state.chats.shutdown().await;
}

/// A terminal sync can still have a worktree left on disk (a crashed end).
/// The sweep removes the directory and leaves the record alone.
#[tokio::test]
async fn sweep_removes_orphan_worktrees() {
    let h = SyncSetup::new().await;
    let chat = "orphan1";
    let chat_dir = h.state_dir().join("sync").join(chat);
    std::fs::create_dir_all(chat_dir.join("site")).unwrap();
    std::fs::write(chat_dir.join("site").join("page.md"), "left\n").unwrap();
    let record = sync_record(chat, 1_000, SyncPhase::Ended, Some(chat_dir.join("site")));
    history::insert(&h.state_dir(), &h.project, record.clone()).unwrap();

    sync_chat::sweep(&h.state).await;

    assert!(!chat_dir.exists(), "the leftover worktree is removed");
    assert_eq!(
        history::get(&h.state_dir(), &h.project, chat),
        Some(record),
        "the record is untouched"
    );
    assert!(h.notes(chat).is_empty(), "no status line is appended");
}

/// An unfinished turn leaves the merge alone, so the writer gets one plain line
/// and the chat stays open instead of landing markers.
#[tokio::test]
async fn markers_leave_a_note_and_keep_working() {
    let h = SyncSetup::new().await;
    let script = h._tmp.path().join("leave.sh");
    std::fs::write(&script, "exit 0\n").unwrap();
    sync_provider("fake-leave", &script);

    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-leave")
        .await
        .unwrap();
    let before = history::get(&h.state_dir(), &h.project, &chat)
        .unwrap()
        .sync
        .unwrap();

    h.wait_for_note(&chat, "1 page still needs a decision.")
        .await;

    let rec = history::get(&h.state_dir(), &h.project, &chat).unwrap();
    assert_eq!(rec.sync.unwrap().phase, SyncPhase::Working);
    assert_eq!(git(&h.dir, &["rev-parse", "content/page"]), before.tip);
    assert_eq!(git(&h.dir, &["show", "content/page:page.md"]), "mine");
    assert!(h.site(&chat).is_dir(), "the sync keeps its worktree");
    h.state.chats.shutdown().await;
}

/// An agent that saves the merge itself must not land; it gets one plain line
/// and the writer keeps the conversation.
#[tokio::test]
async fn agent_commit_keeps_working() {
    let h = SyncSetup::new().await;
    let script = h._tmp.path().join("commit.sh");
    std::fs::write(
        &script,
        "git checkout --ours -- $(git diff --name-only --diff-filter=U); \
         git add -A; git commit -qm x\n",
    )
    .unwrap();
    sync_provider("fake-commit", &script);

    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-commit")
        .await
        .unwrap();
    let before = history::get(&h.state_dir(), &h.project, &chat)
        .unwrap()
        .sync
        .unwrap();

    h.wait_for_note(&chat, sync_chat::AGENT_MOVED).await;

    let rec = history::get(&h.state_dir(), &h.project, &chat).unwrap();
    assert_eq!(rec.sync.unwrap().phase, SyncPhase::Working);
    assert_eq!(git(&h.dir, &["rev-parse", "content/page"]), before.tip);
    assert_eq!(git(&h.dir, &["show", "content/page:page.md"]), "mine");
    assert!(h.site(&chat).is_dir());
    let notes: Vec<String> = h
        .chunks(&chat)
        .into_iter()
        .filter_map(|chunk| match chunk {
            UiChunk::SyncStatus { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        !notes.iter().any(|note| note == sync_chat::ONE_FILE),
        "{notes:?}"
    );
    assert_plain(sync_chat::AGENT_MOVED);
    h.state.chats.shutdown().await;
}

/// A draft that moves while the assistant works ends the sync: no land, no
/// worktree, and the writer is told to get latest again.
#[tokio::test]
async fn draft_moved_ends() {
    let h = SyncSetup::new().await;
    let script = h._tmp.path().join("leave.sh");
    std::fs::write(&script, "exit 0\n").unwrap();
    sync_provider("fake-leave-draft", &script);

    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-leave-draft")
        .await
        .unwrap();
    h.wait_for_note(&chat, "1 page still needs a decision.")
        .await;

    // The writer saves another change to the draft while the sync is open.
    git(&h.dir, &["commit", "--allow-empty", "-m", "x"]);
    h.state
        .chats
        .send_message(&h.project, &chat, "please continue")
        .await
        .unwrap();

    h.wait_for_phase(&chat, SyncPhase::Ended).await;

    let rec = history::get(&h.state_dir(), &h.project, &chat).unwrap();
    let st = rec.sync.unwrap();
    assert_eq!(st.phase, SyncPhase::Ended);
    assert!(st.ended_at.is_some());
    let chunks = h.chunks(&chat);
    assert!(
        chunks
            .iter()
            .any(|chunk| matches!(chunk, UiChunk::SyncStatus { text, .. }
                if text == sync_chat::DRAFT_MOVED)),
        "{chunks:?}"
    );
    h.wait_for_gone(&chat).await;
    assert_eq!(git(&h.dir, &["show", "content/page:page.md"]), "mine");
    assert_plain(sync_chat::DRAFT_MOVED);
    h.state.chats.shutdown().await;
}

/// A resolved turn builds the site and lands the draft as one combine commit.
#[tokio::test]
async fn resolved_turn_builds_and_lands() {
    let h = SyncSetup::new().await;
    h.provider(
        "fake-resolve",
        "git checkout --theirs -- $(git diff --name-only --diff-filter=U) && git add -A\n",
    );

    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-resolve")
        .await
        .unwrap();
    let before = history::get(&h.state_dir(), &h.project, &chat)
        .unwrap()
        .sync
        .unwrap();
    // The draft and its checkout stay put while the assistant works.
    assert_eq!(git(&h.dir, &["rev-parse", "content/page"]), before.tip);
    assert_eq!(git(&h.dir, &["show", "content/page:page.md"]), "mine");

    h.wait_for_phase(&chat, SyncPhase::Combined).await;
    h.wait_for_gone(&chat).await;

    let rec = history::get(&h.state_dir(), &h.project, &chat).unwrap();
    let st = rec.sync.unwrap();
    assert_eq!(st.phase, SyncPhase::Combined);
    assert!(st.ended_at.is_some());

    // The draft moved exactly once, to a commit with both sides.
    let tip = git(&h.dir, &["rev-parse", "content/page"]);
    let log = git(&h.dir, &["rev-list", "--parents", "-n", "1", &tip]);
    let parents: Vec<&str> = log.split(' ').collect();
    assert_eq!(parents, [tip.as_str(), &st.tip, &st.incoming]);
    assert_eq!(git(&h.dir, &["show", "content/page:page.md"]), "latest");

    assert!(
        h.state.conflicts.get(&h.session).await.is_err(),
        "a combined sync finishes its conflict session"
    );
    let chunks = h.chunks(&chat);
    assert!(
        matches!(chunks.last(), Some(UiChunk::SyncStatus { phase, text })
            if phase == "combined" && text == sync_chat::DONE),
        "{chunks:?}"
    );
    h.state.chats.shutdown().await;
}

/// A failed build gets exactly one hidden repair turn; the second build passes.
#[tokio::test]
async fn build_failure_repairs_once() {
    let h = SyncSetup::with_build("test -f ok").await;
    let runs = h._tmp.path().join("runs");
    h.provider(
        "fake-resolve-once",
        &format!(
            "n=0\n[ -f {runs} ] && n=$(cat {runs})\nn=$((n+1))\necho $n > {runs}\n\
             [ \"$n\" -ge 2 ] && touch ok\n\
             paths=$(git diff --name-only --diff-filter=U)\n\
             [ -n \"$paths\" ] && git checkout --theirs -- $paths\n\
             git add -A\n",
            runs = runs.display()
        ),
    );

    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-resolve-once")
        .await
        .unwrap();
    h.wait_for_phase(&chat, SyncPhase::Combined).await;

    let st = history::get(&h.state_dir(), &h.project, &chat)
        .unwrap()
        .sync
        .unwrap();
    assert!(st.repair_attempted);
    assert_eq!(
        h.notes(&chat)
            .iter()
            .filter(|note| *note == sync_chat::BUILD_RETRY)
            .count(),
        1,
        "{:?}",
        h.notes(&chat)
    );
    // The repair prompt and the build output are not the writer's words.
    assert_eq!(h.user_texts(&chat).len(), 1, "{:?}", h.user_texts(&chat));
    h.state.chats.shutdown().await;
}

/// A second failed build ends the sync with the draft untouched.
#[tokio::test]
async fn second_build_failure_ends() {
    let h = SyncSetup::with_build("false").await;
    h.provider(
        "fake-resolve-bad",
        "paths=$(git diff --name-only --diff-filter=U)\n\
         [ -n \"$paths\" ] && git checkout --theirs -- $paths\n\
         git add -A\n",
    );

    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-resolve-bad")
        .await
        .unwrap();
    h.wait_for_phase(&chat, SyncPhase::Ended).await;
    h.wait_for_gone(&chat).await;

    let st = history::get(&h.state_dir(), &h.project, &chat)
        .unwrap()
        .sync
        .unwrap();
    assert_eq!(st.phase, SyncPhase::Ended);
    assert!(h
        .notes(&chat)
        .iter()
        .any(|note| note == sync_chat::BUILD_FAILED_TWICE));
    assert_eq!(git(&h.dir, &["rev-parse", "content/page"]), st.tip);
    assert_eq!(git(&h.dir, &["show", "content/page:page.md"]), "mine");
    // The writer can still pick the changes themselves.
    assert!(h.state.conflicts.get(&h.session).await.is_ok());
    h.state.chats.shutdown().await;
}

/// The composer is locked while the site is checked, so a message cannot race
/// the build or land the draft twice.
#[tokio::test]
async fn message_during_checking_is_rejected() {
    let h = SyncSetup::with_build("sleep 2").await;
    h.provider(
        "fake-resolve-slow",
        "git checkout --theirs -- $(git diff --name-only --diff-filter=U) && git add -A\n",
    );

    let chat = sync_chat::start(&h.state, &h.project, &h.session, "fake-resolve-slow")
        .await
        .unwrap();
    let before = history::get(&h.state_dir(), &h.project, &chat)
        .unwrap()
        .sync
        .unwrap()
        .tip;

    h.wait_for_phase(&chat, SyncPhase::Checking).await;
    let app = http::router(h.state.clone());
    let (status, body) = call(
        &app,
        "POST",
        &format!("/api/chat/{}/{chat}/message", h.project),
        Some(json!({ "text": "hello" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"]["message"].as_str(),
        Some("Checking the site still works.")
    );
    assert_plain(body["error"]["message"].as_str().unwrap());

    h.wait_for_phase(&chat, SyncPhase::Combined).await;
    let tip = git(&h.dir, &["rev-parse", "content/page"]);
    assert_eq!(
        git(
            &h.dir,
            &[
                "rev-list",
                "--first-parent",
                "--count",
                &format!("{before}..{tip}")
            ]
        ),
        "1",
        "the rejected message must not land twice"
    );
    assert_eq!(h.user_texts(&chat).len(), 1, "{:?}", h.user_texts(&chat));
    h.state.chats.shutdown().await;
}

#[test]
fn decision_line_is_singular_only_for_one() {
    assert_eq!(
        sync_chat::pages_needing_decision(1),
        "1 page still needs a decision."
    );
    assert_eq!(
        sync_chat::pages_needing_decision(2),
        "2 pages still need a decision."
    );
    assert_eq!(
        sync_chat::pages_needing_decision(5),
        "5 pages still need a decision."
    );
    assert_plain(&sync_chat::pages_needing_decision(1));
    assert_plain(&sync_chat::pages_needing_decision(2));
}
