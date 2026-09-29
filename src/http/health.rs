//! `GET /api/health` (no auth).

use axum::Json;
use serde_json::{json, Value};

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": crate::version() }))
}
