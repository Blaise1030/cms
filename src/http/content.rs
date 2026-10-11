use super::{project_dir, query_or_badreq};
use crate::content::catalog;
use crate::content::images::{self, ImageMeta, Upload};
use crate::content::mutation::{self, DeleteEntry, RenameEntry, SaveEntry, SaveTarget};
use crate::{error::ApiError, state::AppState};
use axum::body::Bytes;
use axum::extract::{rejection::QueryRejection, FromRequest, Multipart, Query, Request, State};
use axum::http::header;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
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
    #[serde(default)]
    images: Vec<ImageMeta>,
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
    #[serde(default)]
    images: Vec<ImageMeta>,
}
fn parse<T: serde::de::DeserializeOwned>(body: &Bytes, shape: &str) -> Result<T, ApiError> {
    serde_json::from_slice(body)
        .map_err(|e| ApiError::badreq(format!("expected JSON body {{{shape}}}: {e}")))
}
fn too_big(e: impl std::fmt::Display) -> ApiError {
    ApiError::badreq(format!(
        "the save is too large ({e}); attach at most {} MiB of images at once",
        images::MAX_SAVE_BYTES >> 20
    ))
    .with_status(413)
    .with_reason("request_too_large")
}

/// A create/update request: a JSON body, or `multipart/form-data` with a JSON `entry`
/// part and one part per image named by its client ID. Images are read with a per-file
/// cap so an oversized one is refused without buffering it.
async fn save_request<T: serde::de::DeserializeOwned>(
    req: Request,
    shape: &str,
    meta: impl Fn(&mut T) -> Vec<ImageMeta>,
) -> Result<(T, Vec<Upload>), ApiError> {
    let multipart = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.trim_start()
                .to_ascii_lowercase()
                .starts_with("multipart/form-data")
        });
    if !multipart {
        let body = Bytes::from_request(req, &()).await.map_err(|e| {
            if e.status() == 413 {
                too_big(e.body_text())
            } else {
                ApiError::badreq(e.body_text())
            }
        })?;
        let mut b: T = parse(&body, shape)?;
        // JSON cannot carry image bytes; any image metadata is reported as missing.
        let uploads = images::collect_uploads(meta(&mut b), HashMap::new())?;
        return Ok((b, uploads));
    }
    let form_error = |e: axum::extract::multipart::MultipartError| {
        if e.status() == 413 {
            too_big(e.body_text())
        } else {
            ApiError::badreq(format!("invalid multipart body: {}", e.body_text()))
        }
    };
    let mut form = Multipart::from_request(req, &())
        .await
        .map_err(|e| ApiError::badreq(e.body_text()))?;
    let mut entry: Option<T> = None;
    let mut parts = HashMap::new();
    while let Some(mut field) = form.next_field().await.map_err(form_error)? {
        let name = field.name().unwrap_or_default().to_string();
        if name == "entry" {
            if entry.is_some() {
                return Err(ApiError::badreq("multipart body has two entry parts"));
            }
            entry = Some(parse(&field.bytes().await.map_err(form_error)?, shape)?);
            continue;
        }
        if !images::valid_client_id(&name) {
            return Err(
                ApiError::badreq(format!("unexpected multipart part {name:?}"))
                    .with_reason("invalid_image"),
            );
        }
        if parts.len() >= images::MAX_IMAGES || parts.contains_key(&name) {
            return Err(
                ApiError::badreq("too many or duplicate image parts").with_reason("invalid_image")
            );
        }
        let filename = field.file_name().unwrap_or(&name).to_string();
        let mut bytes = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(form_error)? {
            if bytes.len() + chunk.len() > images::MAX_IMAGE_BYTES {
                return Err(images::too_large(&filename));
            }
            bytes.extend_from_slice(&chunk);
        }
        parts.insert(name, bytes);
    }
    let mut b = entry.ok_or_else(|| ApiError::badreq("multipart body has no entry part"))?;
    let uploads = images::collect_uploads(meta(&mut b), parts)?;
    Ok((b, uploads))
}

/// `POST /api/content/entry` creates an entry on the active change.
pub async fn create(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<mutation::SavedEntry>, ApiError> {
    let (b, images): (CreateBody, _) = save_request(
        req,
        "project, collection, slug, branch, fields, body?, unset?, images?",
        |b: &mut CreateBody| std::mem::take(&mut b.images),
    )
    .await?;
    let request = SaveEntry {
        project: b.project,
        collection: b.collection,
        branch: b.branch,
        target: SaveTarget::Create { slug: b.slug },
        fields: b.fields,
        body: b.body,
        unset: b.unset,
        images,
    };
    Ok(Json(mutation::save_entry(&state, request).await?))
}
/// `PUT /api/content/entry` updates the entry opened at `branch` with content `hash`.
pub async fn update(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<mutation::SavedEntry>, ApiError> {
    let (b, images): (UpdateBody, _) = save_request(
        req,
        "project, collection, id, branch, hash, fields, body?, unset?, images?",
        |b: &mut UpdateBody| std::mem::take(&mut b.images),
    )
    .await?;
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
        images,
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
