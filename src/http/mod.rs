//! HTTP layer: router, middleware wiring.

pub mod checkpoints;
pub mod error;
pub mod files;
pub mod health;
pub mod projects;

use std::path::PathBuf;

use axum::extract::rejection::QueryRejection;
use axum::extract::Query;
use axum::http::{header, HeaderValue, Method};
use axum::routing::{delete, get, post};
use axum::{middleware, Router};
use tower_http::cors::CorsLayer;

use crate::auth;
use crate::error::ApiError;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let mut app = Router::new()
        .route("/api/health", get(health::health))
        .route("/api/projects", get(projects::list).post(projects::create))
        .route("/api/projects/{id}", delete(projects::remove))
        .route("/api/checkpoints", get(checkpoints::list))
        .route("/api/checkpoints/{sha}/diff", get(checkpoints::diff))
        .route("/api/checkpoints/{sha}/restore", post(checkpoints::restore))
        .route("/api/tree", get(files::tree))
        .route("/api/file", get(files::file))
        .fallback(error::not_found)
        .method_not_allowed_fallback(error::method_not_allowed)
        // Applied bottom-up: host check runs first, then auth.
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::check_host,
        ));

    // CORS: only `--origin`.
    if let Some(origin) = state
        .config
        .origin
        .as_deref()
        .and_then(|o| HeaderValue::from_str(o.trim_end_matches('/')).ok())
    {
        app = app.layer(
            CorsLayer::new()
                .allow_origin(origin)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
                .allow_credentials(true),
        );
    }
    app.with_state(state)
}

/// Directory of a registered project, or `ENOENT`.
pub(crate) async fn project_dir(state: &AppState, id: &str) -> Result<PathBuf, ApiError> {
    state
        .projects
        .get(id)
        .await
        .map(|p| p.dir)
        .ok_or_else(|| ApiError::enoent(format!("no such project: {id}")))
}

/// Query-string rejections become the uniform error envelope.
pub(crate) fn query_or_badreq<T>(q: Result<Query<T>, QueryRejection>) -> Result<T, ApiError> {
    q.map(|Query(v)| v)
        .map_err(|e| ApiError::badreq(format!("invalid query: {e}")))
}
