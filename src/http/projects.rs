//! `GET/POST /api/projects`, `DELETE /api/projects/{id}?confirm=true`.

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::git::Identity;
use crate::github;
use crate::projects::{ProjectInfo, Source};
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
    let identity = state.github.identity().await;
    let info = create_and_start(&state, req.name, Source::Template, identity).await?;
    Ok((StatusCode::CREATED, Json(info)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CloneRequest {
    repo: String,
    name: Option<String>,
}

/// `POST /api/github/clone` `{ repo: "owner/name", name? }` → the new project (201).
pub async fn clone(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<(StatusCode, Json<ProjectInfo>), ApiError> {
    let req: CloneRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{\"repo\": \"owner/name\", \"name\"?: string}}: {e}"
        ))
    })?;
    let repo = github::validate_repo(&req.repo)?;
    let env = state.github.git_env().await?;
    let name = req
        .name
        .unwrap_or_else(|| repo.rsplit('/').next().unwrap_or(&repo).to_string());
    let source = Source::Clone {
        url: state.github.clone_url(&repo),
        repo,
        env,
    };
    let identity = state.github.identity().await;
    let info = create_and_start(&state, name, source, identity).await?;
    Ok((StatusCode::CREATED, Json(info)))
}

/// Create a project and finish its start-up (identity re-check, watcher, dev server) on
/// its own task: axum drops the handler future when the client disconnects, which would
/// abort a clone midway and leak the reserved id/port and a half-filled directory.
/// Spawned, it always runs to completion and cleans up after itself.
async fn create_and_start(
    state: &AppState,
    name: String,
    source: Source,
    identity: Identity,
) -> Result<ProjectInfo, ApiError> {
    let state = state.clone();
    tokio::spawn(async move {
        let info = state.projects.create_from(&name, source, &identity).await?;
        Ok(started(&state, info, &identity).await)
    })
    .await
    .map_err(|e| ApiError::io(format!("project creation task failed: {e}")))?
}

/// Watch a freshly created project and supervise its dev server; return its current info.
/// `used` is the identity the project was created with: if the user signed in or out while
/// scaffolding, `apply_identity` missed this project, so bring it up to date here.
async fn started(state: &AppState, info: ProjectInfo, used: &Identity) -> ProjectInfo {
    let current = state.github.identity().await;
    if current != *used {
        if let Some(p) = state.projects.get(&info.id).await {
            if let Err(e) = crate::git::set_identity(&p.dir, &current).await {
                eprintln!("orso: cannot update identity for {}: {e}", info.id);
            }
        }
    }
    state.watch_project(&info.id).await;
    // `start` marks the project `crashed` (and publishes it) if it cannot supervise, so
    // the response reflects the real state either way; the restart endpoint retries.
    if let Err(e) = state.devservers.start(&info.id).await {
        eprintln!("orso: cannot start dev server for {}: {e}", info.id);
    }
    match state.projects.get(&info.id).await {
        Some(p) => ProjectInfo::from(&p),
        None => info,
    }
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

/// `POST /api/projects/{id}/protection-hint/dismiss` → `{ dismissed: true }`. Hides the
/// "protect main" hint shown after Share preview for this project, across restarts.
pub async fn dismiss_protection_hint(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.projects.dismiss_protection_hint(&id).await?;
    Ok(Json(json!({ "dismissed": true })))
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
    // Stop the project's dev server, terminal and chat turn first so nothing writes into (or
    // serves from) a directory being removed.
    state.devservers.stop(&id).await;
    state.terminals.kill(&id).await;
    state.chats.stop(&id).await;
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
