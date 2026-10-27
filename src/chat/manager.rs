//! One long-lived ACP agent per `(project_id, chat_id)`.
//!
//! Two chats in the same project are two processes. There is no PTY claim:
//! project delete uses [`ChatManager::any_live`]; Get latest only waits for
//! [`ChatManager::any_running`] prompts, since an idle agent edits nothing.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use serde_json::json;
use tokio::sync::{broadcast, watch, Notify};

use super::acp::{AcpClient, AcpEvent};
use super::chunk::UiChunk;
use super::models::ModelConfig;
use super::normalize::to_chunks;
use super::transcript;
use crate::events::EventBus;

/// How long an agent gets to answer `initialize` / `session/new`.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

const BROADCAST_CAPACITY: usize = 256;

#[derive(Debug)]
pub enum ChatError {
    Invalid(String),
    NotLive,
    Failed(String),
}

impl std::fmt::Display for ChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatError::Invalid(m) | ChatError::Failed(m) => write!(f, "{m}"),
            ChatError::NotLive => write!(f, "no live chat for this session"),
        }
    }
}

impl std::error::Error for ChatError {}

pub type ChatResult<T> = Result<T, ChatError>;

/// Fired after a `turn_end` chunk is recorded. [`crate::state::AppState`] hooks
/// checkpointing and the preview/content refresh onto it.
#[derive(Debug, Clone)]
pub struct TurnEnded {
    pub project_id: String,
    pub chat_id: String,
    pub session_id: String,
    pub stop_reason: String,
    /// The turn's last agent text, used as the checkpoint's commit subject.
    pub summary: Option<String>,
    pub cwd: PathBuf,
}

pub type TurnEndHook = Arc<dyn Fn(TurnEnded) + Send + Sync>;

/// Replay of chunks already recorded, then the live ones.
pub struct Subscription {
    pub replay: Vec<UiChunk>,
    pub rx: broadcast::Receiver<UiChunk>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    project_id: String,
    chat_id: String,
}

