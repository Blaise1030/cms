//! ACP (Agent Client Protocol) client: spawns a coding-agent adapter per project and
//! drives it over newline-delimited JSON-RPC on stdio.
//!
//! Lifecycle per project (at most one live session):
//! `initialize` → `session/new` (cwd = project dir) → `session/prompt` (one per turn).
//! While a prompt is in flight the agent streams `session/update` notifications, which
//! are forwarded as `chat.chunk` / `chat.tool` / `chat.plan` events, and may issue
//! `session/request_permission` requests, which are answered by policy or by the user.
//!
//! Permission policy (`session/request_permission`):
//! * the project's `auto_approve` toggle allows everything;
//! * file operations (`read`, `edit`, `delete`, `move`, `search`, `think`) are always allowed;
//! * everything else (`execute`, `fetch`, `other`, unknown) surfaces a `permission.request`
//!   event and waits for `Allow`/`Deny` via `POST /api/agent/permission/{requestId}`.
//!
//! A turn *settles* once the prompt has returned, no tool call is still running, and the
//! agent has been quiet for the settle period (2 s). Only a true settle triggers a git
//! checkpoint; if the agent dies or a tool is still running after the settle cap (30 s) the
//! turn fails with `chat.error` and nothing is committed. Turns are appended to
//! `{projects-dir}/.workbench/{id}/history.jsonl` *after* settling so late `session/update`s
//! are included. Terminating a session waits for its in-flight turn task to finish, and a
//! terminated turn neither persists nor checkpoints.
//!
//! The protocol subset we need is tiny, so this is a hand-rolled JSON-RPC client rather
//! than an SDK dependency (keeps cold start and binary size down, and the wire format is
//! pinned by the fake-agent tests).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex, OwnedMutexGuard};

use crate::agents_config::{AgentsConfig, Provider};
use crate::config::Config;
use crate::error::ApiError;
use crate::events::EventBus;
use crate::git;
use crate::projects::{ProjectRegistry, STATE_DIR};

/// ACP protocol version we speak.
const PROTOCOL_VERSION: u32 = 1;
/// Budget for `initialize` and `session/new` (first `npx` run may download the adapter).
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(120);
/// Quiet period after a turn before it is checkpointed.
pub const DEFAULT_SETTLE_QUIET: Duration = Duration::from_secs(2);
/// A tool stuck in `running` for this long fails the turn (no checkpoint is taken).
pub const DEFAULT_MAX_SETTLE: Duration = Duration::from_secs(30);
/// Graceful-exit budgets when terminating an agent subprocess.
const EXIT_GRACE: Duration = Duration::from_millis(1500);
const TERM_GRACE: Duration = Duration::from_secs(2);
/// Bytes of agent stderr kept for error messages.
const STDERR_TAIL: usize = 4096;
const HISTORY_FILE: &str = "history.jsonl";

fn lock<T>(m: &StdMutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn random_id(prefix: &str) -> String {
    let mut bytes = [0u8; 8];
    rand::rng().fill_bytes(&mut bytes);
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{prefix}{hex}")
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

// ---------------------------------------------------------------------------
// History (JSONL, outside the site repo)
// ---------------------------------------------------------------------------

/// `{ tool, summary, state }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSummary {
    pub tool: String,
    pub summary: String,
    pub state: String,
}

/// One completed turn as persisted / returned by `GET /api/agent/history`.
/// `ts` is unix seconds (same unit as checkpoints).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnRecord {
    pub id: String,
    pub session_id: String,
    pub provider: String,
    pub ts: i64,
    pub user_text: String,
    pub assistant_text: String,
    pub tools: Vec<ToolSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `{projects-dir}/.workbench/{id}/history.jsonl`.
pub fn history_path(projects_dir: &Path, project_id: &str) -> PathBuf {
    projects_dir
        .join(STATE_DIR)
        .join(project_id)
        .join(HISTORY_FILE)
}

async fn append_history(path: &Path, record: &TurnRecord) -> Result<(), ApiError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut line = serde_json::to_string(record)
        .map_err(|e| ApiError::io(format!("serialize history: {e}")))?;
    line.push('\n');
    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    f.write_all(line.as_bytes()).await?;
    f.flush().await?;
    Ok(())
}

/// Turns in file order, optionally only those of `session`. A missing file is empty history;
/// unparseable lines (e.g. a torn final write) are skipped.
pub async fn read_history(path: &Path, session: Option<&str>) -> Result<Vec<TurnRecord>, ApiError> {
    let text = match tokio::fs::read_to_string(path).await {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    Ok(text
        .lines()
        .filter_map(|l| serde_json::from_str::<TurnRecord>(l).ok())
        .filter(|t| session.is_none_or(|s| t.session_id == s))
        .collect())
}

// ---------------------------------------------------------------------------
// Turn state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolState {
    Running,
    Done,
    Failed,
}

