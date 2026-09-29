//! HTTP layer: router, middleware wiring.

pub mod error;
pub mod health;
pub mod projects;

use axum::http::{header, HeaderValue, Method};
use axum::routing::{delete, get};
use axum::{middleware, Router};
use tower_http::cors::CorsLayer;

use crate::auth;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let mut app = Router::new()
        .route("/api/health", get(health::health))
        .route("/api/projects", get(projects::list).post(projects::create))
        .route("/api/projects/{id}", delete(projects::remove))
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
