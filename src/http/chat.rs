//! `/api/chat/*`: one ACP agent per chat, tabsh-shaped `UiChunk` SSE.
//!
//! Chat ids are `^[A-Za-z0-9_-]{1,64}$`. `turn` is an ordinary id. The stream
//! replays recorded chunks and stays open; it does not send an AI SDK `[DONE]`
//! trailer. ACP argv comes from the provider registry (`agents.json` or the
//! built-in defaults), with the test/env override first.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::path::PathBuf;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::stream::Stream;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::chat::chunk::{sse_frame, UiChunk};
use crate::chat::history::{self, SessionRecord};
use crate::chat::manager::ChatError;
use crate::chat::models;
use crate::chat::transcript;
use crate::config::random_token;
use crate::error::ApiError;
use crate::projects::STATE_DIR;
use crate::providers;
use crate::state::AppState;

const TITLE_MAX: usize = 72;

fn state_dir(state: &AppState) -> PathBuf {
    state.config.projects_dir.join(STATE_DIR)
}

fn chat_error(err: ChatError) -> ApiError {
    match err {
        ChatError::Invalid(message) => ApiError::badreq(message),
        ChatError::NotLive => ApiError::conflict("no live chat for this session"),
        ChatError::Failed(message) => ApiError::agent(message),
    }
}

fn require_chat_id(id: &str) -> Result<(), ApiError> {
    if transcript::valid_chat_id(id) {
        Ok(())
    } else {
        Err(ApiError::badreq(format!("invalid chat id: {id}")))
    }
}

fn require_record(state: &AppState, project: &str, chat: &str) -> Result<SessionRecord, ApiError> {
    history::get(&state_dir(state), project, chat)
        .ok_or_else(|| ApiError::enoent(format!("no such chat: {chat}")))
}

fn summary(state: &AppState, project: &str, record: &SessionRecord) -> Value {
    json!({
        "id": record.id,
        "provider": record.provider,
        "model": record.model,
        "title": record.title,
        "updatedAt": record.updated_at,
        "live": state.chats.is_live(project, &record.id),
        "running": state.chats.is_running(project, &record.id),
    })
}

fn chunk_event(chunk: &UiChunk) -> Event {
    let framed = sse_frame(chunk);
    let data = framed
        .strip_prefix("data: ")
        .unwrap_or(&framed)
        .trim_end_matches('\n');
    Event::default().data(data)
}

fn truncate_title(text: &str) -> String {
    let line = text.trim().lines().next().unwrap_or("").trim();
    if line.chars().count() <= TITLE_MAX {
        return line.to_string();
    }
    let mut out: String = line.chars().take(TITLE_MAX - 1).collect();
    out.push('…');
    out
}

fn parse_json<T: for<'de> Deserialize<'de>>(body: &Bytes, what: &str) -> Result<T, ApiError> {
    serde_json::from_slice(body).map_err(|e| ApiError::badreq(format!("expected {what}: {e}")))
}

async fn project_cwd(state: &AppState, project: &str) -> Result<PathBuf, ApiError> {
    super::project_dir(state, project).await
}

fn known_provider(state: &AppState, id: &str) -> Result<bool, ApiError> {
    Ok(acp_argv_for(state, id)?.is_some())
}

/// ACP argv for `provider_id`: the test/env override first, then the provider
/// registry (`agents.json` or the built-in defaults). `None` when the provider
/// has no ACP command.
fn acp_argv_for(state: &AppState, provider_id: &str) -> Result<Option<Vec<String>>, ApiError> {
    if let Some(argv) = providers::acp_argv(provider_id) {
        return Ok(Some(argv));
    }
    Ok(providers::load(state.config.agents_config.as_deref())?
        .into_iter()
        .find(|p| p.id == provider_id)
        .map(|p| p.acp)
        .filter(|argv| !argv.is_empty()))
}

/// Spawn the chat's ACP process with `session/new` (not a terminal `AgentSession`).
async fn ensure_agent(
    state: &AppState,
    project: &str,
    chat: &str,
    cwd: &std::path::Path,
    record: &SessionRecord,
) -> Result<(), ApiError> {
    let argv = acp_argv_for(state, &record.provider)?.ok_or_else(|| {
        ApiError::badreq(format!("provider {} has no ACP command", record.provider))
    })?;
    let env = agent_env(state, project);
    let env: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    state
        .chats
        .start(project, chat, cwd, &argv, &env)
        .await
        .map_err(chat_error)?;
    if let Some(model) = record.model.clone() {
        state
            .chats
            .set_model(project, chat, &model)
            .await
            .map_err(chat_error)?;
    }
    Ok(())
}

