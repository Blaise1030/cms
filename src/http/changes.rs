//! `POST /api/changes`, `GET /api/changes`, `GET /api/changes/active`,
//! `POST /api/changes/{branch}/switch`, `POST /api/changes/{branch}/share`,
//! `GET /api/changes/{branch}/status`, `GET /api/docs/branch-protection`, `POST /api/changes/update`,
//! `POST /api/changes/{branch}/cleanup`, `POST /api/changes/{branch}/ready`,
//! `POST /api/changes/{branch}/publish`.

use std::collections::BTreeMap;

use axum::body::Bytes;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{project_dir, query_or_badreq};
use crate::changes::auto_resolve::JobContext;
use crate::changes::conflicts::{self, Choice};
use crate::changes::{self, PrMerge};
use crate::conflict_preview::{PreviewState, Source};
use crate::error::ApiError;
use crate::github::{pr_body, pr_title, BranchProtection, PrRef, PrState};
use crate::projects::STATE_DIR;
use crate::providers;
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
    save_edits(&state, &req.project, &dir, true).await?;
    let change = changes::create(&dir, &req.title).await?;
    Ok((StatusCode::CREATED, Json(change_json(&change))))
}

/// Checkpoint leftover edits onto the change they were made on, so moving to another change
/// doesn't stop on them. Edits on `main` are left alone when `carry_main` (a new change takes
/// them) and on GitHub projects (never committed to `main`). Not while a terminal is live: the
/// agent may be mid-edit.
async fn save_edits(
    state: &AppState,
    project_id: &str,
    dir: &std::path::Path,
    carry_main: bool,
) -> Result<(), ApiError> {
    if state.terminals.status(project_id).is_some() || !changes::is_dirty(dir).await? {
        return Ok(());
    }
    let on_change = changes::active(dir).await?.is_some();
    let local_only = state
        .projects
        .get(project_id)
        .await
        .is_some_and(|p| p.repo.is_none());
    if !on_change && (carry_main || !local_only) {
        return Ok(());
    }
    if let Some(cp) = crate::git::checkpoint(dir, "Saved edits").await? {
        state.events.emit(
            "checkpoint.created",
            json!({
                "projectId": project_id,
                "sha": cp.sha,
                "summary": cp.summary,
                "author": cp.author,
                "ts": cp.ts,
            }),
        );
    }
    Ok(())
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
        ApiError::badreq(format!("expected JSON body {{\"project\": string}}: {e}"))
    })?;
    let dir = project_dir(&state, &req.project).await?;
    save_edits(&state, &req.project, &dir, false).await?;
    let change = changes::switch(&dir, &branch).await?;
    Ok(Json(change_json(&change)))
}

#[derive(Deserialize)]
struct ShareRequest {
    project: String,
}

/// Where the daemon serves `docs/branch-protection.md` (bundled into the binary).
pub const PROTECTION_DOCS_PATH: &str = "/api/docs/branch-protection";

/// `GET /api/docs/branch-protection` → the branch protection guide as plain text.
pub async fn branch_protection_doc() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        include_str!("../../docs/branch-protection.md"),
    )
}

/// The "protect main" hint for a share response: `{ settingsUrl, docsUrl }` only when
/// `main` is known to be unprotected and the hint is not dismissed for `project`, else null.
/// The dismissal is read now (not from a snapshot taken before the push); once dismissed,
/// GitHub is not asked at all.
async fn protection_hint_for(state: &AppState, project: &str, repo: &str) -> Value {
    if state.projects.protection_hint_dismissed(project).await {
        return Value::Null;
    }
    match state.github.main_protection(repo).await {
        BranchProtection::Unprotected => json!({
            "settingsUrl": state.github.branch_settings_url(repo),
            "docsUrl": PROTECTION_DOCS_PATH,
        }),
        BranchProtection::Protected | BranchProtection::Unknown => Value::Null,
    }
}

/// `POST /api/changes/{branch}/share` `{ project }` → `{ branch, title, prUrl, protectionHint }`.
/// Pushes the branch and opens (or reuses) a draft PR titled after the change.
/// `protectionHint` is `{ settingsUrl, docsUrl }` when `main` is unprotected (see
/// `docs/branch-protection.md`), else null.
pub async fn share(
    State(state): State<AppState>,
    Path(branch): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: ShareRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!("expected JSON body {{\"project\": string}}: {e}"))
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

    let hint = protection_hint_for(&state, &req.project, &repo).await;

    Ok(Json(json!({
        "branch": change.branch,
        "title": change.title,
        "prUrl": pr.html_url,
        "protectionHint": hint,
    })))
}