struct LiveChat {
    client: Arc<AcpClient>,
    acp_session_id: String,
    /// Token the agent was given in `ORSO_HOOK_TOKEN`; `orso ctl` presents it.
    hook_token: String,
    /// A prompt is in flight (a turn is running).
    running: AtomicBool,
    tx: broadcast::Sender<UiChunk>,
    history: Mutex<Vec<UiChunk>>,
    models: Mutex<Option<ModelConfig>>,
    state_dir: PathBuf,
    project_id: String,
    chat_id: String,
    cwd: PathBuf,
    /// Shared with the `Starting` entry so a stop during handshake is
    /// still visible once the client exists.
    cancel: Arc<AtomicBool>,
    /// Process death is one error chunk, then the live entry is dropped.
    exited: AtomicBool,
    /// Every `session/prompt` task spawned for this chat, including ones a
    /// later send already aborted. Dropping a `JoinHandle` does not abort, so
    /// the handles stay here until drop, which aborts all of them. Otherwise
    /// an older task keeps an `AcpClient` and the child survives `forget`.
    prompts: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl Drop for LiveChat {
    fn drop(&mut self) {
        let prompts = self.prompts.get_mut().unwrap_or_else(|e| e.into_inner());
        for task in prompts.drain(..) {
            task.abort();
        }
    }
}

impl LiveChat {
    /// One chunk, in order: written to the transcript, kept for SSE replay,
    /// then sent to live subscribers. Push and broadcast share the history
    /// lock with [`ChatManager::subscribe`], so a chunk is in exactly one
    /// of a subscriber's replay or live stream.
    fn record(&self, chunk: UiChunk) {
        if let Err(e) = transcript::append(&self.state_dir, &self.project_id, &self.chat_id, &chunk)
        {
            eprintln!(
                "orso: failed to write transcript of {}/{}: {e}",
                self.project_id, self.chat_id
            );
        }
        let mut history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        history.push(chunk.clone());
        let _ = self.tx.send(chunk);
    }
}

/// The agent text of the turn that just ended, for the checkpoint subject.
fn last_turn_text(live: &LiveChat) -> Option<String> {
    let history = live.history.lock().unwrap_or_else(|e| e.into_inner());
    let start = history
        .iter()
        .rposition(|chunk| matches!(chunk, UiChunk::UserText { .. }))
        .map_or(0, |at| at + 1);
    let text: String = history[start..]
        .iter()
        .filter_map(|chunk| match chunk {
            UiChunk::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// A start in flight. Its handshake runs outside the map lock; waiters park
/// on `done` until the live chat (or the failure) is published.
struct Starting {
    done: watch::Sender<bool>,
    cancel: Arc<AtomicBool>,
    /// `forget` dropped this reservation. The handshake must not publish, and
    /// a start waiting on it must not spawn a replacement.
    abandoned: AtomicBool,
}

enum Entry {
    Live(Arc<LiveChat>),
    Starting(Arc<Starting>),
}

struct Slots {
    chats: HashMap<Key, Entry>,
    /// Projects inside [`ChatManager::stop_project`]. Cleared under this same
    /// mutex only once that project has no entries, so a `start` cannot
    /// publish in the gap after the last cancel await.
    stopping: HashSet<String>,
}

/// Test seam: `stop_project` notifies `entered` after it has marked the
/// project and taken the current entries, then waits on `release`.
#[cfg(test)]
struct StopGate {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

struct Inner {
    slots: Mutex<Slots>,
    state_dir: PathBuf,
    events: EventBus,
    on_turn_end: Mutex<Option<TurnEndHook>>,
    /// Projects whose permission requests are allowed without asking.
    auto_approve: Mutex<HashSet<String>>,
    /// Set by [`ChatManager::shutdown`]. A handshake that finishes after
    /// this does not publish a live chat.
    shutdown: AtomicBool,
    #[cfg(test)]
    stop_gate: Mutex<Option<StopGate>>,
}

/// Every live chat. Clones share the same map.
#[derive(Clone)]
pub struct ChatManager {
    inner: Arc<Inner>,
}

fn split_argv(argv: &[String]) -> ChatResult<(&str, &[String])> {
    match argv.split_first() {
        Some((cmd, args)) if !cmd.is_empty() => Ok((cmd.as_str(), args)),
        _ => Err(ChatError::Invalid("empty ACP command".into())),
    }
}

impl ChatManager {
    fn slots(&self) -> std::sync::MutexGuard<'_, Slots> {
        self.inner.slots.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn new(events: EventBus, state_dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                slots: Mutex::new(Slots {
                    chats: HashMap::new(),
                    stopping: HashSet::new(),
                }),
                state_dir,
                events,
                on_turn_end: Mutex::new(None),
                auto_approve: Mutex::new(HashSet::new()),
                shutdown: AtomicBool::new(false),
                #[cfg(test)]
                stop_gate: Mutex::new(None),
            }),
        }
    }

    /// Turns auto-approval of permission requests on or off for a project.
    pub fn set_auto_approve(&self, project_id: &str, on: bool) {
        let mut set = self
            .inner
            .auto_approve
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if on {
            set.insert(project_id.to_string());
        } else {
            set.remove(project_id);
        }
    }

    fn auto_approves(&self, project_id: &str) -> bool {
        self.inner
            .auto_approve
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(project_id)
    }

    /// Permission requests the live chat's agent is still waiting on.
    pub fn pending_permissions(&self, project_id: &str, chat_id: &str) -> Vec<String> {
        self.live(project_id, chat_id)
            .map(|live| live.client.pending_permission_ids())
            .unwrap_or_default()
    }

    /// Checkpoint / content refresh runs after each `turn_end` chunk; wired by
    /// [`crate::state::AppState`]. The hook is synchronous and should not block
    /// the event pump for long.
    pub fn set_on_turn_end(&self, hook: impl Fn(TurnEnded) + Send + Sync + 'static) {
        *self
            .inner
            .on_turn_end
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(hook));
    }

    /// Starts the chat's ACP agent. A second call for the same key reuses the
    /// process already live or still handshaking — it does not spawn another.
    pub async fn start(
        &self,
        project_id: &str,
        chat_id: &str,
        cwd: &Path,
        acp_argv: &[String],
        env: &[(&str, &str)],
    ) -> ChatResult<()> {
        self.start_inner(project_id, chat_id, cwd, acp_argv, env)
            .await
            .map(|_| ())
    }

    /// [`start`], named for callers that only want an agent if one is missing.
    pub async fn ensure_started(
        &self,
        project_id: &str,
        chat_id: &str,
        cwd: &Path,
        acp_argv: &[String],
        env: &[(&str, &str)],
    ) -> ChatResult<()> {
        self.start(project_id, chat_id, cwd, acp_argv, env).await
    }

    async fn start_inner(
        &self,
        project_id: &str,
        chat_id: &str,
        cwd: &Path,
        acp_argv: &[String],
        env: &[(&str, &str)],
    ) -> ChatResult<Arc<LiveChat>> {
        self.check_ids(project_id, chat_id)?;
        if !cwd.is_dir() {
            return Err(ChatError::Invalid(format!(
                "cwd is not a directory: {}",
                cwd.display()
            )));
        }
        if self.inner.shutdown.load(Ordering::SeqCst) {
            return Err(ChatError::Failed("chat manager is shut down".into()));
        }
        let key = Key {
            project_id: project_id.to_string(),
            chat_id: chat_id.to_string(),
        };

        enum Action {
            Existing(Arc<LiveChat>),
            Wait(Arc<Starting>),
            Reserve(Arc<Starting>),
        }
        let starting = loop {
            let action = {
                let mut slots = self.slots();
                // Re-checked under the map lock: `stop_project` sets `stopping`
                // before it awaits, and clears it only while the map is empty.
                if self.inner.shutdown.load(Ordering::SeqCst) || slots.stopping.contains(project_id)
                {
                    return Err(ChatError::Failed("chat start was cancelled".into()));
                }
                match slots.chats.get(&key) {
                    Some(Entry::Live(live)) => Action::Existing(live.clone()),
                    Some(Entry::Starting(s)) => Action::Wait(s.clone()),
                    None => {
                        let s = Arc::new(Starting {
                            done: watch::channel(false).0,
                            cancel: Arc::new(AtomicBool::new(false)),
                            abandoned: AtomicBool::new(false),
                        });
                        slots.chats.insert(key.clone(), Entry::Starting(s.clone()));
                        Action::Reserve(s)
                    }
                }
            };
            match action {
                Action::Existing(live) => return Ok(live),
                Action::Wait(s) => {
                    let mut rx = s.done.subscribe();
                    let _ = rx.wait_for(|done| *done).await;
                    if s.abandoned.load(Ordering::SeqCst) {
                        return Err(ChatError::Failed("chat start was cancelled".into()));
                    }
                }
                Action::Reserve(s) => break s,
            }
        };

        let handshake = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            let (cmd, args) = split_argv(acp_argv)?;
            let hook_token = crate::config::random_token()[..32].to_string();
            let child_env: Vec<(String, String)> = env
                .iter()
                .filter(|(k, _)| *k != "ORSO_HOOK_TOKEN")
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .chain(std::iter::once((
                    "ORSO_HOOK_TOKEN".to_string(),
                    hook_token.clone(),
                )))
                .collect();
            let env_refs: Vec<(&str, &str)> = child_env
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let client = AcpClient::spawn_with_env(cmd, args, cwd, &env_refs)
                .await
                .map_err(|e| ChatError::Failed(e.to_string()))?;
            client
                .initialize()
                .await
                .map_err(|e| ChatError::Failed(e.to_string()))?;
            let created = client
                .session_new(cwd)
                .await
                .map_err(|e| ChatError::Failed(e.to_string()))?;
            // A resumed backend session can still hold a prompt from a prior
            // process. Clear it so the first real message is accepted.
            let _ = client.cancel(&created.session_id).await;
            client.clear_cancelled();
            Ok::<_, ChatError>((client, created, hook_token))
        })
        .await
        .map_err(|_| ChatError::Failed("ACP agent handshake timed out".into()))
        .and_then(|r| r);

