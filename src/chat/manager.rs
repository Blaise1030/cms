//! One long-lived ACP agent per `(project_id, chat_id)`.
//!
//! Two chats in the same project are two processes. There is no PTY claim:
//! the terminal module still asks [`ChatManager::any_live`] so a project
//! delete / terminal start can see chat agents until the terminal goes away.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use serde_json::json;
use tokio::sync::{broadcast, watch};

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

/// Fired after a `turn_end` chunk is recorded. Task 6 attaches the checkpoint hook.
#[derive(Debug, Clone)]
pub struct TurnEnded {
    pub project_id: String,
    pub chat_id: String,
    pub session_id: String,
    pub stop_reason: String,
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
    /// In-flight `session/prompt`. Aborted when this chat is dropped so the
    /// task releases its `AcpClient` and `kill_on_drop` can reap the process.
    prompt: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Drop for LiveChat {
    fn drop(&mut self) {
        if let Some(task) = self
            .prompt
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
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

/// A start in flight. Its handshake runs outside the map lock; waiters park
/// on `done` until the live chat (or the failure) is published.
struct Starting {
    done: watch::Sender<bool>,
    cancel: Arc<AtomicBool>,
}

enum Entry {
    Live(Arc<LiveChat>),
    Starting(Arc<Starting>),
}

struct Inner {
    chats: Mutex<HashMap<Key, Entry>>,
    state_dir: PathBuf,
    events: EventBus,
    on_turn_end: Mutex<Option<TurnEndHook>>,
    /// Set by [`ChatManager::shutdown`]. A handshake that finishes after
    /// this does not publish a live chat.
    shutdown: AtomicBool,
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
    pub fn new(events: EventBus, state_dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                chats: Mutex::new(HashMap::new()),
                state_dir,
                events,
                on_turn_end: Mutex::new(None),
                shutdown: AtomicBool::new(false),
            }),
        }
    }

    /// Checkpoint / content refresh (Task 6) runs after each `turn_end` chunk.
    /// The hook is synchronous and should not block the event pump for long.
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
                let mut chats = self.inner.chats.lock().unwrap_or_else(|e| e.into_inner());
                match chats.get(&key) {
                    Some(Entry::Live(live)) => Action::Existing(live.clone()),
                    Some(Entry::Starting(s)) => Action::Wait(s.clone()),
                    None => {
                        let s = Arc::new(Starting {
                            done: watch::channel(false).0,
                            cancel: Arc::new(AtomicBool::new(false)),
                        });
                        chats.insert(key.clone(), Entry::Starting(s.clone()));
                        Action::Reserve(s)
                    }
                }
            };
            match action {
                Action::Existing(live) => return Ok(live),
                Action::Wait(s) => {
                    let mut rx = s.done.subscribe();
                    let _ = rx.wait_for(|done| *done).await;
                }
                Action::Reserve(s) => break s,
            }
        };

        let handshake = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            let (cmd, args) = split_argv(acp_argv)?;
            let client = AcpClient::spawn_with_env(cmd, args, cwd, env)
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
            Ok::<_, ChatError>((client, created))
        })
        .await
        .map_err(|_| ChatError::Failed("ACP agent handshake timed out".into()))
        .and_then(|r| r);

        let (client, created) = match handshake {
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
            tx: broadcast::channel(BROADCAST_CAPACITY).0,
            history: Mutex::new(transcript::load(&self.inner.state_dir, project_id, chat_id)),
            models: Mutex::new(created.models),
            state_dir: self.inner.state_dir.clone(),
            project_id: project_id.to_string(),
            chat_id: chat_id.to_string(),
            cwd: cwd.to_path_buf(),
            cancel: starting.cancel.clone(),
            exited: AtomicBool::new(false),
            prompt: Mutex::new(None),
        });

        let publish = {
            let mut chats = self.inner.chats.lock().unwrap_or_else(|e| e.into_inner());
            let still_ours =
                matches!(chats.get(&key), Some(Entry::Starting(s)) if Arc::ptr_eq(s, &starting));
            if self.inner.shutdown.load(Ordering::SeqCst) || !still_ours {
                false
            } else {
                if live.cancel.load(Ordering::SeqCst) {
                    live.client.mark_cancelled();
                }
                chats.insert(key.clone(), Entry::Live(live.clone()));
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
        let mut chats = self.inner.chats.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(chats.get(key), Some(Entry::Starting(s)) if Arc::ptr_eq(s, starting)) {
            chats.remove(key);
        }
        drop(chats);
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
        if let Some((session_id, stop_reason)) = turn_end {
            let ended = TurnEnded {
                project_id: live.project_id.clone(),
                chat_id: live.chat_id.clone(),
                session_id,
                stop_reason,
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
    }

    /// Records `text` as the user's chunk and runs it as an ACP prompt.
    /// The turn's chunks arrive through the event pump.
    fn dispatch(&self, live: &Arc<LiveChat>, text: &str) {
        live.record(UiChunk::UserText {
            session_id: live.acp_session_id.clone(),
            text: text.to_string(),
        });
        if live.cancel.load(Ordering::SeqCst) {
            live.record(UiChunk::TurnEnd {
                session_id: live.acp_session_id.clone(),
                stop_reason: "cancelled".into(),
            });
            return;
        }
        let client = live.client.clone();
        let acp_session_id = live.acp_session_id.clone();
        let text = text.to_string();
        let weak = Arc::downgrade(live);
        let mgr = self.clone();
        let task = tokio::spawn(async move {
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
        *live.prompt.lock().unwrap_or_else(|e| e.into_inner()) = Some(task);
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
            let chats = self.inner.chats.lock().unwrap_or_else(|e| e.into_inner());
            match chats.get(&Key {
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
        let live = self.live(project_id, chat_id).ok_or(ChatError::NotLive)?;
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

    /// Drops the live chat, if any. The agent process dies with its last
    /// client reference (`kill_on_drop`). A start still handshaking is left
    /// to publish or clear its own reservation.
    pub fn forget(&self, project_id: &str, chat_id: &str) {
        let mut chats = self.inner.chats.lock().unwrap_or_else(|e| e.into_inner());
        let key = Key {
            project_id: project_id.to_string(),
            chat_id: chat_id.to_string(),
        };
        if matches!(chats.get(&key), Some(Entry::Live(_))) {
            chats.remove(&key);
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
        self.inner
            .chats
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .any(|key| key.project_id == project_id)
    }

    /// Cancels and drops every chat agent for one project (project delete).
    pub async fn stop_project(&self, project_id: &str) {
        let keys: Vec<Key> = self
            .inner
            .chats
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .filter(|key| key.project_id == project_id)
            .cloned()
            .collect();
        for key in keys {
            let _ = self.cancel(&key.project_id, &key.chat_id).await;
            self.inner
                .chats
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&key);
        }
    }

    /// Cancels and drops every chat. In-flight handshakes do not publish.
    pub async fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
        let entries: Vec<(Key, Entry)> = self
            .inner
            .chats
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain()
            .collect();
        for (key, entry) in entries {
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
            let _ = key;
        }
    }

    fn live(&self, project_id: &str, chat_id: &str) -> Option<Arc<LiveChat>> {
        self.inner
            .chats
            .lock()
            .unwrap_or_else(|e| e.into_inner())
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
}
