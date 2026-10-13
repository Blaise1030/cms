use super::query_or_badreq;
use crate::design::{self, DesignDoc, SaveDesign};
use crate::{error::ApiError, state::AppState};
use axum::body::Bytes;
use axum::extract::{rejection::QueryRejection, Query, State};
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DesignQuery {
    project: String,
}

/// `GET /api/design?project=` reads the project's `DESIGN.md` (or the starter when absent).
pub async fn get(
    State(state): State<AppState>,
    q: Result<Query<DesignQuery>, QueryRejection>,
) -> Result<Json<DesignDoc>, ApiError> {
    let q = query_or_badreq(q)?;
    Ok(Json(design::read(&state, &q.project).await?))
}

/// `PUT /api/design` saves tokens and body to `DESIGN.md` on the open change.
pub async fn put(State(state): State<AppState>, body: Bytes) -> Result<Json<DesignDoc>, ApiError> {
    let req: SaveDesign = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{project, branch, hash?, tokens, body}}: {e}"
        ))
    })?;
    Ok(Json(design::save(&state, req).await?))
}
