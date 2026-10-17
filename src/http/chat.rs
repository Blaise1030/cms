//! Chat panel HTTP: headless turns streamed as AI SDK UI message streams (SSE).

use std::convert::Infallible;
use std::path::PathBuf;

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::{stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast::error::RecvError;

use super::checkpoints;
use super::terminal::new_agent_session;
use crate::chat::chunk::{sse_frame, SSE_DONE};
use crate::chat::manager::{Subscription, TurnRequest};
use crate::chat::transcript;
use crate::chat::translate::is_chat_provider;
use crate::error::ApiError;
use crate::projects::{Project, STATE_DIR};
use crate::providers::{self, AgentSession};
use crate::state::AppState;
use crate::terminal::history::{self, SessionRecord};

const CHAT_PROVIDERS_ONLY: &str = "Chat supports Cursor CLI, Claude Code and Codex";

fn state_dir(state: &AppState) -> PathBuf {
    state.config.projects_dir.join(STATE_DIR)
}

async fn project(state: &AppState, id: &str) -> Result<Project, ApiError> {
    state
        .projects
        .get(id)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {id}")))
}

fn check_chat_id(id: &str) -> Result<(), ApiError> {
    if transcript::valid_chat_id(id) {
        Ok(())
    } else {
        Err(ApiError::badreq(format!("invalid chat id: {id}")))
    }
}

/// What `DefaultChatTransport` sends (`provider` through the transport's `body`).
#[derive(Deserialize)]
struct PostRequest {
    id: String,
    messages: Vec<Value>,
    #[serde(default)]
    provider: Option<String>,
}

/// Text parts of the last user message, joined by newlines.
fn prompt(messages: &[Value]) -> String {
    let Some(user) = messages.iter().rev().find(|m| m["role"] == "user") else {
        return String::new();
    };
    user["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["type"] == "text")
        .filter_map(|p| p["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `POST /api/chat/{project}` `{ id, messages, provider? }` → the turn's UI message stream.
/// An unknown `id` starts a new chat with that id (`provider` required).
pub async fn post(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let req: PostRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{id, messages, provider?}}: {e}"
        ))
    })?;
    check_chat_id(&req.id)?;
    let project = project(&state, &project_id).await?;
    let state_dir = state_dir(&state);
    let existing = history::get(&state_dir, &project_id, &req.id);
    let provider_id = match &existing {
        Some(record) => record.provider.clone(),
        None => req
            .provider
            .clone()
            .ok_or_else(|| ApiError::badreq("provider is required for a new chat"))?,
    };
    if !is_chat_provider(&provider_id) {
        return Err(ApiError::badreq(CHAT_PROVIDERS_ONLY));
    }
    let prompt = prompt(&req.messages);
    if prompt.trim().is_empty() {
        return Err(ApiError::badreq("the last user message has no text"));
    }
    let provider = providers::load(state.config.agents_config.as_deref())?
        .into_iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| ApiError::badreq(format!("unknown provider: {provider_id}")))?;
    // Checked again, atomically, by `ChatManager::start`; here so a refused turn leaves no record.
    if state.terminals.status(&project_id).is_some() {
        return Err(ApiError::conflict("The agent is busy in Terminal"));
    }
    if state.chats.running(&project_id).is_some() {
        return Err(ApiError::conflict("The agent is busy in Chat"));
    }

    // Agents given `GIT_GUIDE` start changes themselves; other agents need one first.
    let mut guided = providers::gets_git_guide(&provider.id);
    if provider.id == "cursor" {
        if let Err(e) = providers::write_cursor_rule(&project.dir) {
            eprintln!("workbench: cannot write the Cursor git guide rule: {e}");
            guided = false;
        }
    }
    if !guided {
        checkpoints::require_change(&state, &project_id).await?;
    }

    let session = match existing {
        Some(record) => record
            .agent_session_id
            .map_or(AgentSession::Fresh, AgentSession::Resume),
        None => {
            let session = new_agent_session(&provider, &project.dir).await;
            let now = history::now_ms();
            let record = SessionRecord {
                id: req.id.clone(),
                provider: provider.id.clone(),
                agent_session_id: match &session {
                    AgentSession::Assign(id) => Some(id.clone()),
                    _ => None,
                },
                title: None,
                created_at: now,
                updated_at: now,
            };
            history::insert(&state_dir, &project_id, record)
                .map_err(|e| ApiError::io(format!("cannot save the chat: {e}")))?;
            session
        }
    };
    let sub = state.chats.start(
        state.clone(),
        TurnRequest {
            project_id,
            chat_id: req.id,
            provider,
            dir: project.dir,
            prompt,
            session,
        },
    )?;
    Ok(sse(sub))
}

