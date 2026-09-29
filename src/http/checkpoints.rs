//! `GET /api/checkpoints`, `GET /api/checkpoints/{sha}/diff`, `POST /api/checkpoints/{sha}/restore`.

use axum::body::Bytes;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{project_dir, query_or_badreq};
use crate::error::ApiError;
use crate::git;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ProjectQuery {
    project: String,
}

/// `GET /api/checkpoints?project=` → `{ checkpoints: [{ sha, summary, ts }] }` (newest first).
pub async fn list(
    State(state): State<AppState>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let dir = project_dir(&state, &q.project).await?;
    Ok(Json(json!({ "checkpoints": git::log(&dir).await? })))
}

/// `GET /api/checkpoints/{sha}/diff?project=` → the commit's unified diff (`text/plain`).
pub async fn diff(
    State(state): State<AppState>,
    Path(sha): Path<String>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let q = query_or_badreq(q)?;
    let dir = project_dir(&state, &q.project).await?;
    let text = git::diff(&dir, &sha).await?;
    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response())
}

#[derive(Deserialize)]
struct RestoreRequest {
    project: String,
}

/// `POST /api/checkpoints/{sha}/restore` `{ project }` → `{ restored: <full sha> }`.
/// Destructive by design: uncommitted work is discarded (`reset --hard` + `clean -fd`).
pub async fn restore(
    State(state): State<AppState>,
    Path(sha): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: RestoreRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!("expected JSON body {{\"project\": string}}: {e}"))
    })?;
    let dir = project_dir(&state, &req.project).await?;
    let restored = git::restore(&dir, &sha).await?;
    Ok(Json(json!({ "restored": restored })))
}