#[derive(Deserialize)]
struct UpdateRequest {
    project: String,
}

/// `POST /api/changes/update` `{ project }` →
/// `{ mainUpdated, mainDiverged, updated: [{ branch, title }], conflicts: [{ branch, title }] }`.
/// Get latest: pull `main`, merge it into every open change; conflicting changes are left as-is.
/// While that project's terminal is live the checkout is left alone (`skipped`) — git must not
/// rewrite files under the CLI — and caught up by [`finish_pending_update`] once it is idle.
pub async fn update(State(state): State<AppState>, body: Bytes) -> Result<Json<Value>, ApiError> {
    let req: UpdateRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!("expected JSON body {{\"project\": string}}: {e}"))
    })?;
    let leave_checkout = state.terminals.status(&req.project).is_some();
    Ok(Json(
        run_update(&state, &req.project, leave_checkout).await?,
    ))
}

/// Finish a Get latest that skipped the checked-out change, now that nothing is editing it
/// (turn end, terminal stopped). Emits `changes.updated` with the result or the error.
pub async fn finish_pending_update(state: AppState, project_id: String) {
    let pending = state.pending_latest.lock().unwrap().remove(&project_id);
    if !pending {
        return;
    }
    let payload = match run_update(&state, &project_id, false).await {
        Ok(result) => json!({ "projectId": project_id, "result": result }),
        Err(e) => json!({ "projectId": project_id, "error": e.message }),
    };
    state.events.emit("changes.updated", payload);
}

async fn run_update(
    state: &AppState,
    project_id: &str,
    leave_checkout: bool,
) -> Result<Value, ApiError> {
    let project = state
        .projects
        .get(project_id)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {project_id}")))?;
    if !leave_checkout {
        save_edits(state, project_id, &project.dir, false).await?;
    }
    let _update_guard = state.conflict_updates.lock().await;
    if !state.conflicts.list(project_id).await.is_empty() {
        return Err(ApiError::conflict(
            "Finish or abandon the open conflict before running Get latest again",
        ));
    }
    // GitHub projects fetch with the signed-in user's credentials (private repos).
    let env = if project.repo.is_some() {
        state.github.git_env().await?
    } else {
        Vec::new()
    };
    let result = if leave_checkout {
        changes::get_latest_leaving_checkout(&project.dir, &env).await?
    } else {
        changes::get_latest(&project.dir, &env).await?
    };
    {
        let mut pending = state.pending_latest.lock().unwrap();
        if result.skipped.is_some() {
            pending.insert(project_id.to_string());
        } else {
            pending.remove(project_id);
        }
    }
    let mut value = serde_json::to_value(&result).map_err(|e| ApiError::io(e.to_string()))?;
    let incoming_ref = if result.main_diverged {
        "refs/remotes/origin/main"
    } else {
        "refs/heads/main"
    };
    let incoming = conflicts::text(&project.dir, &["rev-parse", "--verify", incoming_ref]).await?;
    let mut sessions = serde_json::Map::new();
    let mut errors = serde_json::Map::new();
    for change in &result.conflicts {
        match state
            .conflicts
            .start(&project.dir, project_id, &change.branch, &incoming)
            .await
        {
            Ok(session) => {
                sessions.insert(change.branch.clone(), json!(session.id));
            }
            Err(error) => {
                errors.insert(change.branch.clone(), json!(error.to_string()));
            }
        }
    }
    value["conflictSessions"] = Value::Object(sessions);
    value["conflictErrors"] = Value::Object(errors);
    Ok(value)
}

async fn project_session(
    state: &AppState,
    project: &str,
    id: &str,
) -> Result<changes::conflict_sessions::ConflictSession, ApiError> {
    let session = state.conflicts.get(id).await?;
    let dir = project_dir(state, project).await?;
    if session.project_id != project || session.dir != dir {
        return Err(ApiError::enoent("conflict session not found"));
    }
    Ok(session)
}

/// Active conflict sessions survive page reloads while the daemon is running.
pub async fn list_conflicts(
    State(state): State<AppState>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    project_dir(&state, &q.project).await?;
    Ok(Json(
        json!({ "sessions": state.conflicts.list(&q.project).await }),
    ))
}

