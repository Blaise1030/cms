//! `WS /ws/events` — live event stream (chat, permissions, checkpoints, ...).
//!
//! Auth is the same middleware as REST (`?t=<token>`, cookie or Bearer). Additionally the
//! browser `Origin` must be allowed, so a random web page cannot hijack the socket.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, HeaderMap};
use axum::response::Response;
use serde_json::json;
use tokio::sync::broadcast::{self, error::RecvError};

use crate::auth;
use crate::error::ApiError;
use crate::state::AppState;

pub async fn ws(
    State(state): State<AppState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if !auth::ws_origin_allowed(origin, state.config.origin.as_deref()) {
        return Err(ApiError::unauth("origin not allowed").with_status(403));
    }
    // Subscribe before the upgrade completes so nothing emitted meanwhile is lost.
    let rx = state.events.subscribe();
    Ok(upgrade.on_upgrade(move |socket| pump(socket, rx)))
}

async fn pump(mut socket: WebSocket, mut rx: broadcast::Receiver<std::sync::Arc<str>>) {
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(text) => {
                    if socket.send(Message::Text(text.as_ref().into())).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(missed)) => {
                    let note = json!({ "type": "events.lagged", "missed": missed }).to_string();
                    if socket.send(Message::Text(note.into())).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Closed) => break,
            },
            incoming = socket.recv() => match incoming {
                // Clients only listen; reading keeps pings/close frames flowing.
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
}
