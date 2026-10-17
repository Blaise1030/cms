//! Headless chat turns: at most one per project, never while the project's terminal is live.
//!
//! A turn runs the provider's [`headless_spec`] through the login shell (as terminals do), in
//! its own process group, with stdin closed. Its stdout lines go through the provider's
//! [`Translator`]; every chunk is appended to the turn's replay buffer and broadcast under one
//! lock, so a [`Subscription`] sees each chunk exactly once, in its replay or live. Stderr is
//! copied to `{state_dir}/{project}/chat-logs/{chat}.log`, keeping the last 20 lines for error
//! messages.
//!
//! At exit (after a stop: once the whole process group is gone) the turn goes through
//! [`finish_turn`] (checkpoint), ends the stream (`finish`, or
//! `abort` when stopped, then `None`), saves the assistant message to the transcript, updates
//! the chat's history record, forgets the turn and emits `terminal.busy { busy: false }` and
//! `chat.turn { projectId, chatId }`.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr};
use tokio::sync::{broadcast, watch};

use super::chunk::{CheckpointData, UiChunk};
use super::transcript;
use super::translate::{self, Translator};
use crate::config::random_token;
use crate::error::ApiError;
use crate::events::EventBus;
use crate::http::checkpoints::{finish_turn, TurnOutcome};
use crate::projects::STATE_DIR;
use crate::providers::{headless_spec, AgentSession, Provider};
use crate::state::AppState;
use crate::terminal::{history, login_command, signal_group, KILL_GRACE};

const BROADCAST_CAPACITY: usize = 1024;
const STDERR_TAIL_LINES: usize = 20;
const TITLE_MAX: usize = 60;
/// How long an exited CLI's output may keep draining (a background child can hold the pipe).
const DRAIN_WAIT: Duration = Duration::from_millis(200);
/// How long [`ChatManager::stop`] waits for the turn end after the group is gone.
const STOP_WAIT: Duration = Duration::from_secs(10);
const GROUP_POLL: Duration = Duration::from_millis(20);

pub struct TurnRequest {
    pub project_id: String,
    pub chat_id: String,
    pub provider: Provider,
    pub dir: PathBuf,
    pub prompt: String,
    pub session: AgentSession,
}

/// The turn's chunks so far, then the live ones. `None` means the turn is over.
pub struct Subscription {
    pub replay: Vec<UiChunk>,
    pub rx: broadcast::Receiver<Option<UiChunk>>,
}

/// Replay buffer and broadcast sender, always used together under one lock.
struct Stream {
    chunks: Vec<UiChunk>,
    tx: broadcast::Sender<Option<UiChunk>>,
    ended: bool,
}

impl Stream {
    fn push(&mut self, chunk: UiChunk) {
        self.chunks.push(chunk.clone());
        let _ = self.tx.send(Some(chunk));
    }

    fn subscribe(&self) -> Subscription {
        Subscription {
            replay: self.chunks.clone(),
            rx: self.tx.subscribe(),
        }
    }
}

struct Turn {
    chat_id: String,
    /// The CLI's process group (`process_group(0)`: its pid), `None` if it never spawned.
    pgid: Option<libc::pid_t>,
    stopped: AtomicBool,
    stream: Mutex<Stream>,
    /// Flips to `true` once the turn has ended and been forgotten.
    done: watch::Receiver<bool>,
}

impl Turn {
    fn push(&self, chunk: UiChunk) {
        self.stream.lock().unwrap().push(chunk);
    }
}

type Turns = Arc<Mutex<HashMap<String, Arc<Turn>>>>;

/// Owns every project's running chat turn.
pub struct ChatManager {
    events: EventBus,
    turns: Turns,
}

impl ChatManager {
    pub fn new(events: EventBus) -> Self {
        Self {
            events,
            turns: Arc::default(),
        }
    }

    /// The chat id of the project's running turn.
    pub fn running(&self, project_id: &str) -> Option<String> {
        self.get(project_id).map(|t| t.chat_id.clone())
    }