pub async fn get_conflict(
    State(state): State<AppState>,
    Path(id): Path<String>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let session = project_session(&state, &q.project, &id).await?;
    let mut value = serde_json::to_value(&session).map_err(|e| ApiError::io(e.to_string()))?;
    let mut files = Vec::new();
    for file in &session.files {
        let patch = conflicts::text(
            &session.dir,
            &[
                "diff",
                "--no-ext-diff",
                &session.tip,
                &session.incoming,
                "--",
                &file.path,
            ],
        )
        .await?;
        let mut f = serde_json::to_value(file).map_err(|e| ApiError::io(e.to_string()))?;
        f["patch"] = json!(patch);
        files.push(f);
    }
    value["files"] = json!(files);
    value["candidate"] = serde_json::to_value(
        conflicts::candidate_tree(
            &session.dir,
            &session.merged_tree,
            &session.files,
            &session.choices,
        )
        .await?,
    )
    .map_err(|e| ApiError::io(e.to_string()))?;
    Ok(Json(value))
}

pub async fn conflict_previews(
    State(state): State<AppState>,
    Path(id): Path<String>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let session = project_session(&state, &q.project, &id).await?;
    let candidate = conflicts::candidate_tree(
        &session.dir,
        &session.merged_tree,
        &session.files,
        &session.choices,
    )
    .await?;
    let yours = state
        .conflict_previews
        .prepare(&session.dir, &q.project, &id, Source::Yours, &session.tip)
        .await;
    let latest = state
        .conflict_previews
        .prepare(
            &session.dir,
            &q.project,
            &id,
            Source::Latest,
            &session.incoming,
        )
        .await;
    let result = if candidate.complete {
        let commit = conflicts::preview_commit(&session.dir, &candidate.tree, &session.tip).await?;
        state
            .conflict_previews
            .prepare(&session.dir, &q.project, &id, Source::Result, &commit)
            .await
    } else {
        state
            .conflict_previews
            .remove_source(&id, Source::Result)
            .await;
        PreviewState::Unavailable {
            reason: "Choose a resolution for every file to preview the result".into(),
        }
    };
    Ok(Json(
        json!({ "yours": yours, "latest": latest, "result": result }),
    ))
}

#[derive(Deserialize)]
struct ChoiceRequest {
    project: String,
    path: String,
    choice: Choice,
}

pub async fn choose_conflict(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<conflicts::Candidate>, ApiError> {
    let req: ChoiceRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::badreq(format!("invalid conflict choice: {e}")))?;
    project_session(&state, &req.project, &id).await?;
    not_combining(&state, &id).await?;
    let candidate = state.conflicts.choose(&id, &req.path, req.choice).await?;
    state
        .conflict_previews
        .remove_source(&id, Source::Result)
        .await;
    Ok(Json(candidate))
}

pub async fn finish_conflict(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: UpdateRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::badreq(format!("invalid project: {e}")))?;
    project_session(&state, &req.project, &id).await?;
    not_combining(&state, &id).await?;
    let identity = state.github.identity().await;
    let commit = state
        .conflicts
        .finish_validated(&id, &identity, &state.conflict_previews)
        .await?;
    state.conflict_previews.remove_session(&id).await;
    Ok(Json(json!({ "commit": commit })))
}

/// Manual choices, finishing and abandoning would race the agent's job on the same session.
async fn not_combining(state: &AppState, id: &str) -> Result<(), ApiError> {
    if state.auto_resolve.is_running(id).await {
        return Err(ApiError::conflict("Combining is in progress."));
    }
    Ok(())
}

#[derive(Deserialize)]
struct AutoRequest {
    project: String,
    provider: String,
}

/// `POST /api/changes/conflicts/{id}/auto` `{ project, provider }` → `{ started: true }`.
/// Hands the conflict to a headless agent; progress via `GET …/auto` and `changes.auto_resolve`.
pub async fn start_auto(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: AutoRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{\"project\": string, \"provider\": string}}: {e}"
        ))
    })?;
    let session = project_session(&state, &req.project, &id).await?;
    let provider = providers::load(state.config.agents_config.as_deref())?
        .into_iter()
        .find(|p| p.id == req.provider)
        .ok_or_else(|| ApiError::badreq(format!("unknown provider: {}", req.provider)))?;
    state
        .auto_resolve
        .start(JobContext {
            project_id: req.project,
            project_dir: session.dir,
            session_id: id,
            provider,
            author: state.github.identity().await,
            state_dir: state.config.projects_dir.join(STATE_DIR),
            events: state.events.clone(),
            sessions: state.conflicts.clone(),
            previews: state.conflict_previews.clone(),
        })
        .await?;
    Ok(Json(json!({ "started": true })))
}

