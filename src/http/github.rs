//! `/api/github/*`: sign-in status, device flow, sign-out, repositories.

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::github::{DeviceStart, PollOutcome, Status};
use crate::state::AppState;

/// Make every project's commits authored as the current identity, unless they already are.
/// The applied-identity lock is held across the read and the write, so concurrent
/// sign-in, sign-out and status calls cannot leave the projects on a stale identity.
pub async fn apply_current(state: &AppState) {
    let mut applied = state.github.applied_guard().await;
    let current = state.github.identity().await;
    if applied.as_ref() == Some(&current) {
        return;
    }
    state.projects.apply_identity(&current).await;
    *applied = Some(current);
}

/// `GET /api/github/status` → `{ configured, deviceFlow, source, user | null }`. If the
/// GitHub CLI login appeared or changed since the projects were last updated, they get the
/// new identity.
pub async fn status(State(state): State<AppState>) -> Json<Status> {
    let status = state.github.status().await;
    apply_current(&state).await;
    Json(status)
}

/// `POST /api/github/device` → `{ userCode, verificationUri, expiresIn, interval }`.
pub async fn device_start(State(state): State<AppState>) -> Result<Json<DeviceStart>, ApiError> {
    Ok(Json(state.github.start_device().await?))
}

/// `POST /api/github/device/poll` → `{ status, user?, interval? }`. On success every
/// project's future commits are authored by the new user.
pub async fn device_poll(State(state): State<AppState>) -> Result<Json<PollOutcome>, ApiError> {
    let outcome = state.github.poll_device().await?;
    if matches!(outcome, PollOutcome::SignedIn { .. }) {
        apply_current(&state).await;
    }
    Ok(Json(outcome))
}

/// `POST /api/github/signout`. Projects go back to the `Workbench` identity.
pub async fn sign_out(State(state): State<AppState>) -> Json<Value> {
    state.github.sign_out().await;
    apply_current(&state).await;
    Json(json!({ "signedOut": true }))
}

/// `GET /api/github/repos` → `{ repos: [{ fullName, private, defaultBranch, canPush }] }`.
pub async fn repos(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let (result, cleared) = state.github.repos_checked().await;
    if cleared {
        // This call dropped an expired session: projects follow the identity that remains.
        apply_current(&state).await;
    }
    Ok(Json(json!({ "repos": result? })))
}
