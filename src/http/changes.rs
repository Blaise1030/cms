//! `POST /api/changes`, `GET /api/changes`, `GET /api/changes/active`,
//! `POST /api/changes/{branch}/switch`, `POST /api/changes/{branch}/share`,
//! `GET /api/changes/{branch}/status`, `POST /api/changes/{branch}/cleanup`.

use axum::body::Bytes;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{project_dir, query_or_badreq};
use crate::changes::{self, PrMerge};
use crate::error::ApiError;
use crate::github::{pr_body, pr_title, PrRef, PrState};
use crate::state::AppState;

/// Upper bound on each change's PR-state lookup in `GET /api/changes` (they run concurrently),
/// so a slow GitHub can't hold up the changes menu.
const PR_STATE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

#[derive(Deserialize)]
struct CreateRequest {
    project: String,
    title: String,
}

#[derive(Deserialize)]
struct SwitchRequest {
    project: String,
}

fn change_json(c: &changes::Change) -> Value {
    let mut v = json!({ "branch": c.branch, "title": c.title });
    if let Some(ts) = c.last_edited {
        v["lastEdited"] = json!(ts);
    }
    v
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
    Ok((StatusCode::CREATED, Json(change_json(&change))))
}

#[derive(Deserialize)]
pub struct ProjectQuery {
    project: String,
}

/// `GET /api/changes?project=` → `{ changes: [{ branch, title, lastEdited, prState? }, ...] }`.
/// `prState` (draft/open/approved/merged/closed) is best effort: only for a GitHub project
/// while signed in, and left out when GitHub can't be reached.
pub async fn list(
    State(state): State<AppState>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let project = state
        .projects
        .get(&q.project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {}", q.project)))?;
    let changes = changes::list(&project.dir).await?;
    let mut items: Vec<Value> = changes.iter().map(change_json).collect();
    if let Some(repo) = project.repo.as_deref() {
        if state.github.token().await.is_some() {
            let branches: Vec<String> = changes.iter().map(|c| c.branch.clone()).collect();
            let states = state
                .github
                .pr_states(repo, &branches, PR_STATE_TIMEOUT)
                .await;
            for ((item, change), pr) in items.iter_mut().zip(&changes).zip(states) {
                // A PR for other commits belongs to an earlier change that had this name.
                if let Some(pr) = pr {
                    if changes::pr_matches(&project.dir, &change.branch, &pr.head_sha).await {
                        item["prState"] = json!(pr.state);
                    }
                }
            }
        }
    }
    Ok(Json(json!({ "changes": items })))
}

/// `GET /api/changes/active?project=` → `{ branch, title }` or 404 when on `main`.
pub async fn active(
    State(state): State<AppState>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let dir = project_dir(&state, &q.project).await?;
    match changes::active(&dir).await? {
        Some(c) => Ok(Json(change_json(&c))),
        None => Err(ApiError::enoent("no active change")),
    }
}

/// `POST /api/changes/{branch}/switch` `{ project }` → `{ branch, title }`.
/// `branch` is percent-encoded when it contains `/` (e.g. `content%2Flaunch-week`).
pub async fn switch(
    State(state): State<AppState>,
    Path(branch): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: SwitchRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{\"project\": string}}: {e}"
        ))
    })?;
    let dir = project_dir(&state, &req.project).await?;
    let change = changes::switch(&dir, &branch).await?;
    Ok(Json(change_json(&change)))
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

/// `POST /api/changes/{branch}/cleanup` `{ project }` → `{ branch: "main", mainUpdated }`.
/// Deletes a merged change's local branch and returns to an up-to-date `main`. Merged means
/// its merged PR's head is the branch tip, or `main` already contains the branch's commits.
/// `mainUpdated` is false when GitHub couldn't be reached to update `main`.
pub async fn cleanup(
    State(state): State<AppState>,
    Path(branch): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: SwitchRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{\"project\": string}}: {e}"
        ))
    })?;
    let project = state
        .projects
        .get(&req.project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {}", req.project)))?;
    // Fail fast on what git can tell before asking GitHub.
    changes::check_cleanup(&project.dir, &branch).await?;
    let (pr, env) = match project.repo.as_deref() {
        Some(repo) if state.github.token().await.is_some() => {
            let pr = match tokio::time::timeout(
                PR_STATE_TIMEOUT,
                state.github.pr_ref(repo, &branch),
            )
            .await
            {
                Ok(Ok(Some(PrRef {
                    state: PrState::Merged,
                    head_sha,
                }))) => PrMerge::Merged { head_sha },
                Ok(Ok(_)) => PrMerge::NotMerged,
                Ok(Err(e)) => PrMerge::Unknown(e),
                Err(_) => PrMerge::Unknown(ApiError::github(
                    "GitHub didn't answer in time to confirm the merge — try again",
                )),
            };
            (pr, state.github.git_env().await.unwrap_or_default())
        }
        Some(_) => (
            PrMerge::NotMerged,
            state.github.git_env().await.unwrap_or_default(),
        ),
        None => (PrMerge::NotMerged, Vec::new()),
    };
    let done = changes::cleanup(&project.dir, &branch, pr, &env).await?;
    Ok(Json(json!({ "branch": "main", "mainUpdated": done.main_updated })))
}

/// `GET /api/changes/{branch}/status?project=` → `{ pr: { number, url, state, checks,
/// previewUrl } | null }`. `null` when the project has no GitHub remote or the branch no PR.
pub async fn status(
    State(state): State<AppState>,
    Path(branch): Path<String>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let project = state
        .projects
        .get(&q.project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {}", q.project)))?;
    let Some(repo) = project.repo else {
        return Ok(Json(json!({ "pr": null })));
    };
    let pr = state.github.pr_status(&repo, &branch).await?;
    // A PR for other commits belongs to an earlier change that had this name.
    let pr = match pr {
        Some(pr) if changes::pr_matches(&project.dir, &branch, &pr.head_sha).await => Some(pr),
        _ => None,
    };
    Ok(Json(json!({ "pr": pr })))
}