        let (client, created, hook_token) = match handshake {
            Ok(pair) => pair,
            Err(e) => {
                self.note_error(project_id, chat_id, &e.to_string());
                self.clear_reservation(&key, &starting);
                return Err(e);
            }
        };

        let client = Arc::new(client);
        let mut rx = client.event_stream();
        let live = Arc::new(LiveChat {
            client,
            acp_session_id: created.session_id,
            hook_token,
            running: AtomicBool::new(false),
            tx: broadcast::channel(BROADCAST_CAPACITY).0,
            history: Mutex::new(transcript::load(&self.inner.state_dir, project_id, chat_id)),
            models: Mutex::new(created.models),
            state_dir: self.inner.state_dir.clone(),
            project_id: project_id.to_string(),
            chat_id: chat_id.to_string(),
            cwd: cwd.to_path_buf(),
            cancel: starting.cancel.clone(),
            exited: AtomicBool::new(false),
            prompts: Mutex::new(Vec::new()),
        });

        let publish = {
            let mut slots = self.slots();
            let still_ours = matches!(slots.chats.get(&key), Some(Entry::Starting(s)) if Arc::ptr_eq(s, &starting));
            if self.inner.shutdown.load(Ordering::SeqCst)
                || slots.stopping.contains(project_id)
                || !still_ours
            {
                false
            } else {
                if live.cancel.load(Ordering::SeqCst) {
                    live.client.mark_cancelled();
                }
                slots.chats.insert(key.clone(), Entry::Live(live.clone()));
                true
            }
        };
        if !publish {
            drop(live);
            let _ = starting.done.send(true);
            return Err(ChatError::Failed("chat start was cancelled".into()));
        }