/// The agent-facing environment a chat process gets: `orso ctl` reaches the
/// daemon with the chat's hook token (added by [`ChatManager::start`]), the git
/// shim leads `PATH`, and `ORSO_REAL_GIT` names the real git.
pub fn agent_env(state: &AppState, project: &str) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = vec![
        (
            "ORSO_URL".into(),
            format!("http://127.0.0.1:{}", state.config.port),
        ),
        ("ORSO_PROJECT".into(), project.to_string()),
        (
            "ORSO_HOOK_EXE".into(),
            state.config.hook_exe_path().to_string_lossy().into_owned(),
        ),
        (
            "ORSO_REAL_GIT".into(),
            crate::git_shim::resolve_system_git()
                .to_string_lossy()
                .into_owned(),
        ),
    ];
    if let Some(shim) = crate::git_shim::ensure_shim_dir(&state.config) {
        let own = std::env::var_os("PATH").unwrap_or_default();
        let dirs = std::iter::once(shim).chain(std::env::split_paths(&own));
        if let Ok(path) = std::env::join_paths(dirs) {
            env.push(("PATH".into(), path.to_string_lossy().into_owned()));
        }
    }
    env
}

fn touch(state: &AppState, project: &str, chat: &str, title_from: Option<&str>) {
    let _ = history::update(&state_dir(state), project, chat, |record| {
        if record.title.is_none() {
            if let Some(text) = title_from.map(truncate_title).filter(|t| !t.is_empty()) {
                record.title = Some(text);
            }
        }
        record.updated_at = history::now_ms();
    });
}

#[derive(Deserialize)]
struct CreateBody {
    provider: String,
    #[serde(default)]
    model: Option<String>,
}

/// `GET /api/chat/{project}` → `{ chats: [{ id, provider, model, title, updatedAt, live }] }`.
pub async fn list_chats(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<Value>, ApiError> {
    project_cwd(&state, &project).await?;
    let records = history::list(&state_dir(&state), &project);
    let running = records
        .iter()
        .find(|record| state.chats.is_running(&project, &record.id))
        .map(|record| record.id.clone());
    let chats: Vec<Value> = records
        .iter()
        .map(|record| summary(&state, &project, record))
        .collect();
    Ok(Json(json!({ "chats": chats, "running": running })))
}

/// `POST /api/chat/{project}` `{ provider, model? }` → the new chat record.
pub async fn create_chat(
    State(state): State<AppState>,
    Path(project): Path<String>,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    project_cwd(&state, &project).await?;
    let req: CreateBody = parse_json(&body, "JSON body {provider, model?}")?;
    let provider = req.provider.trim();
    if !transcript::valid_chat_id(provider) {
        return Err(ApiError::badreq(format!("invalid provider: {provider}")));
    }
    if !known_provider(&state, provider)? {
        return Err(ApiError::badreq(format!("unknown provider: {provider}")));
    }
    let model = req
        .model
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty());
    let now = history::now_ms();
    let record = SessionRecord {
        id: random_token()[..16].to_string(),
        provider: provider.to_string(),
        model,
        agent_session_id: None,
        title: None,
        created_at: now,
        updated_at: now,
    };
    history::insert(&state_dir(&state), &project, record.clone())?;
    Ok((
        StatusCode::CREATED,
        Json(summary(&state, &project, &record)),
    ))
}

/// `GET /api/chat/{project}/{chat}` → `{ chunks, provider, model, running }`.
pub async fn get_chat(
    State(state): State<AppState>,
    Path((project, chat)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    project_cwd(&state, &project).await?;
    require_chat_id(&chat)?;
    let record = require_record(&state, &project, &chat)?;
    Ok(Json(json!({
        "chunks": state.chats.history(&project, &chat),
        "provider": record.provider,
        "model": record.model,
        "running": state.chats.is_running(&project, &chat),
    })))
}

/// `DELETE /api/chat/{project}/{chat}` drops the record, the transcript, and the agent.
pub async fn delete_chat(
    State(state): State<AppState>,
    Path((project, chat)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    project_cwd(&state, &project).await?;
    require_chat_id(&chat)?;
    if !history::remove(&state_dir(&state), &project, &chat)? {
        return Err(ApiError::enoent(format!("no such chat: {chat}")));
    }
    state.chats.forget(&project, &chat);
    if let Some(path) = transcript::path(&state_dir(&state), &project, &chat) {
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                eprintln!("orso: cannot remove transcript {}: {e}", path.display());
            }
        }
    }
    Ok(Json(json!({ "deleted": chat })))
}

