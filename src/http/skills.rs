use super::query_or_badreq;
use crate::error::ApiError;
use crate::skills::{
    default_global_skills_dir, load_global_root, save_global_root, InstallSkill, SaveSkill,
    SkillDoc, SkillMeta, SkillScope, SkillsList, SkillsSettings,
};
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct ListQuery {
    project: String,
}

#[derive(Deserialize)]
pub struct SkillQuery {
    project: String,
    scope: SkillScope,
}

#[derive(Deserialize)]
pub struct UninstallQuery {
    project: String,
    scope: SkillScope,
    #[serde(default)]
    branch: Option<String>,
}

#[derive(Deserialize)]
struct SaveSettings {
    #[serde(rename = "globalPath")]
    global_path: String,
}

/// `GET /api/skills?project=` lists installed skills and the built-in catalog.
pub async fn list(
    State(state): State<AppState>,
    q: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<SkillsList>, ApiError> {
    let q = query_or_badreq(q)?;
    Ok(Json(crate::skills::list(&state, &q.project).await?))
}

/// `GET /api/skills/:id?project=&scope=` reads one installed skill.
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
    q: Result<Query<SkillQuery>, QueryRejection>,
) -> Result<Json<SkillDoc>, ApiError> {
    let q = query_or_badreq(q)?;
    Ok(Json(crate::skills::get(&state, &q.project, &id, q.scope).await?))
}

/// `PUT /api/skills/:id` saves front matter and body for an installed skill.
pub async fn save(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<SkillDoc>, ApiError> {
    let req: SaveSkill = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!(
            "expected JSON body {{project, branch?, scope, name?, description?, body, hash}}: {e}"
        ))
    })?;
    Ok(Json(crate::skills::save(&state, &id, req).await?))
}

/// `POST /api/skills/install` installs a skill into a scope.
pub async fn install(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<SkillMeta>, ApiError> {
    let req: InstallSkill = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!("expected JSON body {{project, branch?, scope, source}}: {e}"))
    })?;
    Ok(Json(crate::skills::install(&state, req).await?))
}

/// `DELETE /api/skills/:id?project=&scope=&branch=` removes an installed skill.
pub async fn uninstall(
    State(state): State<AppState>,
    Path(id): Path<String>,
    q: Result<Query<UninstallQuery>, QueryRejection>,
) -> Result<Json<()>, ApiError> {
    let q = query_or_badreq(q)?;
    crate::skills::uninstall(&state, &id, &q.project, q.scope, q.branch.as_deref()).await?;
    Ok(Json(()))
}

/// `GET /api/skills/settings` reads the global skills root.
pub async fn settings_get(State(state): State<AppState>) -> Json<SkillsSettings> {
    let projects_dir = state.config.projects_dir.clone();
    let default = default_global_skills_dir(&projects_dir);
    let global_path = load_global_root(&projects_dir);
    Json(SkillsSettings {
        global_path,
        default_global_path: default,
    })
}

/// `PUT /api/skills/settings` sets the global skills root; an empty path resets it.
pub async fn settings_put(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<SkillsSettings>, ApiError> {
    let req: SaveSettings = serde_json::from_slice(&body)
        .map_err(|e| ApiError::badreq(format!("expected JSON body {{globalPath}}: {e}")))?;
    let projects_dir = state.config.projects_dir.clone();
    let default = default_global_skills_dir(&projects_dir);
    let global_path = if req.global_path.is_empty() {
        default.clone()
    } else {
        PathBuf::from(&req.global_path)
    };
    save_global_root(&projects_dir, &global_path)?;
    Ok(Json(SkillsSettings {
        global_path,
        default_global_path: default,
    }))
}
