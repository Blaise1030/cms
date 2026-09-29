//! HTTP rendering of [`ApiError`] and the router fallback.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::error::ApiError;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self.body())).into_response()
    }
}

/// Unknown routes return the uniform envelope.
pub async fn not_found() -> ApiError {
    ApiError::enoent("route not found")
}

/// Router-generated 405s (e.g. `POST /api/health`) use the uniform envelope.
pub async fn method_not_allowed() -> ApiError {
    ApiError::badreq("method not allowed").with_status(405)
}
