//! Terminal HTTP + WebSocket: provider registry, PTY lifecycle, hook checkpoints.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast::error::RecvError;

use super::{checkpoints, project_dir};
use crate::auth;
use crate::error::ApiError;
use crate::projects::STATE_DIR;
use crate::providers;
use crate::state::AppState;
use crate::terminal::Output;

const SUMMARY_MAX: usize = 72;

static TURN_END_SERIAL: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn turn_end_lock(project_id: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut map = TURN_END_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    map.entry(project_id.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// One-line commit subject for terminal hook checkpoints.
pub fn commit_message(summary: Option<&str>, changed: &[String]) -> String {
    let first = summary
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.lines().next().map(str::trim))
        .filter(|s| !s.is_empty());
    match first {
        Some(line) => truncate_summary(line),
        None => {
            let n = changed.len();
            let paths = changed
                .iter()
                .take(3)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            format!("Update {n} files: {paths}")
        }
    }
}

fn truncate_summary(line: &str) -> String {
    if line.chars().count() <= SUMMARY_MAX {
        return line.to_string();
    }
    let mut out = String::new();
    for (i, ch) in line.chars().enumerate() {
        if i >= SUMMARY_MAX - 1 {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

/// `GET /api/terminal/providers` → `{ providers: [{ id, command }] }`.
pub async fn list_providers(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let providers = providers::load(state.config.agents_config.as_deref())?;
    Ok(Json(json!({ "providers": providers })))
}

#[derive(Deserialize)]
struct StartRequest {
    project: String,
    provider: String,
}

/// `POST /api/terminal` `{ project, provider }` → `{ running: true }`.
pub async fn start(State(state): State<AppState>, body: Bytes) -> Result<Json<Value>, ApiError> {
    let req: StartRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::badreq(format!("expected JSON body {{project, provider}}: {e}")))?;
    let project = state
        .projects
        .get(&req.project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {}", req.project)))?;
    checkpoints::require_change(&state, &req.project).await?;
    let providers = providers::load(state.config.agents_config.as_deref())?;
    let provider = providers
        .iter()
        .find(|p| p.id == req.provider)
        .ok_or_else(|| ApiError::badreq(format!("unknown provider: {}", req.provider)))?;
    let hook_exe = state.config.hook_exe_path();
    let state_dir = state.config.projects_dir.join(STATE_DIR);
    let launch = providers::launch_spec(provider, &hook_exe, project.auto_approve, &state_dir);
    state
        .terminals
        .start(&req.project, &req.provider, &project.dir, launch)
        .await?;
    Ok(Json(json!({ "running": true })))
}

/// `GET /api/terminal/{project}` → `{ running, provider? }`.
pub async fn status(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<Value>, ApiError> {
    match state.terminals.status(&project) {
        Some(provider) => Ok(Json(json!({ "running": true, "provider": provider }))),
        None => Ok(Json(json!({ "running": false }))),
    }
}

/// `DELETE /api/terminal/{project}` → `{ stopped: true }`.
pub async fn stop(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.terminals.kill(&project).await;
    Ok(Json(json!({ "stopped": true })))
}

/// `GET /api/terminal/{project}/ws` — PTY byte stream.
pub async fn ws(
    State(state): State<AppState>,
    Path(project): Path<String>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if !auth::ws_origin_allowed(origin, state.config.origin.as_deref()) {
        return Err(ApiError::unauth("origin not allowed").with_status(403));
    }
    let attach = state.terminals.attach(&project)?;
    let state = state.clone();
    let project_id = project;
    Ok(upgrade.on_upgrade(move |socket| pump_ws(socket, state, project_id, attach)))
}

async fn pump_ws(
    mut socket: WebSocket,
    state: AppState,
    project_id: String,
    attach: crate::terminal::Attach,
) {
    if socket
        .send(Message::Binary(attach.replay.into()))
        .await
        .is_err()
    {
        return;
    }
    let mut rx = attach.rx;
    let tm = state.terminals.clone();
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Ok(Output::Data(data)) => {
                    if socket
                        .send(Message::Binary(data.to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Output::Exit(code)) => {
                    let msg = json!({ "exit": true, "code": code }).to_string();
                    let _ = socket.send(Message::Text(msg.into())).await;
                    break;
                }
                Err(RecvError::Lagged(_)) => break,
                Err(RecvError::Closed) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Binary(data))) => {
                    let _ = tm.write(&project_id, data.to_vec());
                }
                Some(Ok(Message::Text(text))) => {
                    if let Ok(v) = serde_json::from_str::<ResizeMsg>(&text) {
                        let _ = tm.resize(&project_id, v.cols, v.rows);
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
}

#[derive(Deserialize)]
struct ResizeMsg {
    cols: u16,
    rows: u16,
}

/// `POST /api/terminal/turn-end` — hook-authenticated checkpoint boundary.
#[derive(Deserialize)]
struct TurnEndRequest {
    project: String,
    token: String,
    summary: Option<String>,
}

pub async fn turn_end(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let req: TurnEndRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::badreq(format!("expected JSON body {{project, token, summary?}}: {e}")))?;
    if !state.terminals.token_matches(&req.project, &req.token) {
        return Err(ApiError::unauth("invalid hook token").with_status(403));
    }
    let lock = turn_end_lock(&req.project);
    let _guard = lock.lock().await;

    if let Err(e) = checkpoints::require_change(&state, &req.project).await {
        state.events.emit(
            "checkpoint.blocked",
            json!({
                "projectId": req.project,
                "reason": e.message,
            }),
        );
        return Ok(Json(json!({ "committed": false })));
    }

    let dir = project_dir(&state, &req.project).await?;
    let changed = crate::git::changed_paths(&dir).await?;
    let message = commit_message(req.summary.as_deref(), &changed);
    match crate::git::checkpoint(&dir, &message).await? {
        Some(cp) => {
            state.events.emit(
                "checkpoint.created",
                json!({
                    "projectId": req.project,
                    "sha": cp.sha,
                    "summary": cp.summary,
                    "author": cp.author,
                    "ts": cp.ts,
                }),
            );
            Ok(Json(json!({ "committed": true })))
        }
        None => Ok(Json(json!({ "committed": false }))),
    }
}