impl ToolState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    fn from_status(status: &str) -> Option<Self> {
        match status {
            "pending" | "in_progress" => Some(Self::Running),
            "completed" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

struct ToolRec {
    call_id: String,
    kind: String,
    title: String,
    state: ToolState,
}

impl ToolRec {
    fn label(&self) -> &str {
        if self.title.is_empty() {
            &self.kind
        } else {
            &self.title
        }
    }
}

struct TurnState {
    id: String,
    user_text: String,
    assistant: String,
    tools: Vec<ToolRec>,
    last_activity: Instant,
    /// `session/prompt` has returned (the turn may still be settling).
    responded: bool,
}

impl TurnState {
    fn running_tools(&self) -> usize {
        self.tools
            .iter()
            .filter(|t| t.state == ToolState::Running)
            .count()
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC connection
// ---------------------------------------------------------------------------

enum Out {
    Line(String),
    /// Close the agent's stdin (polite shutdown request).
    Close,
}

#[derive(Default)]
struct Pending {
    closed: bool,
    map: HashMap<u64, oneshot::Sender<Result<Value, String>>>,
}

struct Conn {
    tx: mpsc::UnboundedSender<Out>,
    pending: StdMutex<Pending>,
    next_id: AtomicU64,
}

impl Conn {
    fn send(&self, msg: Value) {
        let _ = self.tx.send(Out::Line(msg.to_string()));
    }

    fn notify(&self, method: &str, params: Value) {
        self.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn respond(&self, id: Value, result: Result<Value, (i64, String)>) {
        let msg = match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
            }
        };
        self.send(msg);
    }

    async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        {
            let mut p = lock(&self.pending);
            if p.closed {
                return Err("agent connection is closed".into());
            }
            p.map.insert(id, tx);
        }
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let reply = match timeout {
            Some(t) => match tokio::time::timeout(t, rx).await {
                Ok(r) => r,
                Err(_) => {
                    lock(&self.pending).map.remove(&id);
                    return Err(format!("{method} timed out after {}s", t.as_secs()));
                }
            },
            None => rx.await,
        };
        match reply {
            Ok(r) => r,
            Err(_) => Err("agent connection closed".into()),
        }
    }

    fn resolve(&self, id: u64, result: Result<Value, String>) {
        if let Some(tx) = lock(&self.pending).map.remove(&id) {
            let _ = tx.send(result);
        }
    }

    /// Refuse new requests and fail everything in flight.
    fn close(&self) {
        let mut p = lock(&self.pending);
        p.closed = true;
        p.map.clear();
    }
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Deny,
}

enum Verdict {
    Decided(Decision),
    Cancelled,
}

struct PendingPermission {
    session_id: String,
    tx: oneshot::Sender<Verdict>,
}

/// State shared by the manager and every session.
struct Shared {
    config: Arc<Config>,
    projects: Arc<ProjectRegistry>,
    events: EventBus,
    permissions: StdMutex<HashMap<String, PendingPermission>>,
    settle_quiet_ms: AtomicU64,
    max_settle_ms: AtomicU64,
}

impl Shared {
    fn max_settle(&self) -> Duration {
        Duration::from_millis(self.max_settle_ms.load(Ordering::Relaxed))
    }

    fn settle_quiet(&self) -> Duration {
        Duration::from_millis(self.settle_quiet_ms.load(Ordering::Relaxed))
    }

    /// Resolve (as cancelled) every open permission prompt of `session_id`.
    fn cancel_permissions(&self, session_id: &str) {
        let mut map = lock(&self.permissions);
        let ids: Vec<String> = map
            .iter()
            .filter(|(_, p)| p.session_id == session_id)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if let Some(p) = map.remove(&id) {
                let _ = p.tx.send(Verdict::Cancelled);
            }
        }
    }
}

struct Session {
    id: OnceLock<String>,
    project_id: String,
    provider: String,
    dir: PathBuf,
    shared: Arc<Shared>,
    conn: Conn,
    turn: StdMutex<Option<TurnState>>,
    /// Held by the `run_turn` task for its whole life (prompt, settle, persist, checkpoint),
    /// so turns never overlap and [`Session::terminate`] can wait for the task to finish.
    turn_lock: Arc<Mutex<()>>,
    /// A prompt has been accepted and its `session/prompt` has not returned yet.
    awaiting: AtomicBool,
    dead: AtomicBool,
    /// We are shutting the process down on purpose.
    terminating: AtomicBool,
    /// `on_exit` already told the UI the process died (avoids a duplicate `chat.error`).
    exit_reported: AtomicBool,
    child: Mutex<Option<Child>>,
    pid: Option<u32>,
    stderr_tail: StdMutex<String>,
}

/// Clears [`Session::awaiting`] unless disarmed (protects against a dropped HTTP request).
struct AwaitingGuard(Arc<Session>, bool);

impl Drop for AwaitingGuard {
    fn drop(&mut self) {
        if self.1 {
            self.0.awaiting.store(false, Ordering::SeqCst);
        }
    }
}

impl Session {
    fn id(&self) -> &str {
        self.id.get().map_or("", String::as_str)
    }

    fn is_dead(&self) -> bool {
        self.dead.load(Ordering::SeqCst)
    }

    fn emit(&self, kind: &str, mut fields: Value) {
        if let Value::Object(m) = &mut fields {
            m.insert("projectId".into(), json!(self.project_id));
            m.insert("sessionId".into(), json!(self.id()));
        }
        self.shared.events.emit(kind, fields);
    }