    /// Spawn the CLI for `req` and stream its turn. `conflict` while the project's terminal is
    /// live or another turn is running.
    pub fn start(&self, state: AppState, req: TurnRequest) -> Result<Subscription, ApiError> {
        if !transcript::valid_chat_id(&req.chat_id) {
            return Err(ApiError::badreq(format!(
                "invalid chat id: {}",
                req.chat_id
            )));
        }
        let translator = translate::for_provider(&req.provider.id).ok_or_else(|| {
            ApiError::badreq(format!("{} cannot run in the chat panel", req.provider.id))
        })?;
        if !req.dir.is_dir() {
            return Err(ApiError::enoent(format!(
                "project directory is not available: {}",
                req.dir.display()
            )));
        }
        if state.terminals.status(&req.project_id).is_some() {
            return Err(ApiError::conflict("The agent is busy in Terminal"));
        }
        // Spawning is quick; holding the map lock makes check-and-insert atomic.
        let mut turns = self.turns.lock().unwrap();
        if turns.contains_key(&req.project_id) {
            return Err(ApiError::conflict("The agent is busy in Chat"));
        }
        let state_dir = state.config.projects_dir.join(STATE_DIR);
        let spec = headless_spec(
            &req.provider,
            &req.prompt,
            &state_dir,
            &req.project_id,
            &req.session,
        );
        let mut cmd = login_command(&spec.argv, &req.dir);
        cmd.envs(spec.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .process_group(0);
        let child = match cmd.spawn() {
            Ok(child) => Some(child),
            // Reported in the stream like a missing CLI (exit 127).
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(ApiError::io(format!("failed to start the agent: {e}"))),
        };
        let pgid = child
            .as_ref()
            .and_then(Child::id)
            .and_then(|pid| libc::pid_t::try_from(pid).ok());

        let user =
            transcript::user_message(&format!("user-{}", &random_token()[..16]), &req.prompt);
        if let Err(e) = transcript::append(&state_dir, &req.project_id, &req.chat_id, &[user]) {
            eprintln!("workbench: cannot save the chat transcript: {e}");
        }
        let message_id = format!("msg-{}", &random_token()[..16]);
        let mut stream = Stream {
            chunks: Vec::new(),
            tx: broadcast::channel(BROADCAST_CAPACITY).0,
            ended: false,
        };
        stream.push(UiChunk::Start {
            message_id: message_id.clone(),
        });
        stream.push(UiChunk::StartStep);
        let subscription = stream.subscribe();
        let (done_tx, done) = watch::channel(false);
        let turn = Arc::new(Turn {
            chat_id: req.chat_id.clone(),
            pgid,
            stopped: AtomicBool::new(false),
            stream: Mutex::new(stream),
            done,
        });
        turns.insert(req.project_id.clone(), turn.clone());
        drop(turns);

        emit_busy(&self.events, &req.project_id, true);
        let task = TurnTask {
            state,
            events: self.events.clone(),
            turns: self.turns.clone(),
            turn,
            req,
            state_dir,
            message_id,
            done: done_tx,
        };
        tokio::spawn(task.run(child, translator));
        Ok(subscription)
    }

    /// The running turn of `chat_id`: chunks so far and the live ones, taken atomically.
    pub fn subscribe(&self, project_id: &str, chat_id: &str) -> Option<Subscription> {
        let turn = self.get(project_id).filter(|t| t.chat_id == chat_id)?;
        let stream = turn.stream.lock().unwrap();
        (!stream.ended).then(|| stream.subscribe())
    }

    /// `SIGTERM` the turn's process group, `SIGKILL` after 2 s, then wait for the turn end
    /// (which still checkpoints). No-op if no turn is running.
    pub async fn stop(&self, project_id: &str) {
        let Some(turn) = self.get(project_id) else {
            return;
        };
        turn.stopped.store(true, Ordering::SeqCst);
        signal_group(turn.pgid, libc::SIGTERM);
        kill_group_after_grace(turn.pgid).await;
        let mut done = turn.done.clone();
        let _ = tokio::time::timeout(STOP_WAIT, done.wait_for(|d| *d)).await;
    }

    /// Stop every running turn.
    pub async fn shutdown(&self) {
        let ids: Vec<String> = self.turns.lock().unwrap().keys().cloned().collect();
        futures_util::future::join_all(ids.iter().map(|id| self.stop(id))).await;
    }

    fn get(&self, project_id: &str) -> Option<Arc<Turn>> {
        self.turns.lock().unwrap().get(project_id).cloned()
    }
}

/// Everything the background half of a turn needs.
struct TurnTask {
    state: AppState,
    events: EventBus,
    turns: Turns,
    turn: Arc<Turn>,
    req: TurnRequest,
    state_dir: PathBuf,
    message_id: String,
    done: watch::Sender<bool>,
}

impl TurnTask {
    async fn run(self, child: Option<Child>, mut translator: Box<dyn Translator>) {
        let (exit, stderr_tail) = match child {
            Some(child) => self.drive(child, translator.as_mut()).await,
            None => (Some(127), String::new()),
        };
        let stopped = self.turn.stopped.load(Ordering::SeqCst);
        if stopped {
            // Members ignoring SIGTERM can still write: checkpoint and go idle only once the
            // group is gone (escalating here too, in case `stop` was cancelled).
            kill_group_after_grace(self.turn.pgid).await;
        }
        // A stop is not a failure: only close the open parts.
        let closing = if stopped {
            translator.finish(None, "")
        } else {
            translator.finish(exit, &stderr_tail)
        };
        for chunk in closing {
            self.turn.push(chunk);
        }

        let project = &self.req.project_id;
        let summary = translator.summary().map(str::to_owned);
        match finish_turn(&self.state, project, summary.as_deref()).await {
            Ok(TurnOutcome::Committed { checkpoint, files }) => {
                self.turn.push(UiChunk::DataCheckpoint {
                    data: CheckpointData {
                        sha: Some(checkpoint.sha),
                        summary: Some(checkpoint.summary),
                        files,
                        blocked: None,
                    },
                })
            }
            Ok(TurnOutcome::Blocked(reason)) => self.turn.push(UiChunk::DataCheckpoint {
                data: CheckpointData {
                    blocked: Some(reason),
                    ..Default::default()
                },
            }),
            Ok(TurnOutcome::Unchanged) => {}
            Err(e) => eprintln!(
                "workbench: chat turn checkpoint failed for {project}: {}",
                e.message
            ),
        }
        if stopped {
            self.turn.push(UiChunk::Abort);
        } else {
            self.turn.push(UiChunk::FinishStep);
            self.turn.push(UiChunk::Finish);
        }

        let chunks = self.turn.stream.lock().unwrap().chunks.clone();
        let message = transcript::assemble(&self.message_id, &chunks);
        if let Err(e) = transcript::append(&self.state_dir, project, &self.turn.chat_id, &[message])
        {
            eprintln!("workbench: cannot save the chat transcript: {e}");
        }
        self.update_history(translator.session_id());
        {
            let mut stream = self.turn.stream.lock().unwrap();
            stream.ended = true;
            let _ = stream.tx.send(None);
        }
        {
            // Forgotten before `chat.turn`, so a listener can start the next turn right away.
            let mut turns = self.turns.lock().unwrap();
            if turns
                .get(project)
                .is_some_and(|t| Arc::ptr_eq(t, &self.turn))
            {
                turns.remove(project);
            }
        }
        emit_busy(&self.events, project, false);
        self.events.emit(
            "chat.turn",
            json!({ "projectId": project, "chatId": self.turn.chat_id }),
        );
        let _ = self.done.send(true);
    }

