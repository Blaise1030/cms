//! `POST /api/projects/{id}/devserver/restart` — manual restart, e.g. after the crash
//! budget was exhausted. Resets the budget and answers with the project (`starting`).
//!
//! `GET /api/projects/{id}/preview` — where to frame the project's preview:
//! `{ port, token }` of its own origin (see [`crate::preview`]).

use axum::extract::{Path, State};
use axum::Json;

use crate::error::ApiError;
use crate::preview::PreviewAccess;
use crate::projects::ProjectInfo;
use crate::state::AppState;

pub async fn restart(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ProjectInfo>, ApiError> {
    if state.projects.get(&id).await.is_none() {
        return Err(ApiError::enoent(format!("no such project: {id}")));
    }
    state.devservers.start(&id).await?;
    let project = state
        .projects
        .get(&id)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {id}")))?;
    Ok(Json(ProjectInfo::from(&project)))
}

/// The project's preview origin, opened on first use. Behind daemon auth, so only the app
/// learns the origin's token.
pub async fn preview(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PreviewAccess>, ApiError> {
    Ok(Json(state.preview_origins.ensure(&id).await?))
}