/// `POST /api/chat/{project}/{chat}/start` — 202 after a new handshake, 204 if already live.
pub async fn start_chat(
    State(state): State<AppState>,
    Path((project, chat)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let cwd = project_cwd(&state, &project).await?;
    require_chat_id(&chat)?;
    let record = require_record(&state, &project, &chat)?;
    if state.chats.is_live(&project, &chat) {
        return Ok(StatusCode::NO_CONTENT);
    }
    super::checkpoints::require_change(&state, &project).await?;
    ensure_agent(&state, &project, &chat, &cwd, &record).await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
struct MessageBody {
    text: String,
}

/// `POST /api/chat/{project}/{chat}/message` `{ text }` — starts the agent when it is down.
pub async fn post_message(
    State(state): State<AppState>,
    Path((project, chat)): Path<(String, String)>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let cwd = project_cwd(&state, &project).await?;
    require_chat_id(&chat)?;
    let req: MessageBody = parse_json(&body, "JSON body {text}")?;
    if req.text.trim().is_empty() {
        return Err(ApiError::badreq("text is required"));
    }
    let record = require_record(&state, &project, &chat)?;
    super::checkpoints::require_change(&state, &project).await?;
    if !state.chats.is_live(&project, &chat) {
        ensure_agent(&state, &project, &chat, &cwd, &record).await?;
    }
    state
        .chats
        .send_message(&project, &chat, &req.text)
        .await
        .map_err(chat_error)?;
    touch(&state, &project, &chat, Some(&req.text));
    Ok(StatusCode::ACCEPTED)
}

/// `POST /api/chat/{project}/{chat}/stop` cancels the current turn.
pub async fn stop_chat(
    State(state): State<AppState>,
    Path((project, chat)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    project_cwd(&state, &project).await?;
    require_chat_id(&chat)?;
    require_record(&state, &project, &chat)?;
    state
        .chats
        .cancel(&project, &chat)
        .await
        .map_err(chat_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ApproveBody {
    #[serde(rename = "requestId")]
    request_id: String,
    allow: bool,
}

/// `POST /api/chat/{project}/{chat}/approve` `{ requestId, allow }`.
pub async fn approve_chat(
    State(state): State<AppState>,
    Path((project, chat)): Path<(String, String)>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    project_cwd(&state, &project).await?;
    require_chat_id(&chat)?;
    require_record(&state, &project, &chat)?;
    let req: ApproveBody = parse_json(&body, "JSON body {requestId, allow}")?;
    state
        .chats
        .answer(&project, &chat, &req.request_id, req.allow)
        .await
        .map_err(chat_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The transcript chunk for a turn-end checkpoint, or `None` when the turn
/// changed nothing (no commit, no block).
pub fn checkpoint_chunk(outcome: &super::checkpoints::TurnOutcome) -> Option<UiChunk> {
    use super::checkpoints::TurnOutcome;
    match outcome {
        TurnOutcome::Committed { checkpoint, files } => Some(UiChunk::Checkpoint {
            sha: Some(checkpoint.sha.clone()),
            summary: Some(checkpoint.summary.clone()),
            files: files.clone(),
            blocked: None,
        }),
        TurnOutcome::Blocked(reason) => Some(UiChunk::Checkpoint {
            sha: None,
            summary: None,
            files: Vec::new(),
            blocked: Some(reason.clone()),
        }),
        TurnOutcome::Unchanged => None,
    }
}

fn models_body(state: &AppState, project: &str, chat: &str) -> Value {
    match state.chats.models(project, chat) {
        Some(cfg) => json!({ "current": cfg.current, "options": cfg.options }),
        None => json!({ "current": null, "options": [] }),
    }
}

/// `GET /api/chat/{project}/{chat}/models` — empty when the agent is not live.
pub async fn list_models(
    State(state): State<AppState>,
    Path((project, chat)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    project_cwd(&state, &project).await?;
    require_chat_id(&chat)?;
    require_record(&state, &project, &chat)?;
    Ok(Json(models_body(&state, &project, &chat)))
}

#[derive(Deserialize)]
struct ModelBody {
    #[serde(rename = "modelId")]
    model_id: String,
}

/// `POST /api/chat/{project}/{chat}/model` `{ modelId }`.
pub async fn set_model(
    State(state): State<AppState>,
    Path((project, chat)): Path<(String, String)>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    project_cwd(&state, &project).await?;
    require_chat_id(&chat)?;
    require_record(&state, &project, &chat)?;
    let req: ModelBody = parse_json(&body, "JSON body {modelId}")?;
    state
        .chats
        .set_model(&project, &chat, &req.model_id)
        .await
        .map_err(chat_error)?;
    let _ = history::update(&state_dir(&state), &project, &chat, |record| {
        record.model = Some(req.model_id.clone());
        record.updated_at = history::now_ms();
    });
    Ok(Json(models_body(&state, &project, &chat)))
}

struct StreamState {
    st: AppState,
    project: String,
    chat: String,
    replay: VecDeque<UiChunk>,
    rx: Option<broadcast::Receiver<UiChunk>>,
    seen: usize,
}

/// `GET /api/chat/{project}/{chat}/stream` — replay, then live chunks, until the client leaves.
pub async fn stream_chat(
    State(state): State<AppState>,
    Path((project, chat)): Path<(String, String)>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    project_cwd(&state, &project).await?;
    require_chat_id(&chat)?;
    require_record(&state, &project, &chat)?;
    let (replay, rx) = match state.chats.subscribe(&project, &chat) {
        Some(sub) => (sub.replay, Some(sub.rx)),
        None => (state.chats.history(&project, &chat), None),
    };
    let stream_state = StreamState {
        st: state,
        project,
        chat,
        replay: replay.into(),
        rx,
        seen: 0,
    };
    let stream = futures_util::stream::unfold(stream_state, |mut state| async move {
        loop {
            if let Some(chunk) = state.replay.pop_front() {
                state.seen += 1;
                return Some((Ok(chunk_event(&chunk)), state));
            }
            match &mut state.rx {
                Some(rx) => match rx.recv().await {
                    Ok(chunk) => {
                        state.seen += 1;
                        return Some((Ok(chunk_event(&chunk)), state));
                    }
                    // R9: a lagged subscriber has missed chunks. End the
                    // stream; the client reconnects and replays history.
                    Err(broadcast::error::RecvError::Lagged(_))
                    | Err(broadcast::error::RecvError::Closed) => return None,
                },
                None => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    if let Some(sub) = state.st.chats.subscribe(&state.project, &state.chat) {
                        state.replay = sub.replay.into_iter().skip(state.seen).collect();
                        state.rx = Some(sub.rx);
                    }
                }
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// `GET /api/chat/providers` → `{ providers: [{ id, command, acp }] }`.
pub async fn list_providers(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let providers = providers::load(state.config.agents_config.as_deref())?;
    Ok(Json(json!({ "providers": providers })))
}

/// `GET /api/chat/providers/{provider}/models` → `{ models: [{ id, name }] }`;
/// empty when the CLI cannot list them.
pub async fn list_provider_models(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let provider = providers::load(state.config.agents_config.as_deref())?
        .into_iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| ApiError::badreq(format!("unknown provider: {provider_id}")))?;
    Ok(Json(json!({ "models": models::list(&provider).await })))
}

/// Largest file a chat accepts as an attachment.
pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;

/// What a chat may attach, with the extension each is saved under.
const ATTACHMENT_TYPES: &[(&str, &str)] = &[
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("application/pdf", "pdf"),
    ("text/plain", "txt"),
    ("text/markdown", "md"),
    ("text/csv", "csv"),
    ("application/json", "json"),
];

/// `POST /api/chat/{project}/attachments` with an image or document body → `{ path }`,
/// an absolute path the agent can read. Kept in the daemon's state dir so checkpoints
/// don't commit it.
pub async fn attach(
    State(state): State<AppState>,
    Path(project): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    state
        .projects
        .get(&project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {project}")))?;
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let Some(&(_, ext)) = ATTACHMENT_TYPES.iter().find(|(m, _)| *m == mime) else {
        return Err(ApiError::badreq(format!(
            "unsupported attachment type: {mime:?}"
        )));
    };
    let dir = state_dir(&state).join(&project).join("attachments");
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("{}.{ext}", &random_token()[..16]));
    tokio::fs::write(&path, &body).await?;
    Ok(Json(json!({ "path": path })))
}

/// `GET /api/chat/{project}/attachments/{name}` → the saved file, so the chat can show it.
/// Only names [`attach`] generates resolve, which keeps the lookup inside the attachments dir.
pub async fn attachment(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    state
        .projects
        .get(&project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {project}")))?;
    let mime = name
        .split_once('.')
        .filter(|(stem, _)| stem.len() == 16 && stem.bytes().all(|b| b.is_ascii_hexdigit()))
        .and_then(|(_, ext)| ATTACHMENT_TYPES.iter().find(|(_, e)| *e == ext))
        .map(|(m, _)| *m)
        .ok_or_else(|| ApiError::badreq(format!("invalid attachment name: {name}")))?;
    let path = state_dir(&state)
        .join(&project)
        .join("attachments")
        .join(&name);
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(ApiError::enoent(format!("no such attachment: {name}")))
        }
        Err(e) => return Err(e.into()),
    };
    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            ),
        ],
        bytes,
    )
        .into_response())
}
