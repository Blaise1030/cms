use super::{project_dir, query_or_badreq};
use crate::content::catalog;
use crate::content::mutation::{self, SaveEntry, SaveTarget};
use crate::{error::ApiError, state::AppState};
use axum::body::Bytes;
use axum::extract::{rejection::QueryRejection, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};
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

#[derive(Deserialize)]
struct CreateBody {
    project: String,
    collection: String,
    slug: String,
    branch: String,
    fields: Map<String, Value>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    unset: Vec<String>,
}
#[derive(Deserialize)]
struct UpdateBody {
    project: String,
    collection: String,
    id: String,
    branch: String,
    hash: String,
    fields: Map<String, Value>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    unset: Vec<String>,
}
fn parse<T: serde::de::DeserializeOwned>(body: &Bytes, shape: &str) -> Result<T, ApiError> {
    serde_json::from_slice(body)
        .map_err(|e| ApiError::badreq(format!("expected JSON body {{{shape}}}: {e}")))
}
/// `POST /api/content/entry` creates an entry on the active change.
pub async fn create(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<catalog::ContentEntry>, ApiError> {
    let b: CreateBody = parse(
        &body,
        "project, collection, slug, branch, fields, body?, unset?",
    )?;
    let request = SaveEntry {
        project: b.project,
        collection: b.collection,
        branch: b.branch,
        target: SaveTarget::Create { slug: b.slug },
        fields: b.fields,
        body: b.body,
        unset: b.unset,
    };
    Ok(Json(mutation::save_entry(&state, request).await?))
}
/// `PUT /api/content/entry` updates the entry opened at `branch` with content `hash`.
pub async fn update(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<catalog::ContentEntry>, ApiError> {
    let b: UpdateBody = parse(
        &body,
        "project, collection, id, branch, hash, fields, body?, unset?",
    )?;
    let request = SaveEntry {
        project: b.project,
        collection: b.collection,
        branch: b.branch,
        target: SaveTarget::Update {
            id: b.id,
            hash: b.hash,
        },
        fields: b.fields,
        body: b.body,
        unset: b.unset,
    };
    Ok(Json(mutation::save_entry(&state, request).await?))
}