/// Replay, then live chunks, then `[DONE]` once the turn is over. A lagging subscriber's body
/// ends without `[DONE]` so the client resumes through `…/stream`. A client going away leaves
/// the turn running.
fn sse(sub: Subscription) -> Response {
    let Subscription { replay, rx } = sub;
    let replay = stream::iter(replay).map(|chunk| sse_frame(&chunk));
    let live = stream::unfold(Some(rx), |rx| async move {
        let mut rx = rx?;
        match rx.recv().await {
            Ok(Some(chunk)) => Some((sse_frame(&chunk), Some(rx))),
            Ok(None) => Some((SSE_DONE.to_string(), None)),
            Err(RecvError::Lagged(_) | RecvError::Closed) => None,
        }
    });
    let body = Body::from_stream(replay.chain(live).map(Ok::<_, Infallible>));
    (
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
            (
                header::HeaderName::from_static("x-vercel-ai-ui-message-stream"),
                "v1",
            ),
            (header::HeaderName::from_static("x-accel-buffering"), "no"),
        ],
        body,
    )
        .into_response()
}

/// `GET /api/chat/{project}` → `{ chats: [{ id, provider, title?, updatedAt }], running }`,
/// most recently active first. Only records with a transcript are chats.
pub async fn list(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    project(&state, &project_id).await?;
    let state_dir = state_dir(&state);
    let mut records: Vec<SessionRecord> = history::list(&state_dir, &project_id)
        .into_iter()
        .filter(|r| {
            transcript::valid_chat_id(&r.id)
                && transcript::path(&state_dir, &project_id, &r.id).is_file()
        })
        .collect();
    records.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
    let chats: Vec<Value> = records
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "provider": r.provider,
                "title": r.title,
                "updatedAt": r.updated_at,
            })
        })
        .collect();
    Ok(Json(json!({
        "chats": chats,
        "running": state.chats.running(&project_id),
    })))
}

/// `GET /api/chat/{project}/{chat}` → `{ messages, provider, running }`.
pub async fn get(
    State(state): State<AppState>,
    Path((project_id, chat_id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    check_chat_id(&chat_id)?;
    project(&state, &project_id).await?;
    let state_dir = state_dir(&state);
    let record = history::get(&state_dir, &project_id, &chat_id)
        .ok_or_else(|| ApiError::enoent(format!("no such chat: {chat_id}")))?;
    let running = state.chats.running(&project_id).as_deref() == Some(chat_id.as_str());
    Ok(Json(json!({
        "messages": transcript::read(&state_dir, &project_id, &chat_id),
        "provider": record.provider,
        "running": running,
    })))
}

/// `GET /api/chat/{project}/{chat}/stream` → the running turn's stream, or `204`.
pub async fn resume(
    State(state): State<AppState>,
    Path((project_id, chat_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    check_chat_id(&chat_id)?;
    project(&state, &project_id).await?;
    Ok(match state.chats.subscribe(&project_id, &chat_id) {
        Some(sub) => sse(sub),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}

/// `DELETE /api/chat/{project}/turn` → `{ stopped }`, once the turn has ended.
pub async fn stop(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    project(&state, &project_id).await?;
    let stopped = state.chats.running(&project_id).is_some();
    state.chats.stop(&project_id).await;
    Ok(Json(json!({ "stopped": stopped })))
}
