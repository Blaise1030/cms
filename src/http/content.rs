use super::{project_dir, query_or_badreq};
use crate::{content::catalog, error::ApiError, state::AppState};
use axum::extract::{rejection::QueryRejection, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Deserialize)]
pub struct ContentQuery {
    project: String,
    collection: Option<String>,
    id: Option<String>,
}
pub async fn collections(
    State(state): State<AppState>,
    q: Result<Query<ContentQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let root = project_dir(&state, &q.project).await?;
    Ok(Json(
        json!({"collections":catalog::list_collections(&root).await?}),
    ))
}
pub async fn entries(
    State(state): State<AppState>,
    q: Result<Query<ContentQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let q = query_or_badreq(q)?;
    let root = project_dir(&state, &q.project).await?;
    let name = q
        .collection
        .ok_or_else(|| ApiError::badreq("collection is required"))?;
    Ok(Json(
        json!({"entries":catalog::list_entries(&root,&name).await?}),
    ))
}
pub async fn entry(
    State(state): State<AppState>,
    q: Result<Query<ContentQuery>, QueryRejection>,
) -> Result<Json<catalog::ContentEntry>, ApiError> {
    let q = query_or_badreq(q)?;
    let root = project_dir(&state, &q.project).await?;
    let name = q
        .collection
        .ok_or_else(|| ApiError::badreq("collection is required"))?;
    let id = q.id.ok_or_else(|| ApiError::badreq("id is required"))?;
    Ok(Json(catalog::read_entry(&root, &name, &id).await?))
}
