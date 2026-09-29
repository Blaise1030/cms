//! Read-only browsing: `GET /api/tree` and `GET /api/file`.

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{project_dir, query_or_badreq};
use crate::error::ApiError;
use crate::fs;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct PathQuery {
    project: String,
    #[serde(default)]
    path: String,
}

#[derive(Deserialize)]
pub struct FileQuery {
    project: String,
    path: String,
    #[serde(default)]
    raw: bool,
}

/// `GET /api/tree?project=&path=` → `{ path, entries: [{ name, path, kind, size }] }`.
/// `path` defaults to the project root.
pub async fn tree(
    State(state): State<AppState>,
    q: Result<Query<PathQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let dir = project_dir(&state, &q.project).await?;
    let entries = fs::tree(&dir, &q.path).await?;
    Ok(Json(
        json!({ "path": q.path.trim_matches('/'), "entries": entries }),
    ))
}

/// `GET /api/file?project=&path=` → `{ path, size, content, binary }`.
/// With `raw=true` the bytes are returned with a best-effort content type instead.
pub async fn file(
    State(state): State<AppState>,
    q: Result<Query<FileQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let q = query_or_badreq(q)?;
    let dir = project_dir(&state, &q.project).await?;
    if q.raw {
        let (bytes, ct) = fs::read_raw(&dir, &q.path).await?;
        // `sandbox` CSP + nosniff: a served .html/.svg from the project can't script the
        // daemon's origin.
        return Ok((
            [
                (header::CONTENT_TYPE, ct),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (header::CONTENT_SECURITY_POLICY, "sandbox"),
            ],
            bytes,
        )
            .into_response());
    }
    Ok(Json(fs::read_file(&dir, &q.path).await?).into_response())
}