        let weak = Arc::downgrade(&live);
        let mgr = self.clone();
        let card = key.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(AcpEvent::AgentExited) => {
                        let deadline = tokio::time::Instant::now() + Duration::from_millis(50);
                        loop {
                            let left =
                                deadline.saturating_duration_since(tokio::time::Instant::now());
                            if left.is_zero() {
                                break;
                            }
                            match tokio::time::timeout(left, rx.recv()).await {
                                Ok(Some(AcpEvent::AgentExited)) => continue,
                                Ok(Some(ev)) => {
                                    mgr.apply_event(&weak, &ev);
                                }
                                _ => break,
                            }
                        }
                        mgr.note_agent_exit(&card);
                        break;
                    }
                    None => {
                        mgr.note_agent_exit(&card);
                        break;
                    }
                    Some(ev) => {
                        if weak.upgrade().is_none() {
                            break;
                        }
                        mgr.apply_event(&weak, &ev);
                    }
                }
            }
        });
        let _ = starting.done.send(true);
        Ok(live)
    }

    fn check_ids(&self, project_id: &str, chat_id: &str) -> ChatResult<()> {
        if !transcript::valid_project_id(project_id) {
            return Err(ChatError::Invalid(format!(
                "invalid project id: {project_id}"
            )));
        }
        if !transcript::valid_chat_id(chat_id) {
            return Err(ChatError::Invalid(format!("invalid chat id: {chat_id}")));
        }
        Ok(())
    }

    fn clear_reservation(&self, key: &Key, starting: &Arc<Starting>) {
        let mut slots = self.slots();
        if matches!(slots.chats.get(key), Some(Entry::Starting(s)) if Arc::ptr_eq(s, starting)) {
            slots.chats.remove(key);
        }
        drop(slots);
        let _ = starting.done.send(true);
    }

    /// A failed spawn or handshake is an error chunk in the transcript, not a
    /// hung live entry.
    fn note_error(&self, project_id: &str, chat_id: &str, message: &str) {
        let chunk = UiChunk::Error {
            message: message.to_string(),
        };
        if let Err(e) = transcript::append(&self.inner.state_dir, project_id, chat_id, &chunk) {
            eprintln!("orso: failed to write chat error for {project_id}/{chat_id}: {e}");
        }
    }

    fn apply_event(&self, live: &Weak<LiveChat>, ev: &AcpEvent) {
        let Some(live) = live.upgrade() else {
            return;
        };
        let chunks = to_chunks(ev);
        let turn_end = chunks.iter().find_map(|chunk| match chunk {
            UiChunk::TurnEnd {
                session_id,
                stop_reason,
            } => Some((session_id.clone(), stop_reason.clone())),
            _ => None,
        });
        for chunk in chunks {
            live.record(chunk);
        }
        if let AcpEvent::PermissionRequest { request_id, .. } = ev {
            if self.auto_approves(&live.project_id) {
                let client = live.client.clone();
                let request_id = request_id.clone();
                tokio::spawn(async move {
                    let _ = client.answer_permission(&request_id, true).await;
                });
            } else {
                self.inner.events.emit(
                    "chat.permission",
                    json!({"projectId": live.project_id, "chatId": live.chat_id}),
                );
            }
        }
        if let Some((session_id, stop_reason)) = turn_end {
            self.note_turn_end(&live, &session_id, &stop_reason);
        }
    }

    /// `chat.turn` plus the Task 6 hook. Shared by ACP `PromptEnded` and the
    /// cancel latch, which records `TurnEnd` without a prompt response.
    fn note_turn_end(&self, live: &LiveChat, session_id: &str, stop_reason: &str) {
        live.running.store(false, Ordering::SeqCst);
        let ended = TurnEnded {
            project_id: live.project_id.clone(),
            chat_id: live.chat_id.clone(),
            session_id: session_id.to_string(),
            stop_reason: stop_reason.to_string(),
            summary: last_turn_text(live),
            cwd: live.cwd.clone(),
        };
        let hook = self
            .inner
            .on_turn_end
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        self.inner.events.emit(
            "chat.turn",
            json!({
                "projectId": ended.project_id,
                "chatId": ended.chat_id,
            }),
        );
        if let Some(hook) = hook {
            hook(ended);
        }
    }

    /// Records `text` as the user's chunk and runs it as an ACP prompt.
    /// The turn's chunks arrive through the event pump.
    fn dispatch(&self, live: &Arc<LiveChat>, text: &str) {
        live.running.store(true, Ordering::SeqCst);
        live.record(UiChunk::UserText {
            session_id: live.acp_session_id.clone(),
            text: text.to_string(),
        });
        if live.cancel.load(Ordering::SeqCst) {
            let session_id = live.acp_session_id.clone();
            live.record(UiChunk::TurnEnd {
                session_id: session_id.clone(),
                stop_reason: "cancelled".into(),
            });
            self.note_turn_end(live, &session_id, "cancelled");
            return;
        }
        // Abort earlier prompt tasks but keep their handles until drop so a
        // task that is still inside a poll is aborted again when the chat goes.
        let replaced = {
            let mut prompts = live.prompts.lock().unwrap_or_else(|e| e.into_inner());
            let replaced = prompts.iter().any(|task| !task.is_finished());
            for task in prompts.iter() {
                task.abort();
            }
            prompts.retain(|task| !task.is_finished());
            replaced
        };
        let client = live.client.clone();
        let acp_session_id = live.acp_session_id.clone();
        let text = text.to_string();
        let weak = Arc::downgrade(live);
        let mgr = self.clone();
        let task = tokio::spawn(async move {
            if replaced {
                // The previous prompt may still be sitting in the agent
                // (`stuck` / `slow`). Cancel it before writing another.
                let _ = client.cancel(&acp_session_id).await;
                client.clear_cancelled();
            }
            let first = client.prompt(&acp_session_id, &text).await;
            let result = match first {
                Err(e) if prompt_busy(&e.to_string()) => {
                    let _ = client.cancel(&acp_session_id).await;
                    client.clear_cancelled();
                    client.prompt(&acp_session_id, &text).await
                }
                other => other,
            };
            if let Err(e) = result {
                eprintln!("orso: ACP prompt failed: {e}");
                let message = e.to_string();
                let Some(live) = weak.upgrade() else {
                    return;
                };
                if agent_gone(&message) {
                    mgr.note_agent_exit(&Key {
                        project_id: live.project_id.clone(),
                        chat_id: live.chat_id.clone(),
                    });
                } else {
                    live.record(UiChunk::Error { message });
                }
            }
        });
        live.prompts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(task);
    }

    fn note_agent_exit(&self, key: &Key) {
        let Some(live) = self.live(&key.project_id, &key.chat_id) else {
            return;
        };
        if live.exited.swap(true, Ordering::SeqCst) {
            return;
        }
        live.record(UiChunk::Error {
            message: "ACP agent exited".into(),
        });
        self.forget(&key.project_id, &key.chat_id);
    }

    pub async fn send_message(
        &self,
        project_id: &str,
        chat_id: &str,
        text: &str,
    ) -> ChatResult<()> {
        let live = self.live(project_id, chat_id).ok_or(ChatError::NotLive)?;
        // A stop latches the chat until the user sends again.
        live.cancel.store(false, Ordering::SeqCst);
        live.client.clear_cancelled();
        self.dispatch(&live, text);
        Ok(())
    }

    /// Latches cancel for a live chat or one still handshaking. A live agent's
    /// in-flight prompts end `cancelled`. With no chat yet this is a no-op.
    pub async fn cancel(&self, project_id: &str, chat_id: &str) -> ChatResult<()> {
        let client = {
            let slots = self.slots();
            match slots.chats.get(&Key {
                project_id: project_id.to_string(),
                chat_id: chat_id.to_string(),
            }) {
                Some(Entry::Live(live)) => {
                    live.cancel.store(true, Ordering::SeqCst);
                    Some((live.client.clone(), live.acp_session_id.clone()))
                }
                Some(Entry::Starting(starting)) => {
                    starting.cancel.store(true, Ordering::SeqCst);
                    None
                }
                None => None,
            }
        };
        if let Some((client, acp_session_id)) = client {
            client
                .cancel(&acp_session_id)
                .await
                .map_err(|e| ChatError::Failed(e.to_string()))?;
        }
        Ok(())
    }

    /// Answers a `session/request_permission` the agent is blocked on.
    /// An id that is not pending still succeeds.
    pub async fn answer(
        &self,
        project_id: &str,
        chat_id: &str,
        request_id: &str,
        allow: bool,
    ) -> ChatResult<()> {
        let live = self.live(project_id, chat_id).ok_or(ChatError::NotLive)?;
        live.client
            .answer_permission(request_id, allow)
            .await
            .map_err(|e| ChatError::Failed(e.to_string()))
    }

    /// Models the live session advertises. `None` when not live or empty.
    pub fn models(&self, project_id: &str, chat_id: &str) -> Option<ModelConfig> {
        let live = self.live(project_id, chat_id)?;
        let models = live.models.lock().unwrap_or_else(|e| e.into_inner());
        models.clone()
    }

    /// Selects a model on the live ACP session. Refused while a turn is open.
    pub async fn set_model(
        &self,
        project_id: &str,
        chat_id: &str,
        model_id: &str,
    ) -> ChatResult<()> {
        wait_model_gate(project_id, chat_id).await;
        let live = self.live(project_id, chat_id).ok_or(ChatError::NotLive)?;
        // Already selected: a second cold start can observe the first message's
        // open turn. Refusing here drops that message. A different model still waits.
        {
            let models = live.models.lock().unwrap_or_else(|e| e.into_inner());
            if models.as_ref().is_some_and(|cfg| cfg.current == model_id) {
                return Ok(());
            }
        }
        {
            let history = live.history.lock().unwrap_or_else(|e| e.into_inner());
            if transcript_turn_open(&history) {
                return Err(ChatError::Invalid(
                    "Wait for the current turn to finish before changing the model.".into(),
                ));
            }
        }
        let config_id = {
            let models = live.models.lock().unwrap_or_else(|e| e.into_inner());
            let Some(cfg) = models.as_ref() else {
                return Err(ChatError::Invalid(
                    "This agent does not expose a model list.".into(),
                ));
            };
            if !cfg.options.iter().any(|o| o.id == model_id) {
                return Err(ChatError::Invalid(format!("Unknown model: {model_id}")));
            }
            cfg.config_id.clone()
        };
        let refreshed = live
            .client
            .set_config_option(&live.acp_session_id, &config_id, model_id)
            .await
            .map_err(|e| ChatError::Failed(e.to_string()))?;
        let mut models = live.models.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(next) = refreshed {
            *models = Some(next);
        } else if let Some(cfg) = models.as_mut() {
            cfg.current = model_id.to_string();
        }
        Ok(())
    }

    /// Drops a live chat or a handshake still in `Starting`. The live agent
    /// dies with its last client reference (`kill_on_drop`). A handshake
    /// whose reservation was removed does not publish, including when
    /// `forget` wins the race against the publish lock.
    pub fn forget(&self, project_id: &str, chat_id: &str) {
        let removed = {
            let mut slots = self.slots();
            slots.chats.remove(&Key {
                project_id: project_id.to_string(),
                chat_id: chat_id.to_string(),
            })
        };
        if let Some(Entry::Starting(starting)) = removed {
            starting.abandoned.store(true, Ordering::SeqCst);
            starting.cancel.store(true, Ordering::SeqCst);
            let _ = starting.done.send(true);
        }
    }

    /// Live history when a chat is running, otherwise the transcript file.
    pub fn history(&self, project_id: &str, chat_id: &str) -> Vec<UiChunk> {
        if let Some(live) = self.live(project_id, chat_id) {
            let history = live.history.lock().unwrap_or_else(|e| e.into_inner());
            return history.clone();
        }
        transcript::load(&self.inner.state_dir, project_id, chat_id)
    }

    /// Appends a daemon-authored chunk (a checkpoint) to a chat: the live
    /// history when one exists, otherwise the transcript file.
    pub fn record(&self, project_id: &str, chat_id: &str, chunk: UiChunk) {
        if let Some(live) = self.live(project_id, chat_id) {
            live.record(chunk);
        } else if let Err(e) =
            transcript::append(&self.inner.state_dir, project_id, chat_id, &chunk)
        {
            eprintln!("orso: failed to write chat chunk for {project_id}/{chat_id}: {e}");
        }
    }

    /// Replay plus the live stream. `None` when this chat has no process
    /// (use [`history`] for the file).
    pub fn subscribe(&self, project_id: &str, chat_id: &str) -> Option<Subscription> {
        let live = self.live(project_id, chat_id)?;
        let history = live.history.lock().unwrap_or_else(|e| e.into_inner());
        let rx = live.tx.subscribe();
        Some(Subscription {
            replay: history.clone(),
            rx,
        })
    }

    pub fn is_live(&self, project_id: &str, chat_id: &str) -> bool {
        self.live(project_id, chat_id).is_some()
    }

    /// Any chat for `project_id` is live or still handshaking.
    pub fn any_live(&self, project_id: &str) -> bool {
        self.slots()
            .chats
            .keys()
            .any(|key| key.project_id == project_id)
    }

    /// The hook token a live chat's agent was given (`ORSO_HOOK_TOKEN`).
    /// `None` while the handshake is still running.
    pub fn hook_token(&self, project_id: &str, chat_id: &str) -> Option<String> {
        self.live(project_id, chat_id)
            .map(|live| live.hook_token.clone())
    }

    /// A prompt is in flight in this chat.
    pub fn is_running(&self, project_id: &str, chat_id: &str) -> bool {
        self.live(project_id, chat_id)
            .is_some_and(|live| live.running.load(Ordering::SeqCst))
    }

    /// A prompt is in flight in any chat of this project. Unlike [`Self::any_live`],
    /// an idle agent process (kept up between turns) does not count: it writes nothing,
    /// so git may touch the checkout.
    pub fn any_running(&self, project_id: &str) -> bool {
        self.slots().chats.values().any(|entry| match entry {
            Entry::Live(live) => {
                live.project_id == project_id && live.running.load(Ordering::SeqCst)
            }
            Entry::Starting(_) => false,
        })
    }

    /// True when `token` is a live chat agent's hook token in `project_id`.
    pub fn token_matches(&self, project_id: &str, token: &str) -> bool {
        self.slots().chats.values().any(|entry| match entry {
            Entry::Live(live) => {
                live.project_id == project_id && crate::auth::ct_eq(token, &live.hook_token)
            }
            Entry::Starting(_) => false,
        })
    }

    /// Cancels and drops every chat agent for one project (project delete).
    ///
    /// The project is marked stopping before the first await. `start` fails
    /// until that mark is cleared, which happens under the map lock only when
    /// the project has no entries left. A start that arrives during `cancel`
    /// is either rejected or taken by the next pass.
    pub async fn stop_project(&self, project_id: &str) {
        self.slots().stopping.insert(project_id.to_string());
        loop {
            let batch: Vec<Entry> = {
                let mut slots = self.slots();
                let keys: Vec<Key> = slots
                    .chats
                    .keys()
                    .filter(|key| key.project_id == project_id)
                    .cloned()
                    .collect();
                if keys.is_empty() {
                    slots.stopping.remove(project_id);
                    return;
                }
                keys.into_iter()
                    .filter_map(|key| slots.chats.remove(&key))
                    .collect()
            };
            self.pause_stop_for_test().await;
            for entry in batch {
                self.teardown_entry(entry).await;
            }
        }
    }

    /// Cancels and drops every chat. In-flight handshakes do not publish.
    pub async fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
        let entries: Vec<(Key, Entry)> = {
            let mut slots = self.slots();
            slots.chats.drain().collect()
        };
        for (key, entry) in entries {
            self.teardown_entry(entry).await;
            let _ = key;
        }
    }

    async fn teardown_entry(&self, entry: Entry) {
        match entry {
            Entry::Live(live) => {
                live.cancel.store(true, Ordering::SeqCst);
                let _ = live.client.cancel(&live.acp_session_id).await;
                drop(live);
            }
            Entry::Starting(starting) => {
                starting.cancel.store(true, Ordering::SeqCst);
                let _ = starting.done.send(true);
            }
        }
    }

    /// Pauses once inside `stop_project` when a test has armed the gate.
    async fn pause_stop_for_test(&self) {
        #[cfg(test)]
        {
            let gate = self
                .inner
                .stop_gate
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            if let Some(gate) = gate {
                gate.entered.notify_one();
                gate.release.notified().await;
            }
        }
        #[cfg(not(test))]
        {
            let _ = self;
        }
    }

    fn live(&self, project_id: &str, chat_id: &str) -> Option<Arc<LiveChat>> {
        self.slots()
            .chats
            .get(&Key {
                project_id: project_id.to_string(),
                chat_id: chat_id.to_string(),
            })
            .and_then(|entry| match entry {
                Entry::Live(live) => Some(live.clone()),
                Entry::Starting(_) => None,
            })
    }
}

