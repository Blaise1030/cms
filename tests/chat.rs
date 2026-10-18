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

fn fake_cli() -> String {
    fixtures().join("fake_cli.sh").display().to_string()
}

/// A project whose `cursor` provider runs `argv` (and whose terminal-only `fake` provider runs
/// `fake_cli.sh`), with a `sessions.json` record for [`CHAT`].
async fn env(argv: Vec<String>) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let agents = tmp.path().join("agents.json");
    std::fs::write(
        &agents,
        json!({ "cursor": argv, "fake": [fake_cli()] }).to_string(),
    )
    .unwrap();
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

/// The pid of the `slow` fake's SIGTERM-ignoring child, once it is running.
async fn child_pid(pidfile: &Path) -> String {
    let deadline = Instant::now() + WAIT;
    loop {
        let text = std::fs::read_to_string(pidfile).unwrap_or_default();
        if !text.trim().is_empty() {
            let pid = text.trim().to_string();
            assert!(alive(&pid));
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "the fake never started its child"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
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

    let pid = child_pid(&pidfile).await;
    std::fs::write(env.dir.join("notes.md"), "partial\n").unwrap();

    let stopped = Instant::now();
    let (chats, project) = (env.state.chats.clone(), env.project.clone());
    let stop = tokio::spawn(async move { chats.stop(&project).await });
    loop {
        // Read before the liveness check: an idle manager means the group is already dead.
        let running = env.state.chats.running(&env.project);
        if !alive(&pid) {
            break;
        }
        assert!(running.is_some(), "turn ended while child {pid} still ran");
        assert!(
            stopped.elapsed() < Duration::from_secs(3),
            "child {pid} survived stop"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::timeout(WAIT, stop).await.unwrap().unwrap();

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
async fn shutdown_kills_running_turns() {
    let tmp = tempfile::tempdir().unwrap();
    let pidfile = tmp.path().join("child.pid");
    let env = env(fake("slow", Some(&pidfile))).await;
    let sub = env
        .state
        .chats
        .start(env.state.clone(), request(&env, "slow one"))
        .unwrap();
    let pid = child_pid(&pidfile).await;

    tokio::time::timeout(WAIT, env.state.chats.shutdown())
        .await
        .expect("shutdown is bounded");
    assert_eq!(env.state.chats.running(&env.project), None);
    let deadline = Instant::now() + Duration::from_secs(1);
    while alive(&pid) {
        assert!(Instant::now() < deadline, "child {pid} survived shutdown");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(collect(sub).await.last(), Some(&UiChunk::Abort));
}

#[tokio::test]
async fn server_shutdown_ends_watched_turns() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let env = env(fake("slow", None)).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (trigger, signal) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(workbench::serve_until(
        listener,
        env.state.clone(),
        async move {
            let _ = signal.await;
        },
    ));

    let body = json!({
        "id": CHAT,
        "messages": [{"id": "u1", "role": "user", "parts": [{"type": "text", "text": "slow one"}]}],
    })
    .to_string();
    let mut sock = tokio::net::TcpStream::connect(addr).await.unwrap();
    let head = format!(
        "POST /api/chat/{} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        env.project,
        body.len()
    );
    sock.write_all(head.as_bytes()).await.unwrap();
    sock.write_all(body.as_bytes()).await.unwrap();
    let mut seen = String::new();
    let mut buf = [0u8; 4096];
    while !seen.contains("\"type\":\"start\"") {
        let n = tokio::time::timeout(WAIT, sock.read(&mut buf))
            .await
            .expect("the turn starts streaming")
            .unwrap();
        assert!(n > 0, "the response ended early: {seen}");
        seen.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
    assert!(env.state.chats.running(&env.project).is_some());

    trigger.send(()).unwrap();
    tokio::time::timeout(WAIT, server)
        .await
        .expect("the server shuts down while a browser watches the turn")
        .unwrap()
        .unwrap();
    let mut rest = Vec::new();
    tokio::time::timeout(WAIT, sock.read_to_end(&mut rest))
        .await
        .expect("the stream closes")
        .unwrap();
    seen.push_str(&String::from_utf8_lossy(&rest));
    assert!(seen.contains("\"type\":\"abort\""), "{seen}");
    assert!(seen.contains("[DONE]"), "{seen}");
    assert_eq!(env.state.chats.running(&env.project), None);
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

/// `/api/chat/*` through the router.
mod http_routes {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Instant;

    use axum::body::Body;
    use axum::http::{header, HeaderMap, Request, StatusCode};
    use axum::response::Response;
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::ServiceExt;
    use workbench::chat::transcript;
    use workbench::projects::Source;
    use workbench::terminal::Output;
    use workbench::{git, http};

    use super::{env, fake, fake_cli, Env, CHAT, WAIT};

    const FIXTURE_TEXT: &str = "I added the line to `about.md`";

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

    async fn send(env: &Env, r: Request<Body>) -> Response {
        tokio::time::timeout(WAIT, http::router(env.state.clone()).oneshot(r))
            .await
            .expect("response head in time")
            .unwrap()
    }

    async fn call(env: &Env, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let res = send(env, req(method, uri, body)).await;
        let status = res.status();
        let text = read_to_end(res).await;
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// The whole body, which must end within [`WAIT`].
    async fn read_to_end(res: Response) -> String {
        let bytes = tokio::time::timeout(WAIT, res.into_body().collect())
            .await
            .expect("body ends in time")
            .unwrap()
            .to_bytes();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Reads `body` into `acc` until `acc` contains `needle`; `false` if the body ended first.
    async fn read_until(body: &mut Body, acc: &mut String, needle: &str) -> bool {
        let deadline = Instant::now() + WAIT;
        while !acc.contains(needle) {
            let left = deadline.saturating_duration_since(Instant::now());
            let frame = tokio::time::timeout(left, body.frame())
                .await
                .unwrap_or_else(|_| panic!("no {needle:?} in time; got {acc}"));
            match frame {
                Some(Ok(frame)) => {
                    if let Ok(data) = frame.into_data() {
                        acc.push_str(&String::from_utf8_lossy(&data));
                    }
                }
                Some(Err(e)) => panic!("body error: {e}"),
                None => return false,
            }
        }
        true
    }

    fn chat_body(id: &str, provider: Option<&str>, text: &str) -> Value {
        let mut body = json!({
            "id": id,
            "trigger": "submit-message",
            "messages": [
                {"id": "a1", "role": "assistant", "parts": [{"type": "text", "text": "earlier"}]},
                {"id": "u1", "role": "user", "parts": [
                    {"type": "text", "text": text},
                    {"type": "file", "url": "data:,x", "mediaType": "text/plain"},
                ]},
            ],
        });
        if let Some(p) = provider {
            body["provider"] = json!(p);
        }
        body
    }

    fn post_chat(env: &Env, body: Value) -> Request<Body> {
        req("POST", &format!("/api/chat/{}", env.project), Some(body))
    }

    fn header<'a>(headers: &'a HeaderMap, name: &str) -> &'a str {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    }

    /// Ends the project's running turn and waits for it.
    async fn stop_turn(env: &Env) -> Value {
        let (s, v) = call(
            env,
            "DELETE",
            &format!("/api/chat/{}/turn", env.project),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        v
    }

    #[tokio::test]
    async fn post_returns_ai_sdk_sse() {
        let env = env(fake("default", None)).await;
        let res = send(
            &env,
            post_chat(
                &env,
                chat_body("new-chat", Some("cursor"), "Write a haiku\nplease"),
            ),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let h = res.headers().clone();
        assert_eq!(header(&h, "content-type"), "text/event-stream");
        assert_eq!(header(&h, "cache-control"), "no-cache");
        assert_eq!(header(&h, "x-vercel-ai-ui-message-stream"), "v1");
        assert_eq!(header(&h, "x-accel-buffering"), "no");

        let body = read_to_end(res).await;
        assert!(body.ends_with("\n\n"), "{body}");
        let frames: Vec<&str> = body.trim_end().split("\n\n").collect();
        assert!(frames.iter().all(|f| f.starts_with("data: ")), "{body}");
        assert_eq!(frames.last(), Some(&"data: [DONE]"));
        let first: Value = serde_json::from_str(&frames[0]["data: ".len()..]).unwrap();
        assert_eq!(first["type"], "start");
        assert!(body.contains(FIXTURE_TEXT), "{body}");

        let (s, v) = call(
            &env,
            "GET",
            &format!("/api/chat/{}/new-chat", env.project),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let roles: Vec<&Value> = v["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| &m["role"])
            .collect();
        assert_eq!(roles, [&json!("user"), &json!("assistant")], "{v}");
        assert_eq!(
            v["messages"][0]["parts"][0]["text"],
            "Write a haiku\nplease"
        );
        assert_eq!(v["provider"], "cursor");
        assert_eq!(v["running"], false);

        let (s, v) = call(&env, "GET", &format!("/api/chat/{}", env.project), None).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let chats = v["chats"].as_array().unwrap();
        assert_eq!(chats.len(), 1, "only records with a transcript: {v}");
        assert_eq!(chats[0]["id"], "new-chat");
        assert_eq!(chats[0]["provider"], "cursor");
        assert_eq!(chats[0]["title"], "Write a haiku");
        assert!(chats[0]["updatedAt"].is_u64(), "{v}");
        assert_eq!(v["running"], Value::Null);

        let record =
            workbench::terminal::history::get(&env.state_dir, &env.project, "new-chat").unwrap();
        assert_eq!(record.provider, "cursor");

        let (s, _) = call(&env, "GET", "/api/chat/nope", None).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn resume_replays_without_duplicates() {
        let env = env(fake("slow", None)).await;
        let stream = format!("/api/chat/{}/{CHAT}/stream", env.project);
        let posted = send(&env, post_chat(&env, chat_body(CHAT, None, "slow one"))).await;
        assert_eq!(posted.status(), StatusCode::OK);

        let res = send(&env, req("GET", &stream, None)).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(header(res.headers(), "content-type"), "text/event-stream");
        let mut body = res.into_body();
        let mut acc = String::new();
        assert!(read_until(&mut body, &mut acc, FIXTURE_TEXT).await, "{acc}");

        let (s, v) = call(&env, "GET", &format!("/api/chat/{}", env.project), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["running"], CHAT);
        let (_, v) = call(
            &env,
            "GET",
            &format!("/api/chat/{}/{CHAT}", env.project),
            None,
        )
        .await;
        assert_eq!(v["running"], true);

        assert_eq!(stop_turn(&env).await["stopped"], true);
        assert!(
            read_until(&mut body, &mut acc, "data: [DONE]\n\n").await,
            "{acc}"
        );
        assert_eq!(acc.matches(r#""type":"start""#).count(), 1, "{acc}");
        assert_eq!(acc.matches(FIXTURE_TEXT).count(), 1, "{acc}");
        drop(posted);

        let res = send(&env, req("GET", &stream, None)).await;
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        assert_eq!(stop_turn(&env).await["stopped"], false);
    }

    #[tokio::test]
    async fn rejects_bad_input() {
        let env = env(fake("default", None)).await;
        let cases = [
            (chat_body("../x", Some("cursor"), "hi"), "invalid chat id"),
            (
                chat_body("fresh", Some("opencode"), "hi"),
                "Chat supports Cursor CLI, Claude Code and Codex",
            ),
            (
                chat_body("fresh", None, "hi"),
                "provider is required for a new chat",
            ),
            (chat_body(CHAT, None, "  "), ""),
            (
                json!({"id": CHAT, "messages": [{"role": "assistant", "parts": [{"type": "text", "text": "x"}]}]}),
                "",
            ),
            (json!({"id": CHAT}), ""),
        ];
        for (body, message) in cases {
            let res = send(&env, post_chat(&env, body.clone())).await;
            let status = res.status();
            let v: Value = serde_json::from_str(&read_to_end(res).await).unwrap_or(Value::Null);
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {v}");
            let got = v["error"]["message"].as_str().unwrap_or("");
            assert!(got.contains(message), "{body}: {v}");
        }
        assert!(workbench::terminal::history::get(&env.state_dir, &env.project, "fresh").is_none());
        assert_eq!(env.state.chats.running(&env.project), None);

        let res = send(
            &env,
            req(
                "POST",
                "/api/chat/nope",
                Some(chat_body("fresh", Some("cursor"), "hi")),
            ),
        )
        .await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn failed_first_turn_is_not_resumed() {
        let env = env(fake("default", None)).await;
        let agents = env.state.config.agents_config.clone().unwrap();
        let use_claude = |argv: Vec<String>| {
            std::fs::write(&agents, json!({ "claude": argv }).to_string()).unwrap();
        };
        use_claude(vec!["/nonexistent/claude".into()]);
        let res = send(
            &env,
            post_chat(&env, chat_body("first", Some("claude"), "hi")),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let body = read_to_end(res).await;
        assert!(body.contains("not_installed"), "{body}");
        let record =
            workbench::terminal::history::get(&env.state_dir, &env.project, "first").unwrap();
        assert_eq!(
            record.agent_session_id, None,
            "an unconfirmed session id is forgotten"
        );

        let argv_file = env.state_dir.join("argv.txt");
        let fixture = super::fixtures().join("chat/claude/turn.jsonl");
        let script = format!(
            "printf '%s\\n' \"$@\" > '{}'; cat '{}'",
            argv_file.display(),
            fixture.display()
        );
        use_claude(vec!["sh".into(), "-c".into(), script, "claude".into()]);
        let res = send(&env, post_chat(&env, chat_body("first", None, "again"))).await;
        assert_eq!(res.status(), StatusCode::OK);
        read_to_end(res).await;
        let argv = std::fs::read_to_string(&argv_file).unwrap();
        let argv: Vec<&str> = argv.lines().collect();
        assert!(!argv.contains(&"--resume"), "{argv:?}");
        assert!(argv.contains(&"--session-id"), "{argv:?}");
    }

    #[tokio::test]
    async fn get_routes_reject_bad_chat_ids() {
        let env = env(fake("default", None)).await;
        let long = "a".repeat(65);
        for id in ["bad.id", long.as_str(), "..%2Fx"] {
            for suffix in ["", "/stream"] {
                let uri = format!("/api/chat/{}/{id}{suffix}", env.project);
                let (s, v) = call(&env, "GET", &uri, None).await;
                assert_eq!(s, StatusCode::BAD_REQUEST, "{uri}: {v}");
                assert!(
                    v["error"]["message"]
                        .as_str()
                        .unwrap_or("")
                        .contains("invalid chat id"),
                    "{uri}: {v}"
                );
            }
        }
    }

    #[tokio::test]
    async fn chat_and_terminal_exclusive() {
        let env = env(fake("slow", None)).await;
        let terminal = json!({ "project": env.project, "provider": "fake" });
        let (s, v) = call(&env, "POST", "/api/terminal", Some(terminal.clone())).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let (s, v) = call(
            &env,
            "POST",
            &format!("/api/chat/{}", env.project),
            Some(chat_body(CHAT, None, "hi")),
        )
        .await;
        assert_eq!(s, StatusCode::CONFLICT, "{v}");
        assert_eq!(v["error"]["message"], "The agent is busy in Terminal");
        env.state.terminals.kill(&env.project).await;

        let posted = send(&env, post_chat(&env, chat_body(CHAT, None, "slow one"))).await;
        assert_eq!(posted.status(), StatusCode::OK);
        let (s, v) = call(&env, "POST", "/api/terminal", Some(terminal)).await;
        assert_eq!(s, StatusCode::CONFLICT, "{v}");
        assert_eq!(v["error"]["message"], "The agent is busy in Chat");
        assert_eq!(env.state.terminals.status(&env.project), None);
        stop_turn(&env).await;
    }

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
        // Tracked, so the `edit` fake's append dirties the tree.
        std::fs::write(src.join("notes.md"), "notes\n").unwrap();
        git_cli(&src, &["add", "-A"]);
        git_cli(&src, &["commit", "-m", "init"]);
        git_cli(
            tmp,
            &["clone", "--bare", src.to_str().unwrap(), "remote.git"],
        );
        tmp.join("remote.git")
    }

    #[tokio::test]
    async fn blocked_on_main() {
        let env = env(fake("edit", None)).await;
        let git_dir = env.state.config.projects_dir.join("git");
        let bare = seed_bare(&git_dir);
        let gh = env
            .state
            .projects
            .create_from(
                "gh-site",
                Source::Clone {
                    repo: "acme/site".into(),
                    url: format!("file://{}", bare.display()),
                    env: vec![],
                },
                &git::Identity::workbench(),
            )
            .await
            .unwrap();
        let res = send(
            &env,
            req(
                "POST",
                &format!("/api/chat/{}", gh.id),
                Some(chat_body("on-main", Some("cursor"), "edit")),
            ),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let body = read_to_end(res).await;
        let checkpoint = body
            .split("\n\n")
            .filter_map(|f| f.strip_prefix("data: "))
            .filter_map(|f| serde_json::from_str::<Value>(f).ok())
            .find(|v| v["type"] == "data-checkpoint")
            .unwrap_or_else(|| panic!("no data-checkpoint: {body}"));
        assert!(checkpoint["data"]["blocked"].is_string(), "{checkpoint}");
    }

    /// The hook token the token-echoing terminal printed.
    async fn hook_token(env: &Env) -> String {
        let attach = env.state.terminals.attach(&env.project).unwrap();
        let mut acc = String::from_utf8_lossy(&attach.replay).into_owned();
        let mut rx = attach.rx;
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(rest) = acc.split("TOKEN=").nth(1) {
                let token: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
                if token.len() == 32 {
                    return token;
                }
            }
            let left = deadline.saturating_duration_since(Instant::now());
            match tokio::time::timeout(left, rx.recv()).await {
                Ok(Ok(Output::Data(data))) => acc.push_str(&String::from_utf8_lossy(&data)),
                other => panic!("no TOKEN= ({other:?}): {acc:?}"),
            }
        }
    }

    #[tokio::test]
    async fn terminal_turn_marks_transcript() {
        let script = format!("echo TOKEN=$WORKBENCH_HOOK_TOKEN; exec {}", fake_cli());
        let env = env(vec!["sh".into(), "-c".into(), script]).await;
        transcript::append(
            &env.state_dir,
            &env.project,
            CHAT,
            &[transcript::user_message("u1", "hi")],
        )
        .unwrap();
        let (s, v) = call(
            &env,
            "POST",
            "/api/terminal",
            Some(json!({ "project": env.project, "provider": "cursor", "resume": CHAT })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["sessionId"], CHAT);
        let token = hook_token(&env).await;
        let (s, v) = call(
            &env,
            "POST",
            "/api/terminal/turn-end",
            Some(json!({ "project": env.project, "token": token })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        env.state.terminals.kill(&env.project).await;

        let messages = transcript::read(&env.state_dir, &env.project, CHAT);
        assert_eq!(messages.len(), 2, "{messages:?}");
        assert_eq!(messages[1]["parts"][0]["type"], "data-terminal");
    }
}
