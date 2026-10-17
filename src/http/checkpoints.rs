//! `GET /api/checkpoints`, `GET /api/checkpoints/{sha}/diff`, `POST /api/checkpoints/{sha}/restore`.
//!
//! Also hosts the "start a change first" guard used by mutating edit paths (terminal
//! start and turn-end checkpoints), and the shared turn-end commit (`finish_turn`) for
//! terminal hooks and chat. Restore on `main` is intentionally not gated (Slice 3 out of
//! scope).

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use axum::body::Bytes;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{project_dir, query_or_badreq, terminal};
use crate::changes;
use crate::error::ApiError;
use crate::git;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ProjectQuery {
    project: String,
}

static TURN_END_SERIAL: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn turn_end_lock(project_id: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut map = TURN_END_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    map.entry(project_id.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// What an agent turn end did to the project's history.
#[derive(Debug)]
pub enum TurnOutcome {
    /// `files` are the paths the checkpoint captured, sorted.
    Committed {
        checkpoint: git::Checkpoint,
        files: Vec<String>,
    },
    /// Edits were left uncommitted because the project needs a change first.
    Blocked(String),
    Unchanged,
}

/// Checkpoint the agent's edits at the end of a turn (terminal hook or chat).
///
/// Serialised per project. Emits `checkpoint.blocked` / `checkpoint.created`, then lets a
/// pending change update move the checkout now that the agent is idle.
pub async fn finish_turn(
    state: &AppState,
    project_id: &str,
    summary: Option<&str>,
) -> Result<TurnOutcome, ApiError> {
    let lock = turn_end_lock(project_id);
    let _guard = lock.lock().await;
    let dir = project_dir(state, project_id).await?;
    if let Err(e) = require_change(state, project_id).await {
        // A turn that only talked (no edits) on `main` is fine.
        if !changes::is_dirty(&dir).await? {
            return Ok(TurnOutcome::Unchanged);
        }
        state.events.emit(
            "checkpoint.blocked",
            json!({
                "projectId": project_id,
                "reason": e.message,
            }),
        );
        return Ok(TurnOutcome::Blocked(e.message));
    }
    let mut files = git::changed_paths(&dir).await?;
    let message = terminal::commit_message(summary, &files);
    files.sort();
    let outcome = match git::checkpoint(&dir, &message).await? {
        Some(cp) => {
            state.events.emit(
                "checkpoint.created",
                json!({
                    "projectId": project_id,
                    "sha": cp.sha,
                    "summary": cp.summary,
                    "author": cp.author,
                    "ts": cp.ts,
                }),
            );
            TurnOutcome::Committed {
                checkpoint: cp,
                files,
            }
        }
        None => TurnOutcome::Unchanged,
    };
    tokio::spawn(super::changes::finish_pending_update(
        state.clone(),
        project_id.to_string(),
    ));
    Ok(outcome)
}

/// Reject edits on `main` for GitHub-remote projects. Local-only projects may edit `main`.
///
/// Used by terminal start and hook turn-end checkpoints. Restore stays ungated.
pub(crate) async fn require_change(state: &AppState, project_id: &str) -> Result<(), ApiError> {
    let project = state
        .projects
        .get(project_id)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {project_id}")))?;
    changes::require_change(&project.dir, project.repo.as_deref()).await
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