    fn describe_failure(&self, msg: &str) -> String {
        let tail = lock(&self.stderr_tail).trim().to_string();
        if tail.is_empty() {
            msg.to_string()
        } else {
            format!("{msg} (agent stderr: {tail})")
        }
    }

    /// Spawn the adapter and run the `initialize` + `session/new` handshake.
    async fn start(
        shared: Arc<Shared>,
        project_id: &str,
        dir: PathBuf,
        provider: &Provider,
    ) -> Result<Arc<Session>, ApiError> {
        let (program, args) = provider
            .command
            .split_first()
            .ok_or_else(|| ApiError::agent(format!("provider {} has no command", provider.id)))?;
        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            // Own process group so terminating also reaches `npx`'s node grandchild.
            .process_group(0);
        let mut child = cmd.spawn().map_err(|e| {
            ApiError::agent(format!(
                "failed to start provider {} ({}): {e}",
                provider.id,
                provider.command.join(" ")
            ))
        })?;
        let pid = child.id();
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");

        let (tx, rx) = mpsc::unbounded_channel();
        let session = Arc::new(Session {
            id: OnceLock::new(),
            project_id: project_id.to_string(),
            provider: provider.id.clone(),
            dir: dir.clone(),
            shared,
            conn: Conn {
                tx,
                pending: StdMutex::new(Pending::default()),
                next_id: AtomicU64::new(1),
            },
            turn: StdMutex::new(None),
            turn_lock: Arc::new(Mutex::new(())),
            awaiting: AtomicBool::new(false),
            dead: AtomicBool::new(false),
            terminating: AtomicBool::new(false),
            exit_reported: AtomicBool::new(false),
            child: Mutex::new(Some(child)),
            pid,
            stderr_tail: StdMutex::new(String::new()),
        });

        tokio::spawn(write_loop(rx, stdin));
        tokio::spawn(read_loop(session.clone(), stdout));
        tokio::spawn(stderr_loop(session.clone(), stderr));

        match session.handshake().await {
            Ok(id) => {
                let _ = session.id.set(id);
                Ok(session)
            }
            Err(msg) => {
                let msg = session.describe_failure(&msg);
                session.terminate().await;
                Err(ApiError::agent(format!(
                    "provider {} failed to start a session: {msg}",
                    provider.id
                )))
            }
        }
    }

    async fn handshake(&self) -> Result<String, String> {
        self.conn
            .request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    // We implement no fs/terminal client methods: agents use their own tools
                    // inside the project dir.
                    "clientCapabilities": {
                        "fs": { "readTextFile": false, "writeTextFile": false },
                        "terminal": false
                    },
                    "clientInfo": {
                        "name": "workbench",
                        "title": "Workbench",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
                Some(HANDSHAKE_TIMEOUT),
            )
            .await
            .map_err(|e| format!("initialize: {e}"))?;
        let cwd = self.dir.to_string_lossy().into_owned();
        let res = self
            .conn
            .request(
                "session/new",
                json!({ "cwd": cwd, "mcpServers": [] }),
                Some(HANDSHAKE_TIMEOUT),
            )
            .await
            .map_err(|e| format!("session/new: {e}"))?;
        res["sessionId"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "session/new: response has no sessionId".to_string())
    }

