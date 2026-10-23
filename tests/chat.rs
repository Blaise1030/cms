//! `/api/chat/*` against the fake ACP agent. The process argv is installed
//! with `providers::set_test_acp_argv` until Task 6 stores it on the provider.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::config::Config;
use orso::http;
use orso::state::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;

fn argv() -> Vec<String> {
    let (cmd, args) = orso::chat::acp::fake_agent();
    let mut all = vec![cmd];
    all.extend(args);
    all
}

struct Env {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    state: AppState,
    project: String,
}

async fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let config = Config {
        projects_dir: root.clone(),
        no_auth: true,
        ..Default::default()
    };
    let state = AppState::with_scaffold_install(config, false);
    let info = state.projects.create("Site").await.unwrap();
    orso::providers::set_test_acp_argv("fake", argv());
    Env {
        _tmp: tmp,
        root,
        state,
        project: info.id,
    }
}

fn app(env: &Env) -> axum::Router {
    http::router(env.state.clone())
}

fn req(method: &str, path: &str, body: Option<Value>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
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

async fn call(env: &Env, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let res = app(env).oneshot(req(method, path, body)).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn create_chat(env: &Env, model: Option<&str>) -> String {
    let mut body = json!({ "provider": "fake" });
    if let Some(model) = model {
        body["model"] = json!(model);
    }
    let (status, body) = call(
        env,
        "POST",
        &format!("/api/chat/{}", env.project),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["provider"], "fake");
    assert_eq!(body["live"], false);
    body["id"].as_str().unwrap().to_string()
}

fn chunk_text(body: &Value) -> String {
    body["chunks"]
        .as_array()
        .map(|chunks| {
            chunks
                .iter()
                .filter_map(|c| c["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

async fn wait_detail(env: &Env, chat: &str, pred: impl Fn(&Value) -> bool, what: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    let path = format!("/api/chat/{}/{}", env.project, chat);
    loop {
        let (status, body) = call(env, "GET", &path, None).await;
        if status == StatusCode::OK && pred(&body) {
            return body;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}; last {status} {body}"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}

async fn wait_text(env: &Env, chat: &str, needle: &str) -> Value {
    let label = format!("text {needle:?}");
    let needle = needle.to_string();
    wait_detail(
        env,
        chat,
        move |body| chunk_text(body).contains(&needle),
        &label,
    )
    .await
}

async fn read_until(body: &mut Body, buf: &mut String, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !buf.contains(needle) {
        let left = deadline.saturating_duration_since(Instant::now());
        let frame = tokio::time::timeout(left, body.frame())
            .await
            .unwrap_or_else(|_| panic!("timed out; sse so far: {buf}"))
            .unwrap_or_else(|| panic!("stream ended; sse so far: {buf}"))
            .expect("sse frame");
        if let Ok(data) = frame.into_data() {
            buf.push_str(&String::from_utf8_lossy(&data));
        }
    }
}

#[tokio::test]
async fn chat_message_against_fake_acp_appends_transcript() {
    let env = env().await;
    let id = create_chat(&env, Some("fast")).await;
    let (status, body) = call(
        &env,
        "POST",
        &format!("/api/chat/{}/{id}/message", env.project),
        Some(json!({ "text": "hello" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");

    let detail = wait_text(&env, &id, "fake reply: hello").await;
    assert_eq!(detail["provider"], "fake");
    assert_eq!(detail["model"], "fast");
    assert_eq!(detail["running"], true);
    assert!(chunk_text(&detail).contains("hello"));

    let (status, list) = call(&env, "GET", &format!("/api/chat/{}", env.project), None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let chat = list["chats"]
        .as_array()
        .and_then(|chats| chats.iter().find(|c| c["id"] == id))
        .unwrap_or_else(|| panic!("missing chat in {list}"));
    assert_eq!(chat["provider"], "fake");
    assert_eq!(chat["model"], "fast");
    assert_eq!(chat["live"], true);
    assert!(chat["updatedAt"].as_u64().is_some(), "{chat}");

    let path = env
        .root
        .join(".orso")
        .join(&env.project)
        .join("chats")
        .join(format!("{id}.jsonl"));
    let transcript = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e} at {path:?}"));
    assert!(
        transcript.contains("fake reply: hello"),
        "transcript: {transcript}"
    );
    assert!(
        transcript.contains("\"type\":\"user_text\""),
        "{transcript}"
    );
    env.state.chats.shutdown().await;
}

#[tokio::test]
async fn chat_stream_replays_then_live() {
    let env = env().await;
    let id = create_chat(&env, None).await;
    let (status, _) = call(
        &env,
        "POST",
        &format!("/api/chat/{}/{id}/message", env.project),
        Some(json!({ "text": "replay-me" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    wait_text(&env, &id, "fake reply: replay-me").await;

    let res = app(&env)
        .oneshot(req(
            "GET",
            &format!("/api/chat/{}/{id}/stream", env.project),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/event-stream")),
        "{:?}",
        res.headers()
    );
    let mut body = res.into_body();
    let mut buf = String::new();
    read_until(&mut body, &mut buf, "fake reply: replay-me").await;

    let (status, _) = call(
        &env,
        "POST",
        &format!("/api/chat/{}/{id}/message", env.project),
        Some(json!({ "text": "live-me" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    read_until(&mut body, &mut buf, "fake reply: live-me").await;
    let replayed_turns = buf.matches("\"type\":\"turn_end\"").count();
    let deadline = Instant::now() + Duration::from_secs(15);
    while buf.matches("\"type\":\"turn_end\"").count() <= replayed_turns {
        let left = deadline.saturating_duration_since(Instant::now());
        let frame = tokio::time::timeout(left, body.frame())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for live turn_end; sse: {buf}"))
            .unwrap_or_else(|| panic!("stream ended before live turn_end; sse: {buf}"))
            .expect("sse frame");
        if let Ok(data) = frame.into_data() {
            buf.push_str(&String::from_utf8_lossy(&data));
        }
    }
    assert!(!buf.contains("[DONE]"), "sse: {buf}");

    // The turn is finished and the response is still open: no trailer, no close.
    let pending = tokio::time::timeout(Duration::from_millis(300), body.frame()).await;
    assert!(pending.is_err(), "stream closed after the live turn: {buf}");
    drop(body);
    env.state.chats.shutdown().await;
}

#[tokio::test]
async fn chat_stop_cancels_turn() {
    let env = env().await;
    let id = create_chat(&env, None).await;
    let (status, body) = call(
        &env,
        "POST",
        &format!("/api/chat/{}/{id}/message", env.project),
        Some(json!({ "text": "slow please" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    wait_text(&env, &id, "slow started").await;

    let (status, body) = call(
        &env,
        "POST",
        &format!("/api/chat/{}/{id}/stop", env.project),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let detail = wait_detail(
        &env,
        &id,
        |body| {
            body["chunks"].as_array().is_some_and(|chunks| {
                chunks
                    .iter()
                    .any(|c| c["type"] == "turn_end" && c["stop_reason"] == "cancelled")
            })
        },
        "turn_end cancelled",
    )
    .await;
    assert!(
        detail["chunks"].as_array().is_some_and(|chunks| {
            chunks
                .iter()
                .any(|c| c["type"] == "turn_end" && c["stop_reason"] == "cancelled")
        }),
        "{detail}"
    );
    env.state.chats.shutdown().await;
}

#[tokio::test]
async fn chat_approve_permission() {
    let env = env().await;
    let id = create_chat(&env, None).await;
    let (status, body) = call(
        &env,
        "POST",
        &format!("/api/chat/{}/{id}/message", env.project),
        Some(json!({ "text": "permission please" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");

    let deadline = Instant::now() + Duration::from_secs(15);
    let request_id = loop {
        let (status, body) = call(
            &env,
            "GET",
            &format!("/api/chat/{}/{id}", env.project),
            None,
        )
        .await;
        let found = body["chunks"].as_array().and_then(|chunks| {
            chunks.iter().find_map(|c| {
                (c["type"] == "permission")
                    .then(|| c["request_id"].as_str().map(str::to_string))
                    .flatten()
            })
        });
        if let Some(id) = found {
            break id;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for permission; last {status} {body}"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    };

    let (status, body) = call(
        &env,
        "POST",
        &format!("/api/chat/{}/{id}/approve", env.project),
        Some(json!({ "requestId": request_id, "allow": true })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let detail = wait_text(&env, &id, "permission outcome: selected:allow").await;
    assert!(chunk_text(&detail).contains("permission outcome: selected:allow"));
    env.state.chats.shutdown().await;
}

#[tokio::test]
async fn chat_id_traversal_rejected() {
    let env = env().await;
    let project = &env.project;
    for id in ["..%2Fevil", "foo%2Fbar", &"a".repeat(65)] {
        let (status, body) = call(
            &env,
            "POST",
            &format!("/api/chat/{project}/{id}/message"),
            Some(json!({ "text": "hi" })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{id}: {body}");
        assert_eq!(body["error"]["code"], "BADREQ", "{id}: {body}");
    }

    let state = env.root.join(".orso");
    assert!(!state.join("evil.jsonl").exists());
    assert!(!state.join(project).join("evil.jsonl").exists());
    assert!(!state
        .join(project)
        .join("chats")
        .join("..%2Fevil.jsonl")
        .exists());
    assert!(!env.root.join("evil.jsonl").exists());

    // `turn` is an ordinary chat id. It is not the removed stop-all route.
    let (status, body) = call(&env, "DELETE", &format!("/api/chat/{project}/turn"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("no such chat")),
        "{body}"
    );
    let (status, body) = call(&env, "GET", &format!("/api/chat/{project}/turn"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_ne!(body["error"]["message"], "route not found");
}

async fn wait_path(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn delete_during_handshake_does_not_keep_terminal_blocked() {
    let env = env().await;
    let gate = env.root.join("hold");
    let (cmd, args) = orso::chat::acp::fake_agent_hold(&gate);
    let mut argv = vec![cmd];
    argv.extend(args);
    orso::providers::set_test_acp_argv("hold", argv);

    let (status, body) = call(
        &env,
        "POST",
        &format!("/api/chat/{}", env.project),
        Some(json!({ "provider": "hold" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["id"].as_str().unwrap().to_string();

    let state = env.state.clone();
    let project = env.project.clone();
    let start_id = id.clone();
    let started = tokio::spawn(async move {
        let res = http::router(state)
            .oneshot(req(
                "POST",
                &format!("/api/chat/{project}/{start_id}/start"),
                None,
            ))
            .await
            .unwrap();
        res.status()
    });
    wait_path(&std::path::PathBuf::from(format!(
        "{}.ready",
        gate.display()
    )))
    .await;
    assert!(
        env.state.chats.any_live(&env.project),
        "terminal start would not be blocked yet"
    );

    let (status, body) = call(
        &env,
        "DELETE",
        &format!("/api/chat/{}/{id}", env.project),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        !env.state.chats.any_live(&env.project),
        "delete left the handshake up; terminal start stays 409"
    );

    std::fs::write(format!("{}.go", gate.display()), b"").unwrap();
    let start_status = started.await.unwrap();
    assert_ne!(
        start_status,
        StatusCode::ACCEPTED,
        "start published after delete"
    );
    assert_ne!(start_status, StatusCode::NO_CONTENT);
    assert!(!env.state.chats.any_live(&env.project));
    assert!(!env.state.chats.is_live(&env.project, &id));

    for (method, path) in [
        ("GET", format!("/api/chat/{}/{id}", env.project)),
        ("DELETE", format!("/api/chat/{}/{id}", env.project)),
        ("POST", format!("/api/chat/{}/{id}/stop", env.project)),
    ] {
        let (status, body) = call(&env, method, &path, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {body}");
    }
    env.state.chats.shutdown().await;
}

#[tokio::test]
async fn overlapping_cold_messages_with_stored_model_both_send() {
    let env = env().await;
    let id = create_chat(&env, Some("smart")).await;
    let gate = orso::chat::manager::arm_model_gate(&env.project, &id);
    let path = format!("/api/chat/{}/{id}/message", env.project);

    // "slow" holds the first turn open so the other set_model runs mid-turn.
    let alpha = call(
        &env,
        "POST",
        &path,
        Some(json!({ "text": "slow alpha-msg" })),
    );
    let beta = call(
        &env,
        "POST",
        &path,
        Some(json!({ "text": "slow beta-msg" })),
    );
    let ((), (status_a, body_a), (status_b, body_b)) = tokio::join!(
        async {
            let deadline = Instant::now() + Duration::from_secs(15);
            while gate.entered() < 2 {
                assert!(
                    Instant::now() < deadline,
                    "both messages did not reach set_model (entered {})",
                    gate.entered()
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            gate.release_first();
            let _ = wait_detail(
                &env,
                &id,
                |body| {
                    let text = chunk_text(body);
                    text.contains("alpha-msg") || text.contains("beta-msg")
                },
                "first cold message recorded",
            )
            .await;
            gate.release_rest();
        },
        alpha,
        beta,
    );
    assert_eq!(status_a, StatusCode::ACCEPTED, "{body_a}");
    assert_eq!(status_b, StatusCode::ACCEPTED, "{body_b}");
    let detail = wait_detail(
        &env,
        &id,
        |body| {
            let text = chunk_text(body);
            text.contains("alpha-msg") && text.contains("beta-msg")
        },
        "both messages recorded",
    )
    .await;
    assert_eq!(detail["model"], "smart");
    env.state.chats.shutdown().await;
}