/// Test seam for two cold `POST …/message` calls. Both block inside
/// `set_model` until [`ModelGate::release_first`] / [`ModelGate::release_rest`].
struct ArmedModelGate {
    project_id: String,
    chat_id: String,
    entered: Arc<AtomicUsize>,
    first: Arc<Notify>,
    rest: Arc<Notify>,
}

static MODEL_GATE: Mutex<Option<ArmedModelGate>> = Mutex::new(None);

pub struct ModelGate {
    entered: Arc<AtomicUsize>,
    first: Arc<Notify>,
    rest: Arc<Notify>,
}

impl ModelGate {
    pub fn entered(&self) -> usize {
        self.entered.load(Ordering::SeqCst)
    }

    pub fn release_first(&self) {
        self.first.notify_one();
    }

    pub fn release_rest(&self) {
        self.rest.notify_waiters();
    }
}

impl Drop for ModelGate {
    fn drop(&mut self) {
        *MODEL_GATE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.first.notify_waiters();
        self.rest.notify_waiters();
    }
}

/// Arms the gate for one chat. A later arm replaces it. Dropping the handle
/// releases anyone still waiting.
pub fn arm_model_gate(project_id: impl Into<String>, chat_id: impl Into<String>) -> ModelGate {
    let entered = Arc::new(AtomicUsize::new(0));
    let first = Arc::new(Notify::new());
    let rest = Arc::new(Notify::new());
    *MODEL_GATE.lock().unwrap_or_else(|e| e.into_inner()) = Some(ArmedModelGate {
        project_id: project_id.into(),
        chat_id: chat_id.into(),
        entered: entered.clone(),
        first: first.clone(),
        rest: rest.clone(),
    });
    ModelGate {
        entered,
        first,
        rest,
    }
}