/// The session is gone once a job succeeds or the session expires, so a job started by
/// `project` also proves ownership. `idle_ok` lets a session with neither through.
async fn job_owner(
    state: &AppState,
    project: &str,
    id: &str,
    idle_ok: bool,
) -> Result<(), ApiError> {
    project_dir(state, project).await?;
    if state.conflicts.get(id).await.is_ok() {
        return project_session(state, project, id).await.map(drop);
    }
    match state.auto_resolve.project_of(id).await {
        Some(owner) if owner == project => Ok(()),
        None if idle_ok => Ok(()),
        _ => Err(ApiError::enoent("conflict session not found")),
    }
}

/// `GET /api/changes/conflicts/{id}/auto?project=` → the job's status, `idle` when none.
pub async fn auto_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    q: Result<Query<ProjectQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    job_owner(&state, &q.project, &id, true).await?;
    Ok(Json(match state.auto_resolve.status(&id).await {
        Some(status) => serde_json::to_value(status).map_err(|e| ApiError::io(e.to_string()))?,
        None => json!({
            "sessionId": id,
            "state": "idle",
            "stage": "",
            "questions": [],
            "summary": null,
            "error": null,
        }),
    }))
}

#[derive(Deserialize)]
struct AnswersRequest {
    project: String,
    answers: BTreeMap<String, String>,
}

/// `POST /api/changes/conflicts/{id}/auto/answers` `{ project, answers: { qid: text } }`.
pub async fn answer_auto(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: AnswersRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{\"project\": string, \"answers\": object}}: {e}"
        ))
    })?;
    job_owner(&state, &req.project, &id, false).await?;
    state.auto_resolve.answer(&id, req.answers).await?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/changes/conflicts/{id}/auto/cancel` `{ project }`.
pub async fn cancel_auto(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: UpdateRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::badreq(format!("invalid project: {e}")))?;
    job_owner(&state, &req.project, &id, false).await?;
    state.auto_resolve.cancel(&id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn abandon_conflict(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: UpdateRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::badreq(format!("invalid project: {e}")))?;
    project_session(&state, &req.project, &id).await?;
    not_combining(&state, &id).await?;
    state.conflicts.abandon(&id).await?;
    state.conflict_previews.remove_session(&id).await;
    Ok(Json(json!({ "abandoned": true })))
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
        ApiError::badreq(format!("expected JSON body {{\"project\": string}}: {e}"))
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
            let pr =
                match tokio::time::timeout(PR_STATE_TIMEOUT, state.github.pr_ref(repo, &branch))
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
    Ok(Json(
        json!({ "branch": "main", "mainUpdated": done.main_updated }),
    ))
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

/// `POST /api/changes/{branch}/ready` `{ project }` → `{ pr: { number, url, state, checks,
/// previewUrl } }`. Marks the change's draft pull request ready for review, then returns its
/// status (open, even if GitHub's re-read still lags).
pub async fn ready(
    State(state): State<AppState>,
    Path(branch): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: ShareRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!("expected JSON body {{\"project\": string}}: {e}"))
    })?;
    let project = state
        .projects
        .get(&req.project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {}", req.project)))?;
    // Only this project's own changes: never another (e.g. a teammate's) branch's PR.
    changes::get(&project.dir, &branch).await?;
    let repo = project.repo.ok_or_else(|| {
        ApiError::badreq("this project has no GitHub remote — open a repository first")
    })?;
    let pr = state.github.mark_ready_for_review(&repo, &branch).await?;
    Ok(Json(json!({ "pr": pr })))
}

/// `POST /api/changes/{branch}/publish` `{ project }` → `{ pr: { number, url, state } }`.
/// Publishes the change: pushes it, shares it (opening a pull request if it has none), marks
/// it ready, then merges it on GitHub. Anyone on the team may publish — approvals and
/// CODEOWNERS are not checked. Local cleanup afterwards stays with the existing Clean up
/// button ([`cleanup`]); publishing does not delete the change.
pub async fn publish(
    State(state): State<AppState>,
    Path(branch): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: ShareRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!("expected JSON body {{\"project\": string}}: {e}"))
    })?;
    let project = state
        .projects
        .get(&req.project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {}", req.project)))?;
    // Only this project's own changes: never another (e.g. a teammate's) branch's PR.
    let change = changes::get(&project.dir, &branch).await?;
    let repo = project.repo.ok_or_else(|| {
        ApiError::badreq("this project has no GitHub remote — open a repository first")
    })?;
    // Publish the change's latest commits, not an older shared copy.
    let env = state.github.git_env().await?;
    changes::push(&project.dir, &branch, &env)
        .await
        .map_err(|e| publish_step_error(e, true))?;
    let shared = match state
        .github
        .find_pr(&repo, &branch)
        .await
        .map_err(|e| publish_step_error(e, false))?
    {
        Some(existing) => existing,
        None => {
            let title = pr_title(&change.title);
            let body = pr_body(&change.title);
            state
                .github
                .create_draft_pr(&repo, &branch, &title, &body)
                .await
                .map_err(|e| publish_step_error(e, false))?
        }
    };
    // A draft cannot be merged; this is the same step the ready route takes.
    if shared.draft {
        state
            .github
            .mark_ready_for_review(&repo, &branch)
            .await
            .map_err(|e| publish_step_error(e, false))?;
    }
    let pr = state
        .github
        .merge_pr(&repo, &branch)
        .await
        .map_err(|e| publish_step_error(e, false))?;
    Ok(Json(json!({ "pr": pr })))
}

