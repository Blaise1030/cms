//! `ChatManager`: headless turns against `tests/fixtures/fake_headless.sh`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::sync::broadcast;
use workbench::chat::chunk::{AgentErrorCode, UiChunk};
use workbench::chat::manager::{Subscription, TurnRequest};
use workbench::chat::transcript;
use workbench::config::Config;
use workbench::providers::{self, AgentSession, Provider};
use workbench::state::AppState;
use workbench::terminal::history::{self, SessionRecord};

const CHAT: &str = "chat1";
const FIXTURE_SESSION: &str = "34f63cf5-7e32-4205-9b34-72582ec9f80e";
const WAIT: Duration = Duration::from_secs(10);

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Env {
    _tmp: tempfile::TempDir,
    state: AppState,
    project: String,
    dir: PathBuf,
    state_dir: PathBuf,
}

/// A project whose `cursor` provider runs `argv`, with a `sessions.json` record for [`CHAT`].
async fn env(argv: Vec<String>) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let agents = tmp.path().join("agents.json");
    std::fs::write(&agents, json!({ "cursor": argv }).to_string()).unwrap();
    let config = Config {
        port: 0,
        projects_dir: tmp.path().to_path_buf(),
        no_auth: true,
        agents_config: Some(agents),
        ..Default::default()
    };
    let state = AppState::with_scaffold_install(config, false);
    let info = state.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    let state_dir = tmp.path().join(".workbench");
    let now = history::now_ms();
    history::insert(
        &state_dir,
        &info.id,
        SessionRecord {
            id: CHAT.into(),
            provider: "cursor".into(),
            agent_session_id: Some("chat-fake-1".into()),
            title: None,
            created_at: now,
            updated_at: now,
        },
    )
    .unwrap();
    Env {
        _tmp: tmp,
        state,
        project: info.id,
        dir,
        state_dir,
    }
}

/// `sh fake_headless.sh cursor/turn.jsonl <mode>`, with `FAKE_PIDFILE` set when given.
fn fake(mode: &str, pidfile: Option<&Path>) -> Vec<String> {
    let mut argv = Vec::new();
    if let Some(pidfile) = pidfile {
        argv.push("env".to_string());
        argv.push(format!("FAKE_PIDFILE={}", pidfile.display()));
    }
    argv.push("sh".into());
    argv.push(fixtures().join("fake_headless.sh").display().to_string());
    argv.push(
        fixtures()
            .join("chat/cursor/turn.jsonl")
            .display()
            .to_string(),
    );
    argv.push(mode.into());
    argv
}

fn cursor(env: &Env) -> Provider {
    providers::load(env.state.config.agents_config.as_deref())
        .unwrap()
        .into_iter()
        .find(|p| p.id == "cursor")
        .unwrap()
}

fn request(env: &Env, prompt: &str) -> TurnRequest {
    TurnRequest {
        project_id: env.project.clone(),
        chat_id: CHAT.into(),
        provider: cursor(env),
        dir: env.dir.clone(),
        prompt: prompt.into(),
        session: AgentSession::Assign("chat-fake-1".into()),
    }
}

/// Replay plus live chunks until the turn is over.
async fn collect(sub: Subscription) -> Vec<UiChunk> {
    let Subscription { mut replay, mut rx } = sub;
    loop {
        match tokio::time::timeout(WAIT, rx.recv()).await {
            Ok(Ok(Some(chunk))) => replay.push(chunk),
            Ok(Ok(None)) => return replay,
            other => panic!("stream did not end cleanly: {other:?}; got {replay:?}"),
        }
    }
}

/// The project's events up to and including the first of type `until`.
async fn events_until(
    rx: &mut broadcast::Receiver<std::sync::Arc<str>>,
    project: &str,
    until: &str,
) -> Vec<Value> {
    let mut seen = Vec::new();
    loop {
        let text = tokio::time::timeout(WAIT, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("no {until} event; saw {seen:?}"))
            .unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        if v["projectId"] != project {
            continue;
        }
        let done = v["type"] == until;
        seen.push(v);
        if done {
            return seen;
        }
    }
}

fn has_text(chunks: &[UiChunk], needle: &str) -> bool {
    chunks
        .iter()
        .any(|c| matches!(c, UiChunk::TextDelta { delta, .. } if delta.contains(needle)))
}

fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}

