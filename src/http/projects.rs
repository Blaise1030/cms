//! `GET/POST /api/projects`, `DELETE /api/projects/{id}?confirm=true`.

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::projects::ProjectInfo;
use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "projects": state.projects.list().await }))
}

#[derive(Deserialize)]
struct CreateRequest {
    name: String,
}

/// `POST /api/projects` `{ name }`. The body is parsed by hand so malformed input
/// gets the uniform error envelope rather than axum's plain-text rejection.
pub async fn create(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<(StatusCode, Json<ProjectInfo>), ApiError> {
    let req: CreateRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::badreq(format!("expected JSON body {{\"name\": string}}: {e}")))?;
    let info = state.projects.create(&req.name).await?;
    Ok((StatusCode::CREATED, Json(info)))
}

/// `DELETE /api/projects/{id}?confirm=true` — destructive, so the flag is mandatory.
pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
) -> Result<Json<Value>, ApiError> {
    if !has_confirm(query.as_deref()) {
        return Err(ApiError::badreq(
            "deleting a project removes its files; pass ?confirm=true to proceed",
        ));
    }
    state.projects.delete(&id).await?;
    Ok(Json(json!({ "deleted": id })))
}

fn has_confirm(query: Option<&str>) -> bool {
    query
        .unwrap_or("")
        .split('&')
        .any(|pair| pair == "confirm=true")
}

#[cfg(test)]
mod tests {
    use super::has_confirm;

    #[test]
    fn confirm_flag_parsing() {
        assert!(has_confirm(Some("confirm=true")));
        assert!(has_confirm(Some("a=b&confirm=true")));
        for bad in [
            None,
            Some(""),
            Some("confirm=false"),
            Some("confirm"),
            Some("confirm=truex"),
            Some("xconfirm=true"),
        ] {
            assert!(!has_confirm(bad), "{bad:?}");
        }
    }
}