/// Writer-plain copy for a failed publish step: git/GitHub details never reach the
/// Publish button. Already-plain publish messages pass through unchanged.
fn publish_step_error(e: ApiError, push_failed: bool) -> ApiError {
    if e.message.starts_with("Couldn't publish") || e.message.starts_with("Ask an owner") {
        return e;
    }
    if push_failed {
        ApiError::conflict("Couldn't publish. Get latest, then try again.")
    } else {
        ApiError::github("Couldn't publish. Try again in a moment.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_step_errors_hide_git_and_github_details() {
        let push = publish_step_error(ApiError::git("git push failed: rejected"), true);
        assert_eq!(push.message, "Couldn't publish. Get latest, then try again.");
        let ready = publish_step_error(
            ApiError::github("GitHub could not mark the pull request ready for review"),
            false,
        );
        assert_eq!(ready.message, "Couldn't publish. Try again in a moment.");
        assert!(!ready.message.contains("pull request"));
    }

    #[test]
    fn publish_step_errors_keep_plain_publish_copy() {
        let owner = publish_step_error(
            ApiError::sandbox("Ask an owner to add you to this site, then try again."),
            false,
        );
        assert_eq!(owner.message, "Ask an owner to add you to this site, then try again.");
        let already = publish_step_error(ApiError::conflict("Couldn't publish. Get latest, then try again."), true);
        assert_eq!(already.message, "Couldn't publish. Get latest, then try again.");
    }

    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use crate::config::{Config, GithubConfig};

    /// State signed in to a mock GitHub whose `main` is unprotected; counts lookups.
    async fn state_with_unprotected_main(dir: &std::path::Path) -> (AppState, Arc<AtomicUsize>) {
        use axum::routing::get;
        use axum::Router;

        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let routes = Router::new()
            .route(
                "/repos/acme/site/branches/main/protection",
                get(move || {
                    h.fetch_add(1, Ordering::SeqCst);
                    async {
                        (
                            StatusCode::NOT_FOUND,
                            Json(json!({ "message": "Branch not protected" })),
                        )
                    }
                }),
            )
            .route(
                "/repos/acme/site/rules/branches/main",
                get(|| async { Json(json!([])) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, routes).await.ok() });

        let state_dir = dir.join(crate::projects::STATE_DIR);
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(
            state_dir.join("github.json"),
            json!({
                "token": "tok",
                "user": { "login": "ada", "id": 42, "name": null, "avatarUrl": "" }
            })
            .to_string(),
        )
        .unwrap();
        let config = Config {
            projects_dir: dir.to_path_buf(),
            no_auth: true,
            github: GithubConfig {
                api_url: base.clone(),
                web_url: base,
                gh_path: std::path::PathBuf::new(),
                ..GithubConfig::default()
            },
            ..Default::default()
        };
        (AppState::with_scaffold_install(config, false), hits)
    }

    #[tokio::test]
    async fn protection_hint_links_settings_and_the_bundled_docs() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, hits) = state_with_unprotected_main(tmp.path()).await;
        let p = state.projects.create("Site").await.unwrap();

        let hint = protection_hint_for(&state, &p.id, "acme/site").await;
        let base = state.config.github.web_url.clone();
        assert_eq!(
            hint,
            json!({
                "settingsUrl": format!("{base}/acme/site/settings/branches"),
                "docsUrl": PROTECTION_DOCS_PATH,
            })
        );
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn protection_hint_reads_the_dismissal_at_lookup_time() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, hits) = state_with_unprotected_main(tmp.path()).await;
        let p = state.projects.create("Site").await.unwrap();
        // Dismissed after the share started (e.g. during the push): no hint, no lookup.
        let snapshot = state.projects.get(&p.id).await.unwrap();
        assert!(!snapshot.protection_hint_dismissed);
        state.projects.dismiss_protection_hint(&p.id).await.unwrap();

        assert_eq!(
            protection_hint_for(&state, &p.id, "acme/site").await,
            Value::Null
        );
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}
