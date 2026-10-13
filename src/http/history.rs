use super::content::ContentQuery;
use super::{project_dir, query_or_badreq};
use crate::content::catalog;
use crate::{error::ApiError, git, state::AppState};
use axum::extract::{rejection::QueryRejection, Path, Query, State};
use axum::Json;
use serde_json::{json, Value};

/// The project root and the entry the query names.
async fn resolve(
    state: &AppState,
    q: ContentQuery,
) -> Result<(std::path::PathBuf, catalog::ContentEntry), ApiError> {
    let root = project_dir(state, &q.project).await?;
    let name = q
        .collection
        .ok_or_else(|| ApiError::badreq("collection is required"))?;
    let id = q.id.ok_or_else(|| ApiError::badreq("id is required"))?;
    let entry = catalog::read_entry(&root, &name, &id).await?;
    Ok((root, entry))
}

/// `GET /api/content/history` → `{versions}`, newest first.
pub async fn list(
    State(state): State<AppState>,
    q: Result<Query<ContentQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let (root, entry) = resolve(&state, query_or_badreq(q)?).await?;
    Ok(Json(
        json!({"versions": git::file_log(&root, &entry.path).await?}),
    ))
}

/// `GET /api/content/history/{sha}` → the entry as of that version; `sha` may be a prefix.
pub async fn version(
    State(state): State<AppState>,
    Path(sha): Path<String>,
    q: Result<Query<ContentQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let (root, entry) = resolve(&state, query_or_badreq(q)?).await?;
    let v = git::file_log(&root, &entry.path)
        .await?
        .into_iter()
        .find(|v| v.sha.starts_with(&sha))
        .ok_or_else(|| ApiError::enoent(format!("no version {sha} of this entry")))?;
    let source = git::file_at(&root, &v.sha, &v.path).await?;
    let old = catalog::build_entry(
        entry.collection,
        entry.id,
        v.path,
        &entry.kind,
        source,
        entry.branch,
    );
    let parseable = if old.kind == "json" {
        !old.fields.is_null()
    } else {
        old.fields.is_object()
    };
    Ok(Json(json!({
        "sha": v.sha,
        "path": old.path,
        "source": old.source,
        "fields": old.fields,
        "body": old.body,
        "parseable": parseable,
    })))
}
