use super::query_or_badreq;
use crate::settings::{self, SaveSettings, SettingsDoc};
use crate::{error::ApiError, state::AppState};
use axum::body::Bytes;
use axum::extract::{rejection::QueryRejection, Query, State};
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SettingsQuery {
    project: String,
}

/// `GET /api/settings?project=` reads the project's `orso.json`.
pub async fn get(
    State(state): State<AppState>,
    q: Result<Query<SettingsQuery>, QueryRejection>,
) -> Result<Json<SettingsDoc>, ApiError> {
    let q = query_or_badreq(q)?;
    Ok(Json(settings::read(&state, &q.project).await?))
}

/// `PUT /api/settings` saves `orso.json` on the open change.
pub async fn put(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<SettingsDoc>, ApiError> {
    let req: SaveSettings = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{project, branch, hash?, settings}}: {e}"
        ))
    })?;
    Ok(Json(settings::save(&state, req).await?))
}
