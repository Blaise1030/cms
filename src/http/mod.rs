//! HTTP layer: router, middleware wiring.

pub mod changes;
pub mod checkpoints;
pub mod content;
pub mod design;
pub mod devserver;
pub mod error;
pub mod events;
pub mod files;
pub mod github;
pub mod health;
pub mod history;
pub mod projects;
pub mod terminal;

use std::path::PathBuf;

use axum::extract::rejection::QueryRejection;
use axum::extract::{DefaultBodyLimit, Query};
use axum::http::{header, HeaderValue, Method};
use axum::routing::{any, delete, get, post};
use axum::{middleware, Router};
use tower_http::cors::CorsLayer;

use crate::auth;
use crate::error::ApiError;
use crate::preview;
use crate::state::AppState;
use crate::static_files;

pub fn router(state: AppState) -> Router {
    let mut app = Router::new()
        .route("/api/health", get(health::health))
        .route("/api/content/collections", get(content::collections))
        .route("/api/content/entries", get(content::entries))
        .route("/api/content/entry/rename", post(content::rename))
        .route("/api/content/entry/impact", post(content::impact))
        .route("/api/design", get(design::get).put(design::put))
        .route("/api/content/history", get(history::list))
        .route("/api/content/history/{sha}", get(history::version))
        .route(
            "/api/content/entry",
            get(content::entry)
                .post(content::create)
                .put(content::update)
                .delete(content::delete)
                .layer(DefaultBodyLimit::max(
                    crate::content::images::MAX_SAVE_BYTES,
                )),
        )
        .route("/api/projects", get(projects::list).post(projects::create))
        .route("/api/github/clone", post(projects::clone))
        .route("/api/github/status", get(github::status))
        .route("/api/github/device", post(github::device_start))
        .route("/api/github/device/poll", post(github::device_poll))
        .route("/api/github/signout", post(github::sign_out))
        .route("/api/github/repos", get(github::repos))
        .route(
            "/api/projects/{id}",
            delete(projects::remove).patch(projects::update),
        )
        .route(
            "/api/projects/{id}/protection-hint/dismiss",
            post(projects::dismiss_protection_hint),
        )
        .route(
            "/api/projects/{id}/devserver/restart",
            post(devserver::restart),
        )
        .route("/preview/{id}", any(preview::handle))
        .route("/preview/{id}/", any(preview::handle))
        .route("/preview/{id}/{*rest}", any(preview::handle))
        // Astro hardcodes these at the site root even when `base` is `/preview/{id}/`.
        .route("/@vite", any(preview::handle_vite_root))
        .route("/@vite/{*rest}", any(preview::handle_vite_root))
        .route("/@id/{*rest}", any(preview::handle_vite_root))
        .route("/@fs/{*rest}", any(preview::handle_vite_root))
        .route("/node_modules/{*rest}", any(preview::handle_vite_root))
        .route("/src/{*rest}", any(preview::handle_vite_root))
        .route("/api/terminal/providers", get(terminal::list_providers))
        .route("/api/terminal", post(terminal::start))
        .route("/api/terminal/turn-end", post(terminal::turn_end))
        .route(
            "/api/terminal/{project}",
            get(terminal::status).delete(terminal::stop),
        )
        .route("/api/terminal/{project}/ws", get(terminal::ws))
        .route("/api/terminal/{project}/sessions", get(terminal::sessions))
        .route(
            "/api/terminal/{project}/attachments",
            post(terminal::attach)
                .layer(DefaultBodyLimit::max(terminal::MAX_ATTACHMENT_BYTES)),
        )
        .route("/ws/events", get(events::ws))
        .route("/api/checkpoints", get(checkpoints::list))
        .route("/api/checkpoints/{sha}/diff", get(checkpoints::diff))
        .route("/api/checkpoints/{sha}/restore", post(checkpoints::restore))
        .route("/api/changes", get(changes::list).post(changes::create))
        .route("/api/changes/active", get(changes::active))
        .route("/api/changes/update", post(changes::update))
        .route("/api/changes/conflicts", get(changes::list_conflicts))
        .route("/api/changes/conflicts/{id}", get(changes::get_conflict))
        .route(
            "/api/changes/conflicts/{id}/previews",
            get(changes::conflict_previews),
        )
        .route(
            "/api/changes/conflicts/{id}/choice",
            post(changes::choose_conflict),
        )
        .route(
            "/api/changes/conflicts/{id}/finish",
            post(changes::finish_conflict),
        )
        .route(
            "/api/changes/conflicts/{id}/abandon",
            post(changes::abandon_conflict),
        )
        .route("/api/changes/{branch}/switch", post(changes::switch))
        .route("/api/changes/{branch}/share", post(changes::share))
        .route("/api/changes/{branch}/status", get(changes::status))
        .route(
            "/api/docs/branch-protection",
            get(changes::branch_protection_doc),
        )
        .route("/api/changes/{branch}/cleanup", post(changes::cleanup))
        .route("/api/changes/{branch}/ready", post(changes::ready))
        .route("/api/tree", get(files::tree))
        .route("/api/file", get(files::file))
        .fallback(static_files::serve)
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
