//! ACP bridge tests against a scripted fake agent (`examples/fake_acp_agent.rs`).
//! No real Claude/Codex/Opencode install is needed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::broadcast::Receiver;
use tower::ServiceExt;
use workbench::config::Config;
use workbench::state::AppState;
use workbench::{acp, http};

const WAIT: Duration = Duration::from_secs(15);

// ---------------------------------------------------------------- harness

/// Path of the compiled fake agent, building it if `cargo test` did not.
fn fake_agent() -> &'static Path {
    static EXE: OnceLock<PathBuf> = OnceLock::new();
    EXE.get_or_init(|| {
        let deps = std::env::current_exe().unwrap();
        let profile_dir = deps.parent().unwrap().parent().unwrap();
        let exe = profile_dir.join("examples").join("fake_acp_agent");
        if !exe.exists() {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let ok = Command::new(cargo)
                .args(["build", "--example", "fake_acp_agent"])
                .status()
                .unwrap()
                .success();
            assert!(ok && exe.exists(), "could not build fake_acp_agent");
        }
        exe
    })
}

struct Env {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    state: AppState,
    app: axum::Router,
    project: String,
    dir: PathBuf,
    log: PathBuf,
    pidfile: PathBuf,
}

fn config(root: &Path, agents: &Path, token: Option<&str>) -> Config {
    Config {
        port: 0,
        origin: None,
        projects_dir: root.to_path_buf(),
        no_auth: token.is_none(),
        token: token.map(str::to_string),
        agents_config: Some(agents.to_path_buf()),
        ..Default::default()
    }
}

async fn env_with(token: Option<&str>) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let log = root.join("agent.log");
    let pidfile = root.join("agent.pid");
    let agents = root.join("agents.json");
    let exe = fake_agent().to_string_lossy().into_owned();
    std::fs::write(
        &agents,
        json!({
            "fake": [exe, "--log", log, "--pidfile", pidfile],
            "stubborn": [exe, "--log", log, "--pidfile", pidfile, "--ignore-eof"],
            "missing": ["/nonexistent/acp-agent"],
        })
        .to_string(),
    )
    .unwrap();
    let state = AppState::with_scaffold_install(config(&root, &agents, token), false);
    state.agents.set_settle_quiet(Duration::from_millis(150));
    let info = state.projects.create("Site").await.unwrap();
    let dir = root.join(&info.id);
    let app = http::router(state.clone());
    Env {
        _tmp: tmp,
        root,
        state,
        app,
        project: info.id,
        dir,
        log,
        pidfile,
    }
}

