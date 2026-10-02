//! `/api/github/*`: sign-in status, device flow, sign-out, repositories.

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::git::Identity;
use crate::github::{DeviceStart, PollOutcome, Status};
use crate::state::AppState;

/// `GET /api/github/status` → `{ configured, user | null }`.
pub async fn status(State(state): State<AppState>) -> Json<Status> {
    Json(state.github.status().await)
}

/// `POST /api/github/device` → `{ userCode, verificationUri, expiresIn, interval }`.
pub async fn device_start(State(state): State<AppState>) -> Result<Json<DeviceStart>, ApiError> {
    Ok(Json(state.github.start_device().await?))
}

/// `POST /api/github/device/poll` → `{ status, user?, interval? }`. On success every
/// project's future commits are authored by the new user.
pub async fn device_poll(State(state): State<AppState>) -> Result<Json<PollOutcome>, ApiError> {
    let outcome = state.github.poll_device().await?;
    if let PollOutcome::SignedIn { user } = &outcome {
        let applied = user.identity();
        state.projects.apply_identity(&applied).await;
        // A sign-out may have raced in; make the projects match the current session.
        let current = state.github.identity().await;
        if current != applied {
            state.projects.apply_identity(&current).await;
        }
    }
    Ok(Json(outcome))
}

/// `POST /api/github/signout`. Projects go back to the `Workbench` identity.
pub async fn sign_out(State(state): State<AppState>) -> Json<Value> {
    state.github.sign_out().await;
    let applied = Identity::workbench();
    state.projects.apply_identity(&applied).await;
    // A sign-in may have raced in; make the projects match the current session.
    let current = state.github.identity().await;
    if current != applied {
        state.projects.apply_identity(&current).await;
    }
    Json(json!({ "signedOut": true }))
}

/// `GET /api/github/repos` → `{ repos: [{ fullName, private, defaultBranch, canPush }] }`.
pub async fn repos(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    match state.github.repos().await {
        Ok(repos) => Ok(Json(json!({ "repos": repos }))),
        Err(e) => {
            // An expired session is cleared by `repos`; projects revert to `Workbench`.
            if state.github.status().await.user.is_none() {
                state
                    .projects
                    .apply_identity(&state.github.identity().await)
                    .await;
            }
            Err(e)
        }
    }
}
