//! Agent endpoints: providers, session lifecycle, prompts, permissions, history.

use axum::body::Bytes;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use super::query_or_badreq;
use crate::acp::Decision;
use crate::error::ApiError;
use crate::state::AppState;

fn parse_body<T: DeserializeOwned>(body: &Bytes, shape: &str) -> Result<T, ApiError> {
    serde_json::from_slice(body)
        .map_err(|e| ApiError::badreq(format!("expected JSON body {shape}: {e}")))
}

/// `GET /api/agents` → `{ providers: [{ id, command }] }`.
pub async fn providers(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({ "providers": state.agents.providers()? })))
}

#[derive(Deserialize)]
struct SessionRequest {
    project: String,
    provider: String,
}

/// `POST /api/agent/session` `{ project, provider }` → `{ sessionId }`.
pub async fn start_session(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: SessionRequest = parse_body(&body, "{\"project\": string, \"provider\": string}")?;
    let session_id = state
        .agents
        .start_session(&req.project, &req.provider)
        .await?;
    Ok(Json(json!({ "sessionId": session_id })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptRequest {
    project: String,
    session_id: String,
    text: String,
}

/// `POST /api/agent/prompt` `{ project, sessionId, text }` → `{ turnId }`.
pub async fn prompt(State(state): State<AppState>, body: Bytes) -> Result<Json<Value>, ApiError> {
    let req: PromptRequest = parse_body(
        &body,
        "{\"project\": string, \"sessionId\": string, \"text\": string}",
    )?;
    super::checkpoints::require_change(&state, &req.project).await?;
    let turn_id = state
        .agents
        .prompt(&req.project, &req.session_id, &req.text)
        .await?;
    Ok(Json(json!({ "turnId": turn_id })))
}

/// `POST /api/agent/prompt/{turnId}/cancel`.
pub async fn cancel(
    State(state): State<AppState>,
    Path(turn_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.agents.cancel(&turn_id)?;
    Ok(Json(json!({ "cancelled": turn_id })))
}

#[derive(Deserialize)]
struct PermissionRequest {
    decision: Decision,
}

/// `POST /api/agent/permission/{requestId}` `{ decision: "allow"|"deny" }`.
pub async fn permission(
    State(state): State<AppState>,
    Path(request_id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: PermissionRequest = parse_body(&body, "{\"decision\": \"allow\"|\"deny\"}")?;
    state.agents.resolve_permission(&request_id, req.decision)?;
    Ok(Json(json!({ "resolved": request_id })))
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    project: String,
    session: Option<String>,
}

/// `GET /api/agent/history?project=&session=?` → `{ turns: [...] }`.
pub async fn history(
    State(state): State<AppState>,
    q: Result<Query<HistoryQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let turns = state
        .agents
        .history(&q.project, q.session.as_deref())
        .await?;
    Ok(Json(json!({ "turns": turns })))
}