async fn env() -> Env {
    env_with(None).await
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

impl Env {
    async fn call(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let res = self
            .app
            .clone()
            .oneshot(req(method, uri, body))
            .await
            .unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn session(&self, provider: &str) -> String {
        let (s, v) = self
            .call(
                "POST",
                "/api/agent/session",
                Some(json!({ "project": self.project, "provider": provider })),
            )
            .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        v["sessionId"].as_str().unwrap().to_string()
    }

    async fn prompt(&self, session: &str, text: &str) -> String {
        let (s, v) = self.prompt_raw(session, text).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        v["turnId"].as_str().unwrap().to_string()
    }

    async fn prompt_raw(&self, session: &str, text: &str) -> (StatusCode, Value) {
        self.call(
            "POST",
            "/api/agent/prompt",
            Some(json!({ "project": self.project, "sessionId": session, "text": text })),
        )
        .await
    }

    async fn decide(&self, request_id: &str, decision: &str) -> (StatusCode, Value) {
        self.call(
            "POST",
            &format!("/api/agent/permission/{request_id}"),
            Some(json!({ "decision": decision })),
        )
        .await
    }

    fn events(&self) -> Events {
        Events {
            rx: self.state.events.subscribe(),
            seen: Vec::new(),
        }
    }

    fn logged(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    fn pid(&self) -> i32 {
        std::fs::read_to_string(&self.pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    }
}

/// Collects events in order; `seen` keeps everything consumed so far.
struct Events {
    rx: Receiver<std::sync::Arc<str>>,
    seen: Vec<Value>,
}

impl Events {
    /// Consume events until one of `kind` (optionally satisfying `pred`) arrives.
    async fn wait_for(&mut self, kind: &str, pred: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let text = tokio::time::timeout(left, self.rx.recv())
                .await
                .unwrap_or_else(|_| panic!("timed out waiting for {kind}; seen: {:#?}", self.seen))
                .unwrap();
            let v: Value = serde_json::from_str(&text).unwrap();
            self.seen.push(v.clone());
            if v["type"] == kind && pred(&v) {
                return v;
            }
        }
    }

    async fn wait(&mut self, kind: &str) -> Value {
        self.wait_for(kind, |_| true).await
    }

    fn types(&self) -> Vec<&str> {
        self.seen
            .iter()
            .map(|v| v["type"].as_str().unwrap())
            .collect()
    }

    fn text(&self) -> String {
        self.seen
            .iter()
            .filter(|v| v["type"] == "chat.chunk")
            .map(|v| v["delta"].as_str().unwrap())
            .collect()
    }
}

/// History is written once a turn has settled (after `chat.done`), so poll for it.
async fn wait_turns(e: &Env, n: usize) -> Vec<acp::TurnRecord> {
    let deadline = Instant::now() + WAIT;
    loop {
        let turns = e.state.agents.history(&e.project, None).await.unwrap();
        if turns.len() >= n {
            return turns;
        }
        assert!(
            Instant::now() < deadline,
            "history never reached {n} turns: {turns:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only probes for existence.
    unsafe { libc::kill(pid, 0) == 0 }
}

async fn wait_gone(pid: i32) {
    let deadline = Instant::now() + WAIT;
    while alive(pid) {
        assert!(Instant::now() < deadline, "process {pid} still alive");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

// ------------------------------------------------------------------ tests

#[tokio::test]
async fn providers_come_from_the_registry_file() {
    let e = env().await;
    let (s, v) = e.call("GET", "/api/agents", None).await;
    assert_eq!(s, StatusCode::OK);
    let ids: Vec<&str> = v["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["fake", "missing", "stubborn"]);
    assert_eq!(
        v["providers"][1]["command"],
        json!(["/nonexistent/acp-agent"])
    );
}

#[tokio::test]
async fn default_registry_when_no_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut cfg = config(tmp.path(), &tmp.path().join("absent.json"), None);
    cfg.agents_config = Some(tmp.path().join("absent.json"));
    let st = AppState::with_scaffold_install(cfg, false);
    let res = http::router(st)
        .oneshot(req("GET", "/api/agents", None))
        .await
        .unwrap();
    let v: Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let ids: Vec<&str> = v["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["claude", "codex", "opencode"]);
}

#[tokio::test]
async fn session_prompt_streams_and_persists_history() {
    let e = env().await;
    let mut ev = e.events();
    let sid = e.session("fake").await;

    // initialize + session/new were spoken with the right params.
    let log = e.logged();
    let init = log.iter().find(|m| m["method"] == "initialize").unwrap();
    assert_eq!(init["params"]["protocolVersion"], 1);
    let new = log.iter().find(|m| m["method"] == "session/new").unwrap();
    assert_eq!(new["params"]["cwd"], e.dir.to_string_lossy().as_ref());
    assert_eq!(new["params"]["mcpServers"], json!([]));

    let turn = e.prompt(&sid, "hello there").await;
    let done = ev.wait("chat.done").await;
    assert_eq!(done["turnId"], turn);
    assert_eq!(done["sessionId"], sid);
    assert_eq!(done["projectId"], e.project);
    assert_eq!(done["stopReason"], "end_turn");
    assert_eq!(ev.text(), "Hello world");
    wait_turns(&e, 1).await;
    // The prompt reached the agent as a text content block.
    let prompt = e
        .logged()
        .into_iter()
        .find(|m| m["method"] == "session/prompt")
        .unwrap();
    assert_eq!(prompt["params"]["sessionId"], sid);
    assert_eq!(
        prompt["params"]["prompt"],
        json!([{ "type": "text", "text": "hello there" }])
    );

    // History: via REST and as JSONL outside the site repo.
    let (s, h) = e
        .call(
            "GET",
            &format!("/api/agent/history?project={}", e.project),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let t = &h["turns"][0];
    assert_eq!(h["turns"].as_array().unwrap().len(), 1);
    assert_eq!(t["id"], turn);
    assert_eq!(t["sessionId"], sid);
    assert_eq!(t["provider"], "fake");
    assert_eq!(t["userText"], "hello there");
    assert_eq!(t["assistantText"], "Hello world");
    assert_eq!(t["tools"], json!([]));
    assert!(t["ts"].as_i64().unwrap() > 0);

    let file = acp::history_path(&e.root, &e.project);
    assert_eq!(
        file,
        e.root
            .join(".workbench")
            .join(&e.project)
            .join("history.jsonl")
    );
    assert!(file.is_file());
    assert!(!e.dir.join("history.jsonl").exists());
    // Nothing the daemon wrote lands in the site repo.
    assert_eq!(git(&e.dir, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn file_edits_are_auto_approved_and_checkpointed() {
    let e = env().await;
    let mut ev = e.events();
    let sid = e.session("fake").await;
    e.prompt(&sid, "edit the notes\nsecond line").await;

    let cp = ev.wait("checkpoint.created").await;
    assert_eq!(cp["projectId"], e.project);
    assert_eq!(cp["summary"], "edit the notes");
    assert_eq!(cp["author"], "Workbench");
    assert!(cp["ts"].as_i64().unwrap() > 1_600_000_000);
    assert!(
        !ev.types().contains(&"permission.request"),
        "{:?}",
        ev.types()
    );
    assert_eq!(
        std::fs::read_to_string(e.dir.join("notes.txt")).unwrap(),
        "edited by fake agent\n"
    );

    // The checkpoint really is the HEAD commit and includes the edit; tree is clean.
    assert_eq!(
        git(&e.dir, &["rev-parse", "HEAD"]),
        cp["sha"].as_str().unwrap()
    );
    assert_eq!(
        git(&e.dir, &["log", "-1", "--format=%s"]),
        "checkpoint: edit the notes"
    );
    assert!(git(&e.dir, &["show", "--stat", "--format=", "HEAD"]).contains("notes.txt"));
    assert_eq!(git(&e.dir, &["status", "--porcelain"]), "");

    // Ordering: tool running → tool done → chat.done → checkpoint.created.
    let states: Vec<&str> = ev
        .seen
        .iter()
        .filter(|v| v["type"] == "chat.tool")
        .map(|v| v["state"].as_str().unwrap())
        .collect();
    assert_eq!(states, ["running", "done"]);
    let tool = ev.seen.iter().find(|v| v["type"] == "chat.tool").unwrap();
    assert_eq!(tool["tool"], "edit");
    assert_eq!(tool["summary"], "Edit notes.txt");
    let types = ev.types();
    let pos = |t: &str| types.iter().position(|x| *x == t).unwrap();
    assert!(pos("chat.done") < pos("checkpoint.created"));

    // History records the tool summary.
    let (_, h) = e
        .call(
            "GET",
            &format!("/api/agent/history?project={}", e.project),
            None,
        )
        .await;
    assert_eq!(
        h["turns"][0]["tools"],
        json!([{ "tool": "edit", "summary": "Edit notes.txt", "state": "done" }])
    );
    // And the checkpoint is visible through the checkpoints API.
    let (_, log) = e
        .call(
            "GET",
            &format!("/api/checkpoints?project={}", e.project),
            None,
        )
        .await;
    assert_eq!(log["checkpoints"][0]["sha"], cp["sha"]);
}

#[tokio::test]
async fn command_execution_requires_allow_or_deny() {
    let e = env().await;
    let mut ev = e.events();
    let sid = e.session("fake").await;

    // --- Allow
    let turn = e.prompt(&sid, "run tests").await;
    let perm = ev.wait("permission.request").await;
    assert_eq!(perm["sessionId"], sid);
    assert_eq!(perm["turnId"], turn);
    assert_eq!(perm["tool"], "execute");
    assert_eq!(perm["command"], "npm test");
    // Nothing proceeds until the user answers.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        ev.rx.try_recv().is_err(),
        "turn advanced without a decision"
    );
    assert!(!ev.types().contains(&"chat.done"));

    let (s, _) = e.decide(perm["requestId"].as_str().unwrap(), "allow").await;
    assert_eq!(s, StatusCode::OK);
    ev.wait("chat.done").await;
    assert_eq!(ev.text(), "ran");
    // Answered once; a second answer is unknown.
    let (s, v) = e.decide(perm["requestId"].as_str().unwrap(), "allow").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(v["error"]["code"], "ENOENT");
    // The agent saw the allow_once option chosen.
    let answer = e.logged().into_iter().find(|m| m["id"] == "p2").unwrap();
    assert_eq!(
        answer["result"]["outcome"],
        json!({ "outcome": "selected", "optionId": "allow" })
    );

    // --- Deny
    let mut ev = e.events();
    e.prompt(&sid, "run again").await;
    let perm = ev.wait("permission.request").await;
    let (s, _) = e.decide(perm["requestId"].as_str().unwrap(), "deny").await;
    assert_eq!(s, StatusCode::OK);
    ev.wait("chat.done").await;
    assert_eq!(ev.text(), "denied");
    let failed = ev.seen.iter().rfind(|v| v["type"] == "chat.tool").unwrap();
    assert_eq!(failed["state"], "failed");
    let answers: Vec<_> = e.logged().into_iter().filter(|m| m["id"] == "p2").collect();
    assert_eq!(
        answers.last().unwrap()["result"]["outcome"]["optionId"],
        "reject"
    );
}

#[tokio::test]
async fn permission_endpoint_validates_input() {
    let e = env().await;
    let (s, v) = e.decide("perm-nope", "allow").await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("ENOENT"))
    );
    let (s, v) = e.decide("perm-nope", "maybe").await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("BADREQ"))
    );
}

#[tokio::test]
async fn auto_approve_setting_skips_prompts_and_persists() {
    let e = env().await;
    let uri = format!("/api/projects/{}", e.project);
    let (s, v) = e
        .call("PATCH", &uri, Some(json!({ "autoApprove": true })))
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["autoApprove"], true);
    let (_, list) = e.call("GET", "/api/projects", None).await;
    assert_eq!(list["projects"][0]["autoApprove"], true);

    let mut ev = e.events();
    let sid = e.session("fake").await;
    e.prompt(&sid, "run tests").await;
    ev.wait("chat.done").await;
    assert_eq!(ev.text(), "ran");
    assert!(!ev.types().contains(&"permission.request"));

    // Survives a registry reload (daemon restart).
    let reopened = workbench::projects::ProjectRegistry::open(e.root.clone(), false);
    assert!(reopened.get(&e.project).await.unwrap().auto_approve);

    // Toggle back off: prompts return. Also rename.
    let (s, v) = e
        .call(
            "PATCH",
            &uri,
            Some(json!({ "autoApprove": false, "name": "Renamed" })),
        )
        .await;
    assert_eq!(
        (s, v["autoApprove"].clone(), v["name"].clone()),
        (StatusCode::OK, json!(false), json!("Renamed"))
    );
    let mut ev = e.events();
    e.prompt(&sid, "run tests").await;
    let perm = ev.wait("permission.request").await;
    e.decide(perm["requestId"].as_str().unwrap(), "deny").await;
    ev.wait("chat.done").await;
}

#[tokio::test]
async fn patch_project_validates_input() {
    let e = env().await;
    let uri = format!("/api/projects/{}", e.project);
    for body in [
        json!({}),
        json!({ "autoApprove": "yes" }),
        json!({ "nope": 1 }),
        json!({ "name": "" }),
    ] {
        let (s, v) = e.call("PATCH", &uri, Some(body.clone())).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {v}");
        assert_eq!(v["error"]["code"], "BADREQ");
    }
    let (s, v) = e
        .call(
            "PATCH",
            "/api/projects/ghost",
            Some(json!({ "autoApprove": true })),
        )
        .await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("ENOENT"))
    );
}

#[tokio::test]
async fn cancel_stops_the_turn() {
    let e = env().await;
    let mut ev = e.events();
    let sid = e.session("fake").await;
    let turn = e.prompt(&sid, "slow please").await;
    ev.wait_for("chat.chunk", |v| v["delta"] == "working").await;

    // A second prompt while one is running is refused.
    let (s, v) = e.prompt_raw(&sid, "hello").await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::CONFLICT, Some("CONFLICT"))
    );

    let (s, _) = e
        .call("POST", &format!("/api/agent/prompt/{turn}/cancel"), None)
        .await;
    assert_eq!(s, StatusCode::OK);
    let done = ev.wait("chat.done").await;
    assert_eq!(done["stopReason"], "cancelled");
    assert!(e
        .logged()
        .iter()
        .any(|m| m["method"] == "session/cancel" && m["params"]["sessionId"] == sid));

    // Unknown turn ids 404; the session remains usable.
    let (s, _) = e
        .call("POST", "/api/agent/prompt/turn-nope/cancel", None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    e.prompt(&sid, "hello").await;
    ev.wait_for("chat.done", |v| v["stopReason"] == "end_turn")
        .await;
}

#[tokio::test]
async fn cancel_answers_pending_permission_as_cancelled() {
    let e = env().await;
    let mut ev = e.events();
    let sid = e.session("fake").await;
    let turn = e.prompt(&sid, "run tests").await;
    let perm = ev.wait("permission.request").await;

    let (s, _) = e
        .call("POST", &format!("/api/agent/prompt/{turn}/cancel"), None)
        .await;
    assert_eq!(s, StatusCode::OK);
    let done = ev.wait("chat.done").await;
    assert_eq!(done["stopReason"], "cancelled");
    let answer = e.logged().into_iter().find(|m| m["id"] == "p2").unwrap();
    assert_eq!(
        answer["result"]["outcome"],
        json!({ "outcome": "cancelled" })
    );
    // The stale prompt can no longer be answered.
    let (s, _) = e.decide(perm["requestId"].as_str().unwrap(), "allow").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn new_session_terminates_previous_subprocess() {
    let e = env().await;
    let first = e.session("fake").await;
    let pid_a = e.pid();
    assert!(alive(pid_a));

    let second = e.session("fake").await;
    assert_ne!(first, second);
    let pid_b = e.pid();
    assert_ne!(pid_a, pid_b);
    wait_gone(pid_a).await;
    assert!(alive(pid_b));

    // The replaced session id is rejected; the new one works.
    let (s, v) = e.prompt_raw(&first, "hello").await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::CONFLICT, Some("CONFLICT"))
    );
    let mut ev = e.events();
    e.prompt(&second, "hello").await;
    ev.wait("chat.done").await;
}

#[tokio::test]
async fn stubborn_agent_is_killed() {
    let e = env().await;
    e.session("stubborn").await;
    let pid = e.pid();
    let started = Instant::now();
    e.session("fake").await;
    wait_gone(pid).await;
    // It ignored stdin EOF, so escalation to signals was needed.
    assert!(started.elapsed() >= Duration::from_millis(1000));
}

#[tokio::test]
async fn deleting_a_project_stops_its_agent_and_history() {
    let e = env().await;
    let mut ev = e.events();
    let sid = e.session("fake").await;
    e.prompt(&sid, "hello").await;
    ev.wait("chat.done").await;
    wait_turns(&e, 1).await;
    let pid = e.pid();
    assert!(acp::history_path(&e.root, &e.project).is_file());

    let (s, _) = e
        .call(
            "DELETE",
            &format!("/api/projects/{}?confirm=true", e.project),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    wait_gone(pid).await;
    assert!(!acp::history_path(&e.root, &e.project).exists());
    let (s, _) = e.prompt_raw(&sid, "hello").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn agent_crash_surfaces_error_and_marks_session_dead() {
    let e = env().await;
    let mut ev = e.events();
    let sid = e.session("fake").await;
    let turn = e.prompt(&sid, "crash now").await;
    let err = ev.wait("chat.error").await;
    assert_eq!(err["turnId"], turn);
    assert_eq!(err["sessionId"], sid);
    assert!(err["message"].as_str().unwrap().contains("agent"));

    // The failed turn is in history with its error.
    wait_turns(&e, 1).await;
    let (_, h) = e
        .call(
            "GET",
            &format!("/api/agent/history?project={}", e.project),
            None,
        )
        .await;
    assert_eq!(h["turns"][0]["assistantText"], "about to die");
    assert!(h["turns"][0]["error"].is_string());

    // Session is dead: AGENT_ERR until a new one is started.
    let deadline = Instant::now() + WAIT;
    loop {
        let (s, v) = e.prompt_raw(&sid, "hello").await;
        if s == StatusCode::BAD_GATEWAY {
            assert_eq!(v["error"]["code"], "AGENT_ERR");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "session never marked dead: {s} {v}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let fresh = e.session("fake").await;
    e.prompt(&fresh, "hello").await;
}

#[tokio::test]
async fn checkpoint_waits_for_running_tools_to_settle() {
    let e = env().await;
    let mut ev = e.events();
    let sid = e.session("fake").await;
    e.prompt(&sid, "late edit").await;

    let cp = ev.wait("checkpoint.created").await;
    let types = ev.types();
    let pos = |pred: &dyn Fn(&Value) -> bool| ev.seen.iter().position(pred).unwrap();
    let done = pos(&|v| v["type"] == "chat.done");
    let tool_done = pos(&|v| v["type"] == "chat.tool" && v["state"] == "done");
    let created = pos(&|v| v["type"] == "checkpoint.created");
    // chat.done comes as soon as the prompt returns; the checkpoint only after the tool
    // finished and the quiet period elapsed — so it includes the late write.
    assert!(done < tool_done && tool_done < created, "{types:?}");
    assert!(git(
        &e.dir,
        &["show", "--stat", "--format=", cp["sha"].as_str().unwrap()]
    )
    .contains("late.txt"));
}

#[tokio::test]
async fn history_includes_late_updates_after_prompt_returns() {
    let e = env().await;
    let mut ev = e.events();
    let sid = e.session("fake").await;
    e.prompt(&sid, "late edit").await;
    ev.wait("checkpoint.created").await;

    // The prompt returned while the tool was running; the persisted record has the final
    // tool state, not the one at prompt-return time.
    let turns = wait_turns(&e, 1).await;
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].tools.len(), 1);
    assert_eq!(turns[0].tools[0].state, "done");
    assert_eq!(turns[0].error, None);
}

#[tokio::test]
async fn stuck_tool_fails_the_turn_without_checkpoint() {
    let e = env().await;
    e.state.agents.set_max_settle(Duration::from_millis(800));
    let mut ev = e.events();
    let sid = e.session("fake").await;
    let turn = e.prompt(&sid, "stuck").await;

    let err = ev.wait("chat.error").await;
    assert_eq!(err["turnId"], turn);
    assert!(
        err["message"].as_str().unwrap().contains("running tools"),
        "{err}"
    );
    // The failed turn is persisted with its error and the tool still running...
    let turns = wait_turns(&e, 1).await;
    assert_eq!(turns[0].tools[0].state, "running");
    assert!(turns[0].error.is_some());
    // ...but the dirty tree was never committed.
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!ev.types().contains(&"checkpoint.created"));
    while let Ok(t) = ev.rx.try_recv() {
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_ne!(v["type"], "checkpoint.created");
    }
    assert_eq!(git(&e.dir, &["rev-list", "--count", "HEAD"]), "1");
    assert!(git(&e.dir, &["status", "--porcelain"]).contains("stuck.txt"));
}

#[tokio::test]
async fn terminated_session_neither_persists_nor_checkpoints() {
    let e = env().await;
    // Long quiet period: the turn is guaranteed to still be settling when we terminate.
    e.state.agents.set_settle_quiet(Duration::from_secs(2));
    let mut ev = e.events();
    let sid = e.session("fake").await;
    e.prompt(&sid, "late edit").await;
    ev.wait("chat.done").await;

    // Replacing the session must quiesce the old turn before returning.
    let second = e.session("fake").await;
    assert_ne!(sid, second);
    assert!(e
        .state
        .agents
        .history(&e.project, None)
        .await
        .unwrap()
        .is_empty());

    // Even after the old quiet period would have elapsed, nothing shows up.
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(e
        .state
        .agents
        .history(&e.project, None)
        .await
        .unwrap()
        .is_empty());
    while let Ok(t) = ev.rx.try_recv() {
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_ne!(v["type"], "checkpoint.created", "{v}");
    }
    assert_eq!(git(&e.dir, &["rev-list", "--count", "HEAD"]), "1");
}

#[tokio::test]
async fn deleting_a_project_mid_settle_leaves_no_history() {
    let e = env().await;
    e.state.agents.set_settle_quiet(Duration::from_secs(2));
    let mut ev = e.events();
    let sid = e.session("fake").await;
    e.prompt(&sid, "late edit").await;
    ev.wait("chat.done").await;

    let (s, _) = e
        .call(
            "DELETE",
            &format!("/api/projects/{}?confirm=true", e.project),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(!acp::history_path(&e.root, &e.project).exists());
}

#[tokio::test]
async fn clean_turns_create_no_checkpoint() {
    let e = env().await;
    let mut ev = e.events();
    let sid = e.session("fake").await;
    e.prompt(&sid, "hello").await;
    ev.wait("chat.done").await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    while let Ok(t) = ev.rx.try_recv() {
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_ne!(v["type"], "checkpoint.created");
    }
    assert_eq!(git(&e.dir, &["rev-list", "--count", "HEAD"]), "1");
}

#[tokio::test]
async fn history_filters_by_session() {
    let e = env().await;
    let mut ev = e.events();
    let a = e.session("fake").await;
    e.prompt(&a, "hello a").await;
    ev.wait("chat.done").await;
    // A new session before the turn settles would (by design) drop the unsaved turn.
    wait_turns(&e, 1).await;
    let b = e.session("fake").await;
    let mut ev = e.events();
    e.prompt(&b, "hello b").await;
    ev.wait("chat.done").await;
    wait_turns(&e, 2).await;

    let uri = |extra: &str| format!("/api/agent/history?project={}{extra}", e.project);
    let (_, all) = e.call("GET", &uri(""), None).await;
    let texts: Vec<_> = all["turns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["userText"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["hello a", "hello b"]);
    let (_, only_b) = e.call("GET", &uri(&format!("&session={b}")), None).await;
    assert_eq!(only_b["turns"].as_array().unwrap().len(), 1);
    assert_eq!(only_b["turns"][0]["userText"], "hello b");

    // Survives a daemon restart: a fresh state over the same directory sees it.
    let cfg = config(&e.root, &e.root.join("agents.json"), None);
    let fresh = AppState::with_scaffold_install(cfg, false);
    let turns = fresh.agents.history(&e.project, None).await.unwrap();
    assert_eq!(turns.len(), 2);

    let (s, _) = e
        .call("GET", "/api/agent/history?project=ghost", None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = e.call("GET", "/api/agent/history", None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn session_and_prompt_errors_use_the_envelope() {
    let e = env().await;
    let start = |project: &str, provider: &str| {
        e.call(
            "POST",
            "/api/agent/session",
            Some(json!({ "project": project, "provider": provider })),
        )
    };
    let (s, v) = start("ghost", "fake").await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("ENOENT"))
    );
    let (s, v) = start(&e.project, "nope").await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("BADREQ"))
    );
    let (s, v) = start(&e.project, "missing").await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::BAD_GATEWAY, Some("AGENT_ERR"))
    );
    let (s, v) = e
        .call(
            "POST",
            "/api/agent/session",
            Some(json!({ "project": "x" })),
        )
        .await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("BADREQ"))
    );

    let (s, v) = e.prompt_raw("whatever", "hi").await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("ENOENT"))
    );
    let sid = e.session("fake").await;
    let (s, v) = e.prompt_raw(&sid, "   ").await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("BADREQ"))
    );
}

