//! `POST /api/changes`, `GET /api/changes/active`, `POST /api/changes/{branch}/share`.

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
use crate::github::{pr_body, pr_title};
use crate::state::AppState;

#[derive(Deserialize)]
struct CreateRequest {
    project: String,
    title: String,
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
    Ok((
        StatusCode::CREATED,
        Json(json!({ "branch": change.branch, "title": change.title })),
    ))
}

#[derive(Deserialize)]
pub struct ProjectQuery {
    project: String,
}

/// `GET /api/changes/active?project=` → `{ branch, title }` or 404 when on `main`.
pub async fn active(
    State(state): State<AppState>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let dir = project_dir(&state, &q.project).await?;
    match changes::active(&dir).await? {
        Some(c) => Ok(Json(json!({ "branch": c.branch, "title": c.title }))),
        None => Err(ApiError::enoent("no active change")),
    }
}

#[derive(Deserialize)]
struct ShareRequest {
    project: String,
}

/// `POST /api/changes/{branch}/share` `{ project }` → `{ branch, title, prUrl }`.
/// Pushes the branch and opens (or reuses) a draft PR titled after the change.
pub async fn share(
    State(state): State<AppState>,
    Path(branch): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: ShareRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{\"project\": string}}: {e}"
        ))
    })?;
    let project = state
        .projects
        .get(&req.project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {}", req.project)))?;
    let repo = project.repo.ok_or_else(|| {
        ApiError::badreq("this project has no GitHub remote — open a repository first")
    })?;
    let dir = project.dir;
    let change = changes::get(&dir, &branch).await?;
    let env = state.github.git_env().await?;
    changes::push(&dir, &branch, &env).await?;

    let pr = match state.github.find_pr(&repo, &branch).await? {
        Some(existing) => existing,
        None => {
            let title = pr_title(&change.title);
            let body = pr_body(&change.title);
            state
                .github
                .create_draft_pr(&repo, &branch, &title, &body)
                .await?
        }
    };

    Ok(Json(json!({
        "branch": change.branch,
        "title": change.title,
        "prUrl": pr.html_url,
    })))
}
