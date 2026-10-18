//! `POST /api/ctl/*` — app state for the agent running in a project's terminal
//! (`workbench ctl`). Authenticated like turn-end: the live session's hook token in the
//! JSON body, no daemon token. Read-only except `restart-preview`.

use axum::body::Bytes;
use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::changes;
use crate::error::ApiError;
use crate::git;
use crate::state::AppState;

#[derive(Deserialize)]
struct CtlRequest {
    project: String,
    token: String,
}

/// Parse the body and check the hook token; answers the project id.
fn authorize(state: &AppState, body: &Bytes) -> Result<String, ApiError> {
    let req: CtlRequest = serde_json::from_slice(body)
        .map_err(|e| ApiError::badreq(format!("expected JSON body {{project, token}}: {e}")))?;
    if !state.terminals.token_matches(&req.project, &req.token) {
        return Err(ApiError::unauth("invalid hook token").with_status(403));
    }
    Ok(req.project)
}

/// `{devserver: "starting"|"ready"|"crashed", port, previewPath}`.
pub async fn preview_status_value(state: &AppState, project: &str) -> Result<Value, ApiError> {
    let p = state
        .projects
        .get(project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {project}")))?;
    Ok(json!({
        "devserver": p.devserver.as_str(),
        "port": p.port,
        "previewPath": format!("/preview/{}/", p.id),
    }))
}

/// `{active: {branch, title} | null, dirty, changedPaths, changes}`. `dirty` counts new
/// files too: an agent's unsaved work is often a file git does not track yet.
pub async fn change_status_value(state: &AppState, project: &str) -> Result<Value, ApiError> {
    let dir = super::project_dir(state, project).await?;
    let active = changes::active(&dir).await?;
    let changed = git::changed_paths(&dir).await?;
    let open = changes::list(&dir).await?;
    Ok(json!({
        "active": active,
        "dirty": !changed.is_empty(),
        "changedPaths": changed,
        "changes": open,
    }))
}

pub async fn preview_status(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let project = authorize(&state, &body)?;
    Ok(Json(preview_status_value(&state, &project).await?))
}

pub async fn restart_preview(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let project = authorize(&state, &body)?;
    state.devservers.start(&project).await?;
    Ok(Json(preview_status_value(&state, &project).await?))
}

pub async fn change_status(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let project = authorize(&state, &body)?;
    Ok(Json(change_status_value(&state, &project).await?))
}
