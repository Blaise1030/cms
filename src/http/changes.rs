//! `POST /api/changes`, `GET /api/changes`, `GET /api/changes/active`,
//! `POST /api/changes/{branch}/switch`.

use axum::body::Bytes;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{project_dir, query_or_badreq};
use crate::changes;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
struct CreateRequest {
    project: String,
    title: String,
}

#[derive(Deserialize)]
struct SwitchRequest {
    project: String,
}

fn change_json(c: &changes::Change) -> Value {
    let mut v = json!({ "branch": c.branch, "title": c.title });
    if let Some(ts) = c.last_edited {
        v["lastEdited"] = json!(ts);
    }
    v
}

/// `POST /api/changes` `{ project, title }` → `{ branch, title }` (201).
pub async fn create(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let req: CreateRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{\"project\": string, \"title\": string}}: {e}"
        ))
    })?;
    let dir = project_dir(&state, &req.project).await?;
    let change = changes::create(&dir, &req.title).await?;
    Ok((StatusCode::CREATED, Json(change_json(&change))))
}

#[derive(Deserialize)]
pub struct ProjectQuery {
    project: String,
}

/// `GET /api/changes?project=` → `{ changes: [{ branch, title, lastEdited }, ...] }`.
pub async fn list(
    State(state): State<AppState>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let dir = project_dir(&state, &q.project).await?;
    let changes = changes::list(&dir).await?;
    Ok(Json(json!({
        "changes": changes.iter().map(change_json).collect::<Vec<_>>(),
    })))
}

/// `GET /api/changes/active?project=` → `{ branch, title }` or 404 when on `main`.
pub async fn active(
    State(state): State<AppState>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let dir = project_dir(&state, &q.project).await?;
    match changes::active(&dir).await? {
        Some(c) => Ok(Json(change_json(&c))),
        None => Err(ApiError::enoent("no active change")),
    }
}

/// `POST /api/changes/{branch}/switch` `{ project }` → `{ branch, title }`.
/// `branch` is percent-encoded when it contains `/` (e.g. `content%2Flaunch-week`).
pub async fn switch(
    State(state): State<AppState>,
    Path(branch): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: SwitchRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{\"project\": string}}: {e}"
        ))
    })?;
    let dir = project_dir(&state, &req.project).await?;
    let change = changes::switch(&dir, &branch).await?;
    Ok(Json(change_json(&change)))
}