#[tokio::test]
async fn ws_events_stream_with_token_and_origin_checks() {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::Message;

    let e = env_with(Some("abc123")).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(workbench::serve_on(listener, e.state.clone()));

    // No token → refused.
    assert!(connect_async(format!("ws://{addr}/ws/events"))
        .await
        .is_err());
    // Foreign browser origin → refused even with a valid token.
    let mut evil = format!("ws://{addr}/ws/events?t=abc123")
        .into_client_request()
        .unwrap();
    evil.headers_mut()
        .insert("origin", "https://evil.example".parse().unwrap());
    assert!(connect_async(evil).await.is_err());

    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/events?t=abc123"))
        .await
        .unwrap();

    // Drive a turn over REST (Bearer auth) and read the events off the socket.
    let client = |method: &str, path: &str, body: Value| {
        Request::builder()
            .method(method)
            .uri(format!("http://{addr}{path}"))
            .header(header::HOST, "localhost")
            .header(header::AUTHORIZATION, "Bearer abc123")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let call = |r: Request<Body>| {
        let app = http::router(e.state.clone());
        async move {
            let res = app.oneshot(r).await.unwrap();
            serde_json::from_slice::<Value>(&res.into_body().collect().await.unwrap().to_bytes())
                .unwrap()
        }
    };
    let v = call(client(
        "POST",
        "/api/agent/session",
        json!({ "project": e.project, "provider": "fake" }),
    ))
    .await;
    let sid = v["sessionId"].as_str().unwrap().to_string();
    call(client(
        "POST",
        "/api/agent/prompt",
        json!({ "project": e.project, "sessionId": sid, "text": "hello" }),
    ))
    .await;

    let mut types = Vec::new();
    let deadline = Instant::now() + WAIT;
    while !types.contains(&"chat.done".to_string()) {
        let left = deadline.saturating_duration_since(Instant::now());
        let msg = tokio::time::timeout(left, ws.next())
            .await
            .expect("ws timeout")
            .unwrap()
            .unwrap();
        if let Message::Text(t) = msg {
            let v: Value = serde_json::from_str(&t).unwrap();
            assert_eq!(v["sessionId"], sid);
            types.push(v["type"].as_str().unwrap().to_string());
        }
    }
    assert_eq!(types, ["chat.chunk", "chat.chunk", "chat.done"]);
    server.abort();
}