#[tokio::test]
async fn turn_streams_and_checkpoints() {
    let env = env(fake("edit", None)).await;
    workbench::changes::create(&env.dir, "New post")
        .await
        .unwrap();
    let mut events = env.state.events.subscribe();

    let sub = env
        .state
        .chats
        .start(env.state.clone(), request(&env, "Add a note\nsecond line"))
        .unwrap();
    assert_eq!(env.state.chats.running(&env.project).as_deref(), Some(CHAT));
    let chunks = collect(sub).await;

    assert!(
        matches!(&chunks[0], UiChunk::Start { message_id } if message_id.starts_with("msg-")),
        "{chunks:?}"
    );
    assert_eq!(chunks[1], UiChunk::StartStep);
    assert!(has_text(&chunks, "I added the line to `about.md`"));
    let n = chunks.len();
    match &chunks[n - 3] {
        UiChunk::DataCheckpoint { data } => {
            assert_eq!(data.files, ["notes.md"]);
            assert!(data.sha.is_some() && data.blocked.is_none(), "{data:?}");
        }
        other => panic!("expected a checkpoint, got {other:?}"),
    }
    assert_eq!(chunks[n - 2..], [UiChunk::FinishStep, UiChunk::Finish]);

    let messages = transcript::read(&env.state_dir, &env.project, CHAT);
    assert_eq!(messages.len(), 2, "{messages:?}");
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["parts"][0]["text"], "Add a note\nsecond line");
    assert_eq!(messages[1]["role"], "assistant");

    let record = history::get(&env.state_dir, &env.project, CHAT).unwrap();
    assert_eq!(record.agent_session_id.as_deref(), Some(FIXTURE_SESSION));
    assert_eq!(record.title.as_deref(), Some("Add a note"));

    let seen = events_until(&mut events, &env.project, "chat.turn").await;
    let busy: Vec<&Value> = seen
        .iter()
        .filter(|v| v["type"] == "terminal.busy")
        .map(|v| &v["busy"])
        .collect();
    assert_eq!(busy, [&json!(true), &json!(false)], "{seen:?}");
    assert_eq!(seen.last().unwrap()["chatId"], CHAT);
    assert_eq!(env.state.chats.running(&env.project), None);
    assert!(env.state.chats.subscribe(&env.project, CHAT).is_none());
}

#[tokio::test]
async fn second_start_conflicts() {
    let tmp = tempfile::tempdir().unwrap();
    let pidfile = tmp.path().join("child.pid");
    let env = env(fake("slow", Some(&pidfile))).await;
    let _sub = env
        .state
        .chats
        .start(env.state.clone(), request(&env, "one"))
        .unwrap();
    let err = env
        .state
        .chats
        .start(env.state.clone(), request(&env, "two"))
        .err()
        .expect("second start conflicts");
    assert_eq!(err.http_status(), 409);
    assert_eq!(err.message, "The agent is busy in Chat");
    let replay = env
        .state
        .chats
        .subscribe(&env.project, CHAT)
        .unwrap()
        .replay;
    assert!(matches!(replay[0], UiChunk::Start { .. }));
    env.state.chats.stop(&env.project).await;
    assert_eq!(env.state.chats.running(&env.project), None);
}

#[tokio::test]
async fn stop_kills_group_and_checkpoints() {
    let tmp = tempfile::tempdir().unwrap();
    let pidfile = tmp.path().join("child.pid");
    let env = env(fake("slow", Some(&pidfile))).await;
    let sub = env
        .state
        .chats
        .start(env.state.clone(), request(&env, "slow one"))
        .unwrap();

    let deadline = Instant::now() + WAIT;
    let pid = loop {
        let text = std::fs::read_to_string(&pidfile).unwrap_or_default();
        if !text.trim().is_empty() {
            break text.trim().to_string();
        }
        assert!(
            Instant::now() < deadline,
            "the fake never started its child"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert!(alive(&pid));
    std::fs::write(env.dir.join("notes.md"), "partial\n").unwrap();

    let stopped = Instant::now();
    env.state.chats.stop(&env.project).await;
    while alive(&pid) {
        assert!(
            stopped.elapsed() < Duration::from_secs(3),
            "child {pid} survived stop"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let chunks = collect(sub).await;
    let n = chunks.len();
    match &chunks[n - 2] {
        UiChunk::DataCheckpoint { data } => assert_eq!(data.files, ["notes.md"]),
        other => panic!("expected a checkpoint, got {other:?}"),
    }
    assert_eq!(chunks[n - 1], UiChunk::Abort);
    assert!(!chunks.contains(&UiChunk::Finish));
    assert_eq!(env.state.chats.running(&env.project), None);
    assert_eq!(
        transcript::read(&env.state_dir, &env.project, CHAT).len(),
        2
    );
}

#[tokio::test]
async fn missing_cli_is_not_installed() {
    let env = env(vec!["/nonexistent/cursor-agent".into()]).await;
    let sub = env
        .state
        .chats
        .start(env.state.clone(), request(&env, "hi"))
        .unwrap();
    let chunks = collect(sub).await;
    assert!(
        chunks.iter().any(|c| matches!(
            c,
            UiChunk::DataAgentError { data } if data.code == AgentErrorCode::NotInstalled
        )),
        "{chunks:?}"
    );
    assert_eq!(chunks.last(), Some(&UiChunk::Finish));
}
