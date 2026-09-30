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
    let mut info = state.projects.create(&req.name).await?;
    state.watch_project(&info.id).await;
    // Scaffold done: supervise `npm run dev`. A failure here (no usable port) leaves the
    // project created but `crashed`; the restart endpoint retries.
    // `start` marks the project `crashed` (and publishes it) if it cannot supervise, so
    // the response below reflects the real state either way.
    if let Err(e) = state.devservers.start(&info.id).await {
        eprintln!("workbench: cannot start dev server for {}: {e}", info.id);
    }
    if let Some(p) = state.projects.get(&info.id).await {
        info = ProjectInfo::from(&p);
    }
    Ok((StatusCode::CREATED, Json(info)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateRequest {
    auto_approve: Option<bool>,
    name: Option<String>,
}

/// `PATCH /api/projects/{id}` `{ autoApprove?, name? }` → the updated project.
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<ProjectInfo>, ApiError> {
    let req: UpdateRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{\"autoApprove\"?: bool, \"name\"?: string}}: {e}"
        ))
    })?;
    if req.auto_approve.is_none() && req.name.is_none() {
        return Err(ApiError::badreq(
            "nothing to update: pass autoApprove and/or name",
        ));
    }
    let info = state
        .projects
        .update(&id, req.name.as_deref(), req.auto_approve)
        .await?;
    Ok(Json(info))
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
    // Stop the project's dev server and agent first so nothing writes into (or serves
    // from) a directory being removed.
    state.devservers.stop(&id).await;
    state.agents.terminate_project(&id).await;
    state.watchers.stop(&id);
    if let Err(e) = state.projects.delete(&id).await {
        // Delete failed and the project lives on: bring its preview back.
        if state.projects.get(&id).await.is_some() {
            state.watch_project(&id).await;
            let _ = state.devservers.start(&id).await;
        }
        return Err(e);
    }
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
