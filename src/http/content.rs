use super::{project_dir, query_or_badreq};
use crate::content::catalog;
use crate::content::mutation::{self, DeleteEntry, RenameEntry, SaveEntry, SaveTarget};
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

#[derive(Deserialize)]
struct EntryRef {
    project: String,
    collection: String,
    id: String,
}
#[derive(Deserialize)]
struct RenameBody {
    project: String,
    collection: String,
    id: String,
    slug: String,
    branch: String,
    hash: String,
}
#[derive(Deserialize)]
struct DeleteBody {
    project: String,
    collection: String,
    id: String,
    branch: String,
    hash: String,
}
/// `POST /api/content/entry/impact` is the dry run for a rename or delete confirmation.
pub async fn impact(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<mutation::ContentImpact>, ApiError> {
    let b: EntryRef = parse(&body, "project, collection, id")?;
    let root = project_dir(&state, &b.project).await?;
    Ok(Json(
        mutation::inspect_impact(&root, &b.collection, &b.id).await?,
    ))
}
/// `POST /api/content/entry/rename` → `{entry, entries}` for the renamed collection.
pub async fn rename(State(state): State<AppState>, body: Bytes) -> Result<Json<Value>, ApiError> {
    let b: RenameBody = parse(&body, "project, collection, id, slug, branch, hash")?;
    let root = project_dir(&state, &b.project).await?;
    let collection = b.collection.clone();
    let request = RenameEntry {
        project: b.project,
        collection: b.collection,
        id: b.id,
        slug: b.slug,
        branch: b.branch,
        hash: b.hash,
    };
    let entry = mutation::rename_entry(&state, request).await?;
    let entries = catalog::list_entries(&root, &collection).await?;
    Ok(Json(json!({ "entry": entry, "entries": entries })))
}
/// `DELETE /api/content/entry` → `{entries}` for the collection.
pub async fn delete(State(state): State<AppState>, body: Bytes) -> Result<Json<Value>, ApiError> {
    let b: DeleteBody = parse(&body, "project, collection, id, branch, hash")?;
    let root = project_dir(&state, &b.project).await?;
    let collection = b.collection.clone();
    let request = DeleteEntry {
        project: b.project,
        collection: b.collection,
        id: b.id,
        branch: b.branch,
        hash: b.hash,
    };
    mutation::delete_entry(&state, request).await?;
    let entries = catalog::list_entries(&root, &collection).await?;
    Ok(Json(json!({ "entries": entries })))
}