async fn wait_model_gate(project_id: &str, chat_id: &str) {
    let wait = {
        let guard = MODEL_GATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(gate) = guard.as_ref() else {
            return;
        };
        if gate.project_id != project_id || gate.chat_id != chat_id {
            return;
        }
        let n = gate.entered.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            gate.first.clone()
        } else {
            gate.rest.clone()
        }
    };
    wait.notified().await;
}

fn transcript_turn_open(chunks: &[UiChunk]) -> bool {
    let mut open = false;
    for chunk in chunks {
        match chunk {
            UiChunk::UserText { .. } => open = true,
            UiChunk::TurnEnd { .. } | UiChunk::Error { .. } => open = false,
            _ => {}
        }
    }
    open
}

fn agent_gone(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("acp agent exited")
        || lower.contains("broken pipe")
        || lower.contains("os error 32")
}

fn prompt_busy(err: &str) -> bool {
    err.to_ascii_lowercase()
        .contains("already has an active acp prompt")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv() -> Vec<String> {
        let (cmd, args) = crate::chat::acp::fake_agent();
        let mut all = vec![cmd];
        all.extend(args);
        all
    }

    fn manager() -> (ChatManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let chats = ChatManager::new(EventBus::new(), dir.path().to_path_buf());
        (chats, dir)
    }

    async fn wait_until(
        chats: &ChatManager,
        project: &str,
        chat: &str,
        mut pred: impl FnMut(&[UiChunk]) -> bool,
    ) -> Vec<UiChunk> {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            let history = chats.history(project, chat);
            if pred(&history) {
                return history;
            }
            if std::time::Instant::now() >= deadline {
                panic!("timed out waiting for chat history: {history:?}");
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    }

    fn pid_of(chunks: &[UiChunk]) -> Option<String> {
        chunks.iter().find_map(|chunk| match chunk {
            UiChunk::TextDelta { text, .. } => text
                .strip_prefix("pid:")
                .map(|pid| pid.trim().to_string())
                .filter(|pid| !pid.is_empty()),
            _ => None,
        })
    }

    #[tokio::test]
    async fn two_chats_same_project_are_both_live() {
        let (chats, _dir) = manager();
        let cwd = std::env::temp_dir();
        let argv = argv();
        let (a, b) = tokio::join!(
            chats.start("proj", "chat-a", &cwd, &argv, &[]),
            chats.start("proj", "chat-b", &cwd, &argv, &[]),
        );
        a.unwrap();
        b.unwrap();
        assert!(chats.is_live("proj", "chat-a"));
        assert!(chats.is_live("proj", "chat-b"));
        assert!(chats.any_live("proj"));

        let (left, right) = tokio::join!(
            chats.send_message("proj", "chat-a", "show-pid one"),
            chats.send_message("proj", "chat-b", "show-pid two"),
        );
        left.unwrap();
        right.unwrap();
        let hist_a = wait_until(&chats, "proj", "chat-a", |h| pid_of(h).is_some()).await;
        let hist_b = wait_until(&chats, "proj", "chat-b", |h| pid_of(h).is_some()).await;
        let pid_a = pid_of(&hist_a).unwrap();
        let pid_b = pid_of(&hist_b).unwrap();
        assert_ne!(pid_a, pid_b, "the two chats shared one ACP process");
        chats.shutdown().await;
    }

    #[tokio::test]
    async fn send_message_streams_text_delta_then_turn_end() {
        let (chats, _dir) = manager();
        let cwd = std::env::temp_dir();
        let argv = argv();
        chats
            .start("proj", "chat-1", &cwd, &argv, &[])
            .await
            .unwrap();
        chats.send_message("proj", "chat-1", "hi").await.unwrap();
        let history = wait_until(&chats, "proj", "chat-1", |h| {
            let delta = h.iter().any(
                |c| matches!(c, UiChunk::TextDelta { text, .. } if text.contains("fake reply: hi")),
            );
            let end = h.iter().any(
                |c| matches!(c, UiChunk::TurnEnd { stop_reason, .. } if stop_reason == "end_turn"),
            );
            delta && end
        })
        .await;
        let delta_at = history.iter().position(
            |c| matches!(c, UiChunk::TextDelta { text, .. } if text.contains("fake reply: hi")),
        );
        let end_at = history.iter().position(
            |c| matches!(c, UiChunk::TurnEnd { stop_reason, .. } if stop_reason == "end_turn"),
        );
        assert!(delta_at.unwrap() < end_at.unwrap(), "{history:?}");
        // The process stays up, but nothing is running until the next prompt.
        assert!(chats.any_live("proj"));
        assert!(!chats.any_running("proj"));
        chats.shutdown().await;
    }

    #[tokio::test]
    async fn cancel_ends_turn() {
        let (chats, _dir) = manager();
        let cwd = std::env::temp_dir();
        let argv = argv();
        chats
            .start("proj", "chat-1", &cwd, &argv, &[])
            .await
            .unwrap();
        chats
            .send_message("proj", "chat-1", "slow please")
            .await
            .unwrap();
        wait_until(&chats, "proj", "chat-1", |h| {
            h.iter().any(
                |c| matches!(c, UiChunk::TextDelta { text, .. } if text.contains("slow started")),
            )
        })
        .await;
        chats.cancel("proj", "chat-1").await.unwrap();
        let history = wait_until(&chats, "proj", "chat-1", |h| {
            h.iter().any(
                |c| matches!(c, UiChunk::TurnEnd { stop_reason, .. } if stop_reason == "cancelled"),
            )
        })
        .await;
        assert!(
            history.iter().any(|c| {
                matches!(c, UiChunk::TextDelta { text, .. } if text.contains("slow started"))
            }),
            "{history:?}"
        );
        chats.shutdown().await;
    }

    #[tokio::test]
    async fn missing_binary_notes_error_chunk() {
        let (chats, _dir) = manager();
        let cwd = std::env::temp_dir();
        let err = chats
            .start(
                "proj",
                "chat-1",
                &cwd,
                &["/nonexistent-orso-acp-bin".into()],
                &[],
            )
            .await
            .unwrap_err();
        assert!(!chats.is_live("proj", "chat-1"), "{err}");
        assert!(!chats.any_live("proj"), "{err}");
        let history = chats.history("proj", "chat-1");
        assert!(
            matches!(history.last(), Some(UiChunk::Error { message }) if message.contains("nonexistent-orso-acp-bin")),
            "{err}; {history:?}"
        );
    }

    fn pid_alive(pid: &str) -> bool {
        let Ok(pid) = pid.parse::<i32>() else {
            return false;
        };
        let rc = unsafe { libc::kill(pid, 0) };
        if rc == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }

    /// A second send used to detach the first prompt task. `forget` then
    /// aborted only the latest handle, and the detached task kept the
    /// `AcpClient` — and the child — alive.
    #[tokio::test]
    async fn forget_reaps_child_when_a_previous_prompt_is_still_running() {
        let (chats, _dir) = manager();
        let cwd = std::env::temp_dir();
        let argv = argv();
        chats
            .start("proj", "chat-1", &cwd, &argv, &[])
            .await
            .unwrap();
        chats
            .send_message("proj", "chat-1", "show-pid")
            .await
            .unwrap();
        let hist = wait_until(&chats, "proj", "chat-1", |h| pid_of(h).is_some()).await;
        let pid = pid_of(&hist).unwrap();
        chats.send_message("proj", "chat-1", "stuck").await.unwrap();
        wait_until(&chats, "proj", "chat-1", |h| {
            h.iter().any(
                |c| matches!(c, UiChunk::TextDelta { text, .. } if text.contains("stuck started")),
            )
        })
        .await;
        chats
            .send_message("proj", "chat-1", "second")
            .await
            .unwrap();
        chats.forget("proj", "chat-1");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while pid_alive(&pid) {
            assert!(
                std::time::Instant::now() < deadline,
                "pid {pid} still alive after forget"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Delete/forget while `session/new` is still in flight must drop the
    /// reservation. The handshake must not publish afterwards, or terminal
    /// start stays blocked (`any_live`) with no record left to stop.
    #[tokio::test]
    async fn forget_during_handshake_does_not_publish() {
        let (chats, dir) = manager();
        let gate = dir.path().join("hold");
        let (cmd, args) = crate::chat::acp::fake_agent_hold(&gate);
        let mut argv = vec![cmd];
        argv.extend(args);
        let cwd = std::env::temp_dir();
        let started = {
            let chats = chats.clone();
            tokio::spawn(async move { chats.start("proj", "chat-1", &cwd, &argv, &[]).await })
        };
        let ready = format!("{}.ready", gate.display());
        let go = format!("{}.go", gate.display());
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !std::path::Path::new(&ready).exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "fake agent never reached the hold"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            chats.any_live("proj"),
            "handshake should count as live for terminal"
        );
        assert!(!chats.is_live("proj", "chat-1"));
        chats.forget("proj", "chat-1");
        assert!(
            !chats.any_live("proj"),
            "forget left the handshake reservation in place"
        );
        std::fs::write(&go, b"").unwrap();
        let err = started.await.unwrap().unwrap_err();
        assert!(
            !chats.is_live("proj", "chat-1") && !chats.any_live("proj"),
            "published after forget: {err}"
        );
    }

    /// `stop_project` used to snapshot keys and then await `cancel`. A start
    /// in that window was not in the snapshot and stayed live.
    #[tokio::test]
    async fn stop_project_rejects_a_start_that_arrives_while_stopping() {
        let (chats, _dir) = manager();
        let cwd = std::env::temp_dir();
        let argv = argv();
        chats
            .start("proj", "chat-a", &cwd, &argv, &[])
            .await
            .unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        *chats.inner.stop_gate.lock().unwrap() = Some(StopGate {
            entered: entered.clone(),
            release: release.clone(),
        });
        let stopping = {
            let chats = chats.clone();
            tokio::spawn(async move { chats.stop_project("proj").await })
        };
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .expect("stop_project did not reach its cancel window");
        let err = chats
            .start("proj", "sneak", &cwd, &argv, &[])
            .await
            .unwrap_err();
        assert!(
            matches!(err, ChatError::Failed(_)),
            "start during stop_project should fail, got {err}"
        );
        release.notify_one();
        tokio::time::timeout(Duration::from_secs(5), stopping)
            .await
            .expect("stop_project hung")
            .unwrap();
        assert!(!chats.any_live("proj"));
        assert!(!chats.is_live("proj", "sneak"));
        chats
            .start("proj", "after", &cwd, &argv, &[])
            .await
            .expect("start after stop_project should work");
        assert!(chats.is_live("proj", "after"));
        chats.shutdown().await;
    }

    /// The cancel latch records `TurnEnd` without an ACP response. That path
    /// has to emit the same hook and `chat.turn` as `PromptEnded`.
    #[tokio::test]
    async fn cancel_latch_emits_turn_end_hook_and_event() {
        let (chats, _dir) = manager();
        let cwd = std::env::temp_dir();
        let argv = argv();
        chats
            .start("proj", "chat-1", &cwd, &argv, &[])
            .await
            .unwrap();
        let reasons = Arc::new(Mutex::new(Vec::new()));
        let reasons_hook = reasons.clone();
        chats.set_on_turn_end(move |ended| {
            reasons_hook
                .lock()
                .unwrap()
                .push((ended.chat_id, ended.stop_reason));
        });
        let mut events = chats.inner.events.subscribe();
        let live = chats.live("proj", "chat-1").unwrap();
        live.cancel.store(true, Ordering::SeqCst);
        chats.dispatch(&live, "nope");
        assert_eq!(
            reasons.lock().unwrap().as_slice(),
            [("chat-1".to_string(), "cancelled".to_string())]
        );
        assert!(chats.history("proj", "chat-1").iter().any(|c| {
            matches!(c, UiChunk::TurnEnd { stop_reason, .. } if stop_reason == "cancelled")
        }));
        let mut saw = false;
        while let Ok(frame) = events.try_recv() {
            if frame.contains("\"type\":\"chat.turn\"") && frame.contains("chat-1") {
                saw = true;
            }
        }
        assert!(saw, "cancel latch did not emit chat.turn");
        chats.shutdown().await;
    }
}