    /// Feed stdout to the translator until it closes; the exit code and the stderr tail.
    async fn drive(
        &self,
        mut child: Child,
        translator: &mut dyn Translator,
    ) -> (Option<i32>, String) {
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let log = self
            .state_dir
            .join(&self.req.project_id)
            .join("chat-logs")
            .join(format!("{}.log", self.turn.chat_id));
        let stderr = child
            .stderr
            .take()
            .map(|err| tokio::spawn(copy_stderr(err, log, tail.clone())));
        let exit = self.read_stdout(&mut child, translator).await;
        if let Some(task) = stderr {
            let _ = tokio::time::timeout(DRAIN_WAIT, task).await;
        }
        let tail = Vec::from(tail.lock().unwrap().clone()).join("\n");
        (exit, tail)
    }

    async fn read_stdout(&self, child: &mut Child, translator: &mut dyn Translator) -> Option<i32> {
        let Some(stdout) = child.stdout.take() else {
            return child.wait().await.ok().and_then(|s| s.code());
        };
        let mut reader = BufReader::new(stdout);
        let mut line = Vec::new();
        let mut exit = None;
        let drain = tokio::time::sleep(Duration::ZERO);
        tokio::pin!(drain);
        loop {
            tokio::select! {
                read = reader.read_until(b'\n', &mut line) => {
                    if !matches!(read, Ok(1..)) {
                        break;
                    }
                    let text = String::from_utf8_lossy(&line);
                    for chunk in translator.feed(text.trim_end_matches(['\n', '\r'])) {
                        self.turn.push(chunk);
                    }
                    line.clear();
                }
                status = child.wait(), if exit.is_none() => {
                    exit = Some(status.ok().and_then(|s| s.code()));
                    drain.as_mut().reset(tokio::time::Instant::now() + DRAIN_WAIT);
                }
                () = &mut drain, if exit.is_some() => {
                    // The CLI is gone but something it started still holds stdout.
                    signal_group(self.turn.pgid, libc::SIGTERM);
                    break;
                }
            }
        }
        match exit {
            Some(code) => code,
            None => child.wait().await.ok().and_then(|s| s.code()),
        }
    }