    /// Stop the subprocess (close stdin, then SIGTERM the process group, then SIGKILL) and
    /// wait until the session's turn task, if any, has finished.
    async fn terminate(&self) {
        self.terminating.store(true, Ordering::SeqCst);
        self.dead.store(true, Ordering::SeqCst);
        let _ = self.conn.tx.send(Out::Close);
        self.shared.cancel_permissions(self.id());
        let child = self.child.lock().await.take();
        if let Some(mut child) = child {
            if tokio::time::timeout(EXIT_GRACE, child.wait())
                .await
                .is_err()
            {
                self.signal_group(libc::SIGTERM);
                if tokio::time::timeout(TERM_GRACE, child.wait())
                    .await
                    .is_err()
                {
                    self.signal_group(libc::SIGKILL);
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
            }
        }
        self.conn.close();
        // Lifecycle barrier: wait for any in-flight `run_turn` to finish. It sees
        // `terminating` and skips history/checkpoint, and `prompt` refuses to start new
        // turns once `dead` is set, so nothing can run for this session after we return.
        let _quiesced = self.turn_lock.lock().await;
    }

    /// Signal the adapter's whole process group. Only called while the child is unreaped,
    /// so its pid (== pgid) cannot have been recycled.
    fn signal_group(&self, sig: i32) {
        if let Some(pid) = self.pid {
            // SAFETY: plain syscall; pid is our unreaped child, which leads its own group.
            unsafe {
                libc::killpg(pid as i32, sig);
            }
        }
    }

    // ---- incoming messages ------------------------------------------------

    fn handle_message(self: &Arc<Self>, msg: Value) {
        let method = msg.get("method").and_then(Value::as_str);
        let id = msg.get("id").filter(|v| !v.is_null()).cloned();
        match (method, id) {
            (Some(method), Some(id)) => {
                let this = self.clone();
                let method = method.to_string();
                let params = msg.get("params").cloned().unwrap_or(Value::Null);
                tokio::spawn(async move { this.handle_request(id, &method, params).await });
            }
            (Some("session/update"), None) => {
                self.on_update(msg.get("params").unwrap_or(&Value::Null));
            }
            (Some(_), None) => {}
            (None, Some(id)) => {
                let Some(id) = id.as_u64() else { return };
                let result = match msg.get("error") {
                    Some(err) => Err(err
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("agent error")
                        .to_string()),
                    None => Ok(msg.get("result").cloned().unwrap_or(Value::Null)),
                };
                self.conn.resolve(id, result);
            }
            (None, None) => {}
        }
    }

    /// Agent → client request. Runs in its own task so a pending permission prompt never
    /// blocks the read loop.
    async fn handle_request(&self, id: Value, method: &str, params: Value) {
        match method {
            "session/request_permission" => {
                let outcome = self.decide_permission(&params).await;
                self.conn.respond(id, Ok(json!({ "outcome": outcome })));
            }
            _ => self.conn.respond(
                id,
                Err((
                    -32601,
                    format!("method not supported by workbench: {method}"),
                )),
            ),
        }
    }

    async fn decide_permission(&self, params: &Value) -> Value {
        let tool_call = &params["toolCall"];
        let kind = tool_call["kind"].as_str().unwrap_or("other");
        let options = params["options"].as_array().cloned().unwrap_or_default();

        let auto = self
            .shared
            .projects
            .get(&self.project_id)
            .await
            .is_some_and(|p| p.auto_approve);
        let verdict = if auto || is_auto_allowed(kind) {
            Verdict::Decided(Decision::Allow)
        } else {
            self.ask_user(tool_call, kind).await
        };
        self.touch();
        outcome_for(&verdict, &options)
    }

    /// Surface a `permission.request` and wait for Allow/Deny (or cancellation).
    async fn ask_user(&self, tool_call: &Value, kind: &str) -> Verdict {
        let request_id = random_id("perm-");
        let (tx, rx) = oneshot::channel();
        lock(&self.shared.permissions).insert(
            request_id.clone(),
            PendingPermission {
                session_id: self.id().to_string(),
                tx,
            },
        );
        let turn_id = lock(&self.turn).as_ref().map(|t| t.id.clone());
        let title = tool_call["title"].as_str().unwrap_or("");
        let mut ev = json!({
            "requestId": request_id,
            "turnId": turn_id,
            "tool": kind,
            "summary": title,
        });
        if let Some(cmd) = command_of(tool_call, kind) {
            ev["command"] = json!(cmd);
        }
        self.emit("permission.request", ev);
        rx.await.unwrap_or(Verdict::Cancelled)
    }

    fn touch(&self) {
        if let Some(t) = lock(&self.turn).as_mut() {
            t.last_activity = Instant::now();
        }
    }

    /// `session/update` notification → events + turn accumulation.
    fn on_update(&self, params: &Value) {
        let update = &params["update"];
        let Some(kind) = update["sessionUpdate"].as_str() else {
            return;
        };
        let event = {
            let mut guard = lock(&self.turn);
            let Some(turn) = guard.as_mut() else { return };
            turn.last_activity = Instant::now();
            match kind {
                "agent_message_chunk" => {
                    let content = &update["content"];
                    match (content["type"].as_str(), content["text"].as_str()) {
                        (Some("text"), Some(text)) if !text.is_empty() => {
                            turn.assistant.push_str(text);
                            Some(("chat.chunk", json!({ "turnId": turn.id, "delta": text })))
                        }
                        _ => None,
                    }
                }
                "tool_call" | "tool_call_update" => {
                    let tool = upsert_tool(turn, update, kind == "tool_call");
                    tool.map(|t| {
                        (
                            "chat.tool",
                            json!({
                                "turnId": turn.id,
                                "toolCallId": t.0,
                                "tool": t.1,
                                "state": t.2.as_str(),
                                "summary": t.3,
                            }),
                        )
                    })
                }
                "plan" => Some((
                    "chat.plan",
                    json!({ "turnId": turn.id, "entries": update["entries"] }),
                )),
                _ => None,
            }
        };
        if let Some((kind, fields)) = event {
            self.emit(kind, fields);
        }
    }

    /// Reader hit EOF: the process is gone (or was stopped on purpose).
    fn on_exit(&self) {
        self.dead.store(true, Ordering::SeqCst);
        self.conn.close();
        self.shared.cancel_permissions(self.id());
        if self.id.get().is_some()
            && !self.terminating.load(Ordering::SeqCst)
            && !self.awaiting.load(Ordering::SeqCst)
        {
            // Nobody is waiting on a prompt, so tell the UI the session died.
            self.exit_reported.store(true, Ordering::SeqCst);
            let turn_id = lock(&self.turn).as_ref().map(|t| t.id.clone());
            self.emit(
                "chat.error",
                json!({
                    "turnId": turn_id,
                    "message": self.describe_failure("agent process exited"),
                }),
            );
        }
    }
}

/// Apply a `tool_call` / `tool_call_update` to the turn. Returns
/// `(toolCallId, kind, state, summary)` for the event.
fn upsert_tool(
    turn: &mut TurnState,
    update: &Value,
    is_new: bool,
) -> Option<(String, String, ToolState, String)> {
    let call_id = update["toolCallId"].as_str()?.to_string();
    let idx = match turn.tools.iter().position(|t| t.call_id == call_id) {
        Some(i) => i,
        None => {
            turn.tools.push(ToolRec {
                call_id,
                kind: "other".into(),
                title: String::new(),
                state: ToolState::Running,
            });
            turn.tools.len() - 1
        }
    };
    let t = &mut turn.tools[idx];
    if let Some(k) = update["kind"].as_str() {
        t.kind = k.to_string();
    }
    if let Some(title) = update["title"].as_str() {
        t.title = title.to_string();
    }
    match update["status"].as_str().and_then(ToolState::from_status) {
        Some(s) => t.state = s,
        None if is_new => t.state = ToolState::Running,
        None => {}
    }
    Some((
        t.call_id.clone(),
        t.kind.clone(),
        t.state,
        t.label().to_string(),
    ))
}

/// File operations never prompt; anything that could run code or reach the network does.
fn is_auto_allowed(kind: &str) -> bool {
    matches!(
        kind,
        "read" | "edit" | "delete" | "move" | "search" | "think"
    )
}

/// The shell command of an `execute` tool call, if the agent told us.
fn command_of(tool_call: &Value, kind: &str) -> Option<String> {
    let raw = &tool_call["rawInput"];
    match &raw["command"] {
        Value::String(s) if !s.is_empty() => return Some(s.clone()),
        Value::Array(a) => {
            let parts: Vec<&str> = a.iter().filter_map(Value::as_str).collect();
            if !parts.is_empty() {
                return Some(parts.join(" "));
            }
        }
        _ => {}
    }
    (kind == "execute")
        .then(|| tool_call["title"].as_str())
        .flatten()
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// Map our verdict onto one of the agent-offered options.
fn outcome_for(verdict: &Verdict, options: &[Value]) -> Value {
    let pick = |kinds: &[&str]| {
        kinds.iter().find_map(|k| {
            options
                .iter()
                .find(|o| o["kind"].as_str() == Some(k))
                .and_then(|o| o["optionId"].as_str())
        })
    };
    let chosen = match verdict {
        Verdict::Decided(Decision::Allow) => pick(&["allow_once", "allow_always"]),
        Verdict::Decided(Decision::Deny) => pick(&["reject_once", "reject_always"]),
        Verdict::Cancelled => None,
    };
    match chosen {
        Some(id) => json!({ "outcome": "selected", "optionId": id }),
        None => json!({ "outcome": "cancelled" }),
    }
}

async fn write_loop(mut rx: mpsc::UnboundedReceiver<Out>, mut stdin: tokio::process::ChildStdin) {
    while let Some(out) = rx.recv().await {
        match out {
            Out::Line(mut line) => {
                line.push('\n');
                if stdin.write_all(line.as_bytes()).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
            Out::Close => break,
        }
    }
    // Dropping stdin closes it.
}

async fn read_loop(session: Arc<Session>, stdout: tokio::process::ChildStdout) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        // Adapters may print stray non-JSON lines; ignore them.
        if let Ok(msg) = serde_json::from_str::<Value>(&line) {
            session.handle_message(msg);
        }
    }
    session.on_exit();
}

async fn stderr_loop(session: Arc<Session>, mut stderr: tokio::process::ChildStderr) {
    let mut buf = [0u8; 1024];
    while let Ok(n) = stderr.read(&mut buf).await {
        if n == 0 {
            break;
        }
        let mut tail = lock(&session.stderr_tail);
        tail.push_str(&String::from_utf8_lossy(&buf[..n]));
        if tail.len() > STDERR_TAIL {
            let mut cut = tail.len() - STDERR_TAIL;
            while !tail.is_char_boundary(cut) {
                cut += 1;
            }
            tail.drain(..cut);
        }
    }
}

// ---------------------------------------------------------------------------
// Turns
// ---------------------------------------------------------------------------

/// Why waiting for a turn to settle ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Settle {
    /// No running tool and the agent has been quiet: safe to persist and checkpoint.
    Settled,
    /// The session died or is being terminated.
    Died,
    /// A tool was still running (or the agent kept streaming) when the cap elapsed.
    TimedOut,
}

/// Drive one turn to completion: prompt → `chat.done` → settle → history → checkpoint.
///
/// A terminated session emits nothing further and neither persists nor checkpoints.
async fn run_turn(
    session: Arc<Session>,
    turn_id: String,
    text: String,
    _turn_guard: OwnedMutexGuard<()>,
) {
    let result = session
        .conn
        .request(
            "session/prompt",
            json!({
                "sessionId": session.id(),
                "prompt": [{ "type": "text", "text": text }]
            }),
            None,
        )
        .await;
    session.awaiting.store(false, Ordering::SeqCst);
    if session.terminating.load(Ordering::SeqCst) {
        *lock(&session.turn) = None;
        return;
    }

    let (stop_reason, mut error) = match result {
        Ok(v) => (
            Some(v["stopReason"].as_str().unwrap_or("end_turn").to_string()),
            None,
        ),
        Err(e) => (None, Some(session.describe_failure(&e))),
    };
    if let Some(t) = lock(&session.turn).as_mut() {
        t.responded = true;
        t.last_activity = Instant::now();
    }
    match (&stop_reason, &error) {
        (_, Some(msg)) => session.emit("chat.error", json!({ "turnId": turn_id, "message": msg })),
        (Some(reason), None) => session.emit(
            "chat.done",
            json!({ "turnId": turn_id, "stopReason": reason }),
        ),
        (None, None) => {}
    }

    let outcome = settle(&session).await;
    if session.terminating.load(Ordering::SeqCst) {
        *lock(&session.turn) = None;
        return;
    }
    if outcome != Settle::Settled && error.is_none() {
        let msg = match outcome {
            Settle::TimedOut => format!(
                "agent still had running tools after {}s; changes were not checkpointed",
                session.shared.max_settle().as_secs_f64().round()
            ),
            _ => session.describe_failure("agent process exited before the turn settled"),
        };
        if outcome == Settle::TimedOut || !session.exit_reported.load(Ordering::SeqCst) {
            session.emit("chat.error", json!({ "turnId": turn_id, "message": msg }));
        }
        error = Some(msg);
    }

    // Snapshot after settling so late assistant chunks / tool transitions are included.
    let record = lock(&session.turn).as_ref().map(|t| TurnRecord {
        id: t.id.clone(),
        session_id: session.id().to_string(),
        provider: session.provider.clone(),
        ts: now_secs(),
        user_text: t.user_text.clone(),
        assistant_text: t.assistant.clone(),
        tools: t
            .tools
            .iter()
            .map(|x| ToolSummary {
                tool: x.kind.clone(),
                summary: x.label().to_string(),
                state: x.state.as_str().to_string(),
            })
            .collect(),
        stop_reason: stop_reason.clone(),
        error: error.clone(),
    });
    if let Some(record) = record {
        let path = history_path(&session.shared.config.projects_dir, &session.project_id);
        if let Err(e) = append_history(&path, &record).await {
            eprintln!("workbench: cannot write {}: {e}", path.display());
        }
    }

    if outcome == Settle::Settled {
        match git::checkpoint(&session.dir, &text).await {
            Ok(Some(cp)) => session.shared.events.emit(
                "checkpoint.created",
                json!({
                    "projectId": session.project_id,
                    "sha": cp.sha,
                    "summary": cp.summary,
                }),
            ),
            Ok(None) => {}
            Err(e) => eprintln!(
                "workbench: checkpoint failed for {}: {e}",
                session.project_id
            ),
        }
    }
    *lock(&session.turn) = None;
}

/// Wait until no tool is running and the agent has been quiet for the settle period.
/// Only [`Settle::Settled`] is a safe checkpoint boundary.
async fn settle(session: &Session) -> Settle {
    let quiet = session.shared.settle_quiet();
    let max = session.shared.max_settle();
    let started = Instant::now();
    loop {
        if session.is_dead() {
            return Settle::Died;
        }
        if started.elapsed() >= max {
            return Settle::TimedOut;
        }
        let wait = {
            let guard = lock(&session.turn);
            match guard.as_ref() {
                None => return Settle::Died,
                Some(t) if t.running_tools() > 0 => Duration::from_millis(50),
                Some(t) => {
                    let idle = t.last_activity.elapsed();
                    if idle >= quiet {
                        return Settle::Settled;
                    }
                    (quiet - idle).min(Duration::from_millis(250))
                }
            }
        };
        tokio::time::sleep(
            wait.min(
                max.saturating_sub(started.elapsed())
                    .max(Duration::from_millis(1)),
            ),
        )
        .await;
    }
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

/// Owns all agent sessions: one live session per project.
pub struct AgentManager {
    shared: Arc<Shared>,
    /// Serialises session starts/stops per project.
    slots: StdMutex<HashMap<String, Arc<Mutex<()>>>>,
    /// The live session of each project.
    live: StdMutex<HashMap<String, Arc<Session>>>,
}

impl AgentManager {
    pub fn new(config: Arc<Config>, projects: Arc<ProjectRegistry>, events: EventBus) -> Self {
        Self {
            shared: Arc::new(Shared {
                config,
                projects,
                events,
                permissions: StdMutex::new(HashMap::new()),
                settle_quiet_ms: AtomicU64::new(DEFAULT_SETTLE_QUIET.as_millis() as u64),
                max_settle_ms: AtomicU64::new(DEFAULT_MAX_SETTLE.as_millis() as u64),
            }),
            slots: StdMutex::new(HashMap::new()),
            live: StdMutex::new(HashMap::new()),
        }
    }

    /// Override the post-turn quiet period (tests use a short one).
    pub fn set_settle_quiet(&self, quiet: Duration) {
        self.shared
            .settle_quiet_ms
            .store(quiet.as_millis() as u64, Ordering::Relaxed);
    }

    /// Override how long a turn may wait for running tools before it fails (tests).
    pub fn set_max_settle(&self, max: Duration) {
        self.shared
            .max_settle_ms
            .store(max.as_millis() as u64, Ordering::Relaxed);
    }

    /// Providers from the registry file (re-read on every call).
    pub fn providers(&self) -> Result<Vec<Provider>, ApiError> {
        Ok(
            AgentsConfig::load(self.shared.config.agents_config.as_deref())?
                .providers()
                .to_vec(),
        )
    }

    fn slot(&self, project_id: &str) -> Arc<Mutex<()>> {
        lock(&self.slots)
            .entry(project_id.to_string())
            .or_default()
            .clone()
    }

    fn live_session(&self, project_id: &str) -> Option<Arc<Session>> {
        lock(&self.live).get(project_id).cloned()
    }

    /// Spawn + initialize + newSession. Any previous session of the project is terminated
    /// first (cleanly: stdin close, then SIGTERM, then SIGKILL).
    pub async fn start_session(
        &self,
        project_id: &str,
        provider_id: &str,
    ) -> Result<String, ApiError> {
        let project = self
            .shared
            .projects
            .get(project_id)
            .await
            .ok_or_else(|| ApiError::enoent(format!("no such project: {project_id}")))?;
        let registry = AgentsConfig::load(self.shared.config.agents_config.as_deref())?;
        let provider = registry
            .get(provider_id)
            .ok_or_else(|| ApiError::badreq(format!("unknown provider: {provider_id}")))?
            .clone();

        let slot = self.slot(project_id);
        let _serial = slot.lock().await;
        let old = lock(&self.live).remove(project_id);
        if let Some(old) = old {
            old.terminate().await;
        }
        let session =
            Session::start(self.shared.clone(), project_id, project.dir, &provider).await?;
        let id = session.id().to_string();
        lock(&self.live).insert(project_id.to_string(), session);
        Ok(id)
    }

    /// Stop the project's session, if any (project delete, shutdown).
    pub async fn terminate_project(&self, project_id: &str) {
        let slot = self.slot(project_id);
        let _serial = slot.lock().await;
        let old = lock(&self.live).remove(project_id);
        if let Some(old) = old {
            old.terminate().await;
        }
        lock(&self.slots).remove(project_id);
    }

    /// Terminate every live session (daemon shutdown).
    pub async fn shutdown(&self) {
        let sessions: Vec<Arc<Session>> = lock(&self.live).drain().map(|(_, s)| s).collect();
        let stops: Vec<_> = sessions
            .into_iter()
            .map(|s| tokio::spawn(async move { s.terminate().await }))
            .collect();
        for stop in stops {
            let _ = stop.await;
        }
    }

    /// Start a turn. Returns the turn id immediately; output streams over the event bus.
    pub async fn prompt(
        &self,
        project_id: &str,
        session_id: &str,
        text: &str,
    ) -> Result<String, ApiError> {
        if text.trim().is_empty() {
            return Err(ApiError::badreq("text is required"));
        }
        let session = self.live_session(project_id).ok_or_else(|| {
            ApiError::enoent(format!("no active agent session for project {project_id}"))
        })?;
        if session.id() != session_id {
            return Err(ApiError::conflict(format!(
                "session {session_id} is not the active session of project {project_id}"
            )));
        }
        if session.is_dead() {
            return Err(ApiError::agent(
                session.describe_failure("agent session is dead; start a new session"),
            ));
        }
        if session
            .awaiting
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(ApiError::conflict("a turn is already running"));
        }
        let mut guard = AwaitingGuard(session.clone(), true);
        // The previous turn may still be settling; wait for it (bounded by the settle cap).
        let turn_guard = session.turn_lock.clone().lock_owned().await;
        if session.is_dead() {
            return Err(ApiError::agent("agent session died"));
        }
        let turn_id = random_id("turn-");
        *lock(&session.turn) = Some(TurnState {
            id: turn_id.clone(),
            user_text: text.to_string(),
            assistant: String::new(),
            tools: Vec::new(),
            last_activity: Instant::now(),
            responded: false,
        });
        guard.1 = false; // `run_turn` owns clearing `awaiting` from here on.
        tokio::spawn(run_turn(
            session,
            turn_id.clone(),
            text.to_string(),
            turn_guard,
        ));
        Ok(turn_id)
    }

    /// Ask the agent to stop the running turn (`session/cancel`). Open permission prompts
    /// are answered `cancelled`, as the protocol requires. Idempotent for finished turns.
    pub fn cancel(&self, turn_id: &str) -> Result<(), ApiError> {
        let sessions: Vec<Arc<Session>> = lock(&self.live).values().cloned().collect();
        for s in sessions {
            let state = lock(&s.turn)
                .as_ref()
                .filter(|t| t.id == turn_id)
                .map(|t| t.responded);
            match state {
                Some(false) => {
                    s.conn
                        .notify("session/cancel", json!({ "sessionId": s.id() }));
                    s.shared.cancel_permissions(s.id());
                    return Ok(());
                }
                Some(true) => return Ok(()),
                None => {}
            }
        }
        Err(ApiError::enoent(format!("no such turn: {turn_id}")))
    }

    /// Answer an open permission prompt.
    pub fn resolve_permission(&self, request_id: &str, decision: Decision) -> Result<(), ApiError> {
        let pending = lock(&self.shared.permissions).remove(request_id);
        let delivered = pending.is_some_and(|p| p.tx.send(Verdict::Decided(decision)).is_ok());
        if delivered {
            Ok(())
        } else {
            Err(ApiError::enoent(format!(
                "no such permission request: {request_id}"
            )))
        }
    }

    /// Persisted turns of a project, optionally for one session.
    pub async fn history(
        &self,
        project_id: &str,
        session: Option<&str>,
    ) -> Result<Vec<TurnRecord>, ApiError> {
        if self.shared.projects.get(project_id).await.is_none() {
            return Err(ApiError::enoent(format!("no such project: {project_id}")));
        }
        read_history(
            &history_path(&self.shared.config.projects_dir, project_id),
            session,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Vec<Value> {
        vec![
            json!({"optionId": "a", "name": "Always", "kind": "allow_always"}),
            json!({"optionId": "o", "name": "Once", "kind": "allow_once"}),
            json!({"optionId": "r", "name": "No", "kind": "reject_once"}),
        ]
    }

    #[test]
    fn file_ops_auto_allowed_commands_are_not() {
        for k in ["read", "edit", "delete", "move", "search", "think"] {
            assert!(is_auto_allowed(k), "{k}");
        }
        for k in ["execute", "fetch", "other", "mystery", ""] {
            assert!(!is_auto_allowed(k), "{k}");
        }
    }

    #[test]
    fn outcome_prefers_once_options() {
        let allow = outcome_for(&Verdict::Decided(Decision::Allow), &opts());
        assert_eq!(allow, json!({"outcome": "selected", "optionId": "o"}));
        let deny = outcome_for(&Verdict::Decided(Decision::Deny), &opts());
        assert_eq!(deny, json!({"outcome": "selected", "optionId": "r"}));
        assert_eq!(
            outcome_for(&Verdict::Cancelled, &opts()),
            json!({"outcome": "cancelled"})
        );
        // No matching option: never select something of the opposite meaning.
        assert_eq!(
            outcome_for(&Verdict::Decided(Decision::Deny), &opts()[..2]),
            json!({"outcome": "cancelled"})
        );
    }

    #[test]
    fn command_extraction() {
        let tc = json!({"title": "Run ls", "rawInput": {"command": "ls -la"}});
        assert_eq!(command_of(&tc, "execute").as_deref(), Some("ls -la"));
        let tc = json!({"rawInput": {"command": ["git", "status"]}});
        assert_eq!(command_of(&tc, "execute").as_deref(), Some("git status"));
        let tc = json!({"title": "npm test"});
        assert_eq!(command_of(&tc, "execute").as_deref(), Some("npm test"));
        assert_eq!(command_of(&tc, "fetch"), None);
    }

    #[test]
    fn tool_upsert_tracks_state() {
        let mut turn = TurnState {
            id: "t".into(),
            user_text: String::new(),
            assistant: String::new(),
            tools: vec![],
            last_activity: Instant::now(),
            responded: false,
        };
        let (_, kind, state, summary) = upsert_tool(
            &mut turn,
            &json!({"toolCallId": "c1", "title": "Edit a.mdx", "kind": "edit", "status": "pending"}),
            true,
        )
        .unwrap();
        assert_eq!(
            (kind.as_str(), state, summary.as_str()),
            ("edit", ToolState::Running, "Edit a.mdx")
        );
        assert_eq!(turn.running_tools(), 1);
        upsert_tool(
            &mut turn,
            &json!({"toolCallId": "c1", "status": "completed"}),
            false,
        );
        assert_eq!(turn.running_tools(), 0);
        assert_eq!(turn.tools[0].title, "Edit a.mdx");
        assert_eq!(turn.tools[0].state, ToolState::Done);
        assert!(upsert_tool(&mut turn, &json!({"status": "completed"}), false).is_none());
    }

    #[tokio::test]
    async fn history_roundtrip_filters_and_skips_torn_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = history_path(dir.path(), "p1");
        assert!(read_history(&path, None).await.unwrap().is_empty());
        let rec = |id: &str, s: &str| TurnRecord {
            id: id.into(),
            session_id: s.into(),
            provider: "fake".into(),
            ts: 1,
            user_text: "u".into(),
            assistant_text: "a".into(),
            tools: vec![ToolSummary {
                tool: "edit".into(),
                summary: "x".into(),
                state: "done".into(),
            }],
            stop_reason: Some("end_turn".into()),
            error: None,
        };
        append_history(&path, &rec("t1", "s1")).await.unwrap();
        append_history(&path, &rec("t2", "s2")).await.unwrap();
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .await
            .unwrap()
            .write_all(b"{\"id\":\"torn")
            .await
            .unwrap();
        assert_eq!(read_history(&path, None).await.unwrap().len(), 2);
        let s2 = read_history(&path, Some("s2")).await.unwrap();
        assert_eq!(s2.len(), 1);
        assert_eq!(s2[0].id, "t2");
    }
}
