//! `/api/github/*`: sign-in status, device flow, sign-out, repositories.

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::git::Identity;
use crate::github::{DeviceStart, PollOutcome, Status};
use crate::state::AppState;

/// Make every project's commits authored as `identity`, and remember that.
async fn apply(state: &AppState, identity: &Identity) {
    state.github.note_applied(identity).await;
    state.projects.apply_identity(identity).await;
}

/// `GET /api/github/status` → `{ configured, deviceFlow, source, user | null }`. If the
/// GitHub CLI login appeared or changed since the projects were last updated, they get the
/// new identity.
pub async fn status(State(state): State<AppState>) -> Json<Status> {
    let status = state.github.status().await;
    if let Some(identity) = state.github.identity_change().await {
        state.projects.apply_identity(&identity).await;
    }
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
    if let PollOutcome::SignedIn { user } = &outcome {
        let applied = user.identity();
        apply(&state, &applied).await;
        // A sign-out may have raced in; make the projects match the current session.
        let current = state.github.identity().await;
        if current != applied {
            apply(&state, &current).await;
        }
    }
    Ok(Json(outcome))
}

/// `POST /api/github/signout`. Projects go back to the `Workbench` identity.
pub async fn sign_out(State(state): State<AppState>) -> Json<Value> {
    state.github.sign_out().await;
    let applied = Identity::workbench();
    apply(&state, &applied).await;
    // A sign-in may have raced in; make the projects match the current session.
    let current = state.github.identity().await;
    if current != applied {
        apply(&state, &current).await;
    }
    Json(json!({ "signedOut": true }))
}

/// `GET /api/github/repos` → `{ repos: [{ fullName, private, defaultBranch, canPush }] }`.
pub async fn repos(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let (result, cleared) = state.github.repos_checked().await;
    if cleared {
        // This call dropped an expired session; projects revert to `Workbench`.
        let applied = Identity::workbench();
        apply(&state, &applied).await;
        let current = state.github.identity().await;
        if current != applied {
            apply(&state, &current).await;
        }
    }
    Ok(Json(json!({ "repos": result? })))
}