    fn update_history(&self, session_id: Option<&str>) {
        let title: String = self
            .req
            .prompt
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .chars()
            .take(TITLE_MAX)
            .collect();
        let updated = history::update(
            &self.state_dir,
            &self.req.project_id,
            &self.turn.chat_id,
            |record| {
                if let Some(id) = session_id {
                    record.agent_session_id = Some(id.to_string());
                }
                if record.title.as_deref().is_none_or(str::is_empty) && !title.is_empty() {
                    record.title = Some(title);
                }
                record.updated_at = history::now_ms();
            },
        );
        if let Err(e) = updated {
            eprintln!("workbench: cannot save chat session history: {e}");
        }
    }
}

/// Copy the CLI's stderr to its chat log, keeping the last lines in `tail`.
async fn copy_stderr(stderr: ChildStderr, log: PathBuf, tail: Arc<Mutex<VecDeque<String>>>) {
    let mut file = open_log(&log).await;
    let mut reader = BufReader::new(stderr);
    let mut line = Vec::new();
    while matches!(reader.read_until(b'\n', &mut line).await, Ok(1..)) {
        if let Some(f) = &mut file {
            let _ = f.write_all(&line).await;
        }
        let text = String::from_utf8_lossy(&line).trim_end().to_string();
        let mut tail = tail.lock().unwrap();
        tail.push_back(text);
        if tail.len() > STDERR_TAIL_LINES {
            tail.pop_front();
        }
        line.clear();
    }
}

async fn open_log(log: &Path) -> Option<tokio::fs::File> {
    if let Some(dir) = log.parent() {
        let _ = tokio::fs::create_dir_all(dir).await;
    }
    match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .await
    {
        Ok(file) => Some(file),
        Err(e) => {
            eprintln!("workbench: cannot open chat log {}: {e}", log.display());
            None
        }
    }
}

/// Give a signalled group [`KILL_GRACE`] to exit, then `SIGKILL` it and wait as long again.
async fn kill_group_after_grace(pgid: Option<libc::pid_t>) {
    if !group_ends(pgid, KILL_GRACE).await {
        signal_group(pgid, libc::SIGKILL);
        group_ends(pgid, KILL_GRACE).await;
    }
}

/// Whether the process group is gone within `limit` (it may never have existed).
async fn group_ends(pgid: Option<libc::pid_t>, limit: Duration) -> bool {
    let Some(pgid) = pgid.filter(|p| *p > 0) else {
        return true;
    };
    let deadline = tokio::time::Instant::now() + limit;
    loop {
        let gone = unsafe { libc::killpg(pgid, 0) } == -1
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        if gone {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(GROUP_POLL).await;
    }
}

fn emit_busy(events: &EventBus, project_id: &str, busy: bool) {
    events.emit(
        "terminal.busy",
        json!({ "projectId": project_id, "busy": busy }),
    );
}
