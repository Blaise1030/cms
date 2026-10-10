//! `POST /api/changes`, `GET /api/changes`, `GET /api/changes/active`,
//! `POST /api/changes/{branch}/switch`, `POST /api/changes/{branch}/share`.

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
use crate::github::{pr_body, pr_title, BranchProtection};
use crate::state::AppState;

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

/// `GET /api/changes?project=` → `{ changes: [{ branch, title, lastEdited }, ...] }`.
pub async fn list(
    State(state): State<AppState>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let dir = project_dir(&state, &q.project).await?;
    let changes = changes::list(&dir).await?;
    Ok(Json(json!({
        "changes": changes.iter().map(change_json).collect::<Vec<_>>(),
    })))
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

/// The "protect main" hint for a share response: only when `main` is known to be
/// unprotected and the admin has not dismissed it for this project.
fn protection_hint(dismissed: bool, protection: BranchProtection, settings_url: String) -> Value {
    if !dismissed && protection == BranchProtection::Unprotected {
        json!({ "settingsUrl": settings_url })
    } else {
        Value::Null
    }
}

/// `POST /api/changes/{branch}/share` `{ project }` → `{ branch, title, prUrl, protectionHint }`.
/// Pushes the branch and opens (or reuses) a draft PR titled after the change.
/// `protectionHint` is `{ settingsUrl }` when `main` is unprotected (see
/// `docs/branch-protection.md`), else null.
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
    let hint_dismissed = project.protection_hint_dismissed;
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

    let protection = if hint_dismissed {
        BranchProtection::Unknown
    } else {
        state.github.main_protection(&repo).await
    };
    let hint = protection_hint(
        hint_dismissed,
        protection,
        state.github.branch_settings_url(&repo),
    );

    Ok(Json(json!({
        "branch": change.branch,
        "title": change.title,
        "prUrl": pr.html_url,
        "protectionHint": hint,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protection_hint_only_for_unprotected_main_until_dismissed() {
        let url = || "https://github.com/acme/site/settings/branches".to_string();
        assert_eq!(
            protection_hint(false, BranchProtection::Unprotected, url()),
            json!({ "settingsUrl": "https://github.com/acme/site/settings/branches" })
        );
        assert_eq!(
            protection_hint(false, BranchProtection::Protected, url()),
            Value::Null
        );
        // 403 (no admin rights) → unknown → don't show.
        assert_eq!(
            protection_hint(false, BranchProtection::Unknown, url()),
            Value::Null
        );
        assert_eq!(
            protection_hint(true, BranchProtection::Unprotected, url()),
            Value::Null
        );
    }
}
