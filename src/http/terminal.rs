//! Terminal HTTP + WebSocket: provider registry, PTY lifecycle, hook checkpoints.

use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast::error::RecvError;

use super::{changes, checkpoints, project_dir};
use crate::auth;
use crate::config::random_token;
use crate::error::ApiError;
use crate::projects::STATE_DIR;
use crate::providers::{self, AgentSession, Provider};
use crate::state::AppState;
use crate::terminal::history::{self, SessionRecord};
use crate::terminal::{login_command, Output};

const SUMMARY_MAX: usize = 72;
/// How long `cursor-agent create-chat` may take before the session starts without an id.
const CREATE_CHAT_TIMEOUT: Duration = Duration::from_secs(20);

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
    /// A [`history`] record to resume; its provider wins over `provider`.
    #[serde(default)]
    resume: Option<String>,
}

fn state_dir(state: &AppState) -> PathBuf {
    state.config.projects_dir.join(STATE_DIR)
}

/// `POST /api/terminal` `{ project, provider, resume? }` → `{ running: true, sessionId }`.
pub async fn start(State(state): State<AppState>, body: Bytes) -> Result<Json<Value>, ApiError> {
    let req: StartRequest = serde_json::from_slice(&body).map_err(|e| {
        ApiError::badreq(format!("expected JSON body {{project, provider, resume?}}: {e}"))
    })?;
    let project = state
        .projects
        .get(&req.project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {}", req.project)))?;
    let providers = providers::load(state.config.agents_config.as_deref())?;
    let find = |id: &str| {
        providers
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| ApiError::badreq(format!("unknown provider: {id}")))
    };
    let state_dir = state_dir(&state);
    let now = history::now_ms();
    let (provider, session, record) = match &req.resume {
        Some(id) => {
            let past = history::get(&state_dir, &req.project, id)
                .ok_or_else(|| ApiError::enoent(format!("no such session: {id}")))?;
            let agent_id = past
                .agent_session_id
                .clone()
                .filter(|_| providers::can_resume(&past.provider))
                .ok_or_else(|| ApiError::badreq("this session cannot be resumed"))?;
            (find(&past.provider)?, AgentSession::Resume(agent_id), past)
        }
        None => {
            let provider = find(&req.provider)?;
            let session = new_agent_session(provider, &project.dir).await;
            let record = SessionRecord {
                id: random_token()[..16].to_string(),
                provider: provider.id.clone(),
                agent_session_id: match &session {
                    AgentSession::Assign(id) => Some(id.clone()),
                    _ => None,
                },
                title: None,
                created_at: now,
                updated_at: now,
            };
            (provider, session, record)
        }
    };
    // Claude starts changes itself (see `CLAUDE_GIT_GUIDE`); other agents need one first.
    if provider.id != "claude" {
        checkpoints::require_change(&state, &req.project).await?;
    }
    let launch = providers::launch_spec(
        provider,
        &state.config.hook_exe_path(),
        project.auto_approve,
        &state_dir,
        &req.project,
        &session,
    );
    state
        .terminals
        .start(&req.project, &provider.id, &project.dir, launch)
        .await?;
    state.terminals.set_history_id(&req.project, &record.id);
    let session_id = record.id.clone();
    if let Err(e) = history::insert(&state_dir, &req.project, SessionRecord { updated_at: now, ..record }) {
        eprintln!("workbench: cannot save terminal session history: {e}");
    }
    Ok(Json(json!({ "running": true, "sessionId": session_id })))
}

/// A new conversation, with its id up front where the CLI allows it.
async fn new_agent_session(provider: &Provider, dir: &FsPath) -> AgentSession {
    match provider.id.as_str() {
        "claude" => AgentSession::Assign(uuid_v4()),
        "cursor" => create_cursor_chat(provider, dir)
            .await
            .map_or(AgentSession::Fresh, AgentSession::Assign),
        _ => AgentSession::Fresh,
    }
}

/// `<cursor argv> create-chat` prints the new chat's id as its last line.
async fn create_cursor_chat(provider: &Provider, dir: &FsPath) -> Option<String> {
    let mut argv = provider.command.clone();
    argv.push("create-chat".into());
    let mut cmd = login_command(&argv, dir);
    cmd.kill_on_drop(true).stdin(std::process::Stdio::null());
    let out = tokio::time::timeout(CREATE_CHAT_TIMEOUT, cmd.output())
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let id = text.lines().rev().map(str::trim).find(|l| !l.is_empty())?;
    let plausible = (8..=64).contains(&id.len())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    plausible.then(|| id.to_string())
}

/// A random (version 4) UUID, the form Claude's `--session-id` requires.
fn uuid_v4() -> String {
    let mut hex: Vec<u8> = random_token().into_bytes();
    hex.truncate(32);
    hex[12] = b'4';
    hex[16] = b"89ab"[(hex[16] as usize) % 4];
    let s = String::from_utf8(hex).unwrap_or_default();
    format!("{}-{}-{}-{}-{}", &s[..8], &s[8..12], &s[12..16], &s[16..20], &s[20..])
}

/// `GET /api/terminal/{project}` → `{ running, provider?, sessionId? }`.
pub async fn status(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<Value>, ApiError> {
    match state.terminals.status(&project) {
        Some(provider) => Ok(Json(json!({
            "running": true,
            "provider": provider,
            "sessionId": state.terminals.history_id(&project),
        }))),
        None => Ok(Json(json!({ "running": false }))),
    }
}

/// `GET /api/terminal/{project}/sessions` → `{ sessions: [{ id, provider, title?,
/// createdAt, updatedAt, resumable }] }`, most recently active first.
pub async fn sessions(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state
        .projects
        .get(&project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {project}")))?;
    let sessions: Vec<Value> = history::list(&state_dir(&state), &project)
        .into_iter()
        .map(|r| {
            let resumable = r.agent_session_id.is_some() && providers::can_resume(&r.provider);
            json!({
                "id": r.id,
                "provider": r.provider,
                "title": r.title,
                "createdAt": r.created_at,
                "updatedAt": r.updated_at,
                "resumable": resumable,
            })
        })
        .collect();
    Ok(Json(json!({ "sessions": sessions })))
}

/// Largest image the agent tab accepts as an attachment.
pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;

/// `POST /api/terminal/{project}/attachments` with an `image/*` body → `{ path }`, an absolute
/// path the agent CLI can read. Kept in the daemon's state dir so checkpoints don't commit it.
pub async fn attach(
    State(state): State<AppState>,
    Path(project): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    state
        .projects
        .get(&project)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {project}")))?;
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let ext = match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => return Err(ApiError::badreq(format!("unsupported attachment type: {mime:?}"))),
    };
    let dir = state_dir(&state).join(&project).join("attachments");
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("{}.{ext}", &random_token()[..16]));
    tokio::fs::write(&path, &body).await?;
    Ok(Json(json!({ "path": path })))
}

/// `DELETE /api/terminal/{project}` → `{ stopped: true }`.
pub async fn stop(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.terminals.kill(&project).await;
    tokio::spawn(changes::finish_pending_update(state.clone(), project));
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
    /// The CLI's own conversation id, when its hook payload carries one.
    #[serde(default, rename = "sessionId")]
    session_id: Option<String>,
}

/// Fill in the live session's history record from a turn end: the agent's conversation id
/// (if not assigned at launch) and a title from the first summary.
fn record_turn(state: &AppState, req: &TurnEndRequest) {
    let Some(id) = state.terminals.history_id(&req.project) else {
        return;
    };
    let title = req
        .summary
        .as_deref()
        .and_then(|s| s.lines().map(str::trim).find(|l| !l.is_empty()))
        .map(truncate_summary);
    let now = history::now_ms();
    let updated = history::update(&state_dir(state), &req.project, &id, |r| {
        if r.agent_session_id.is_none() {
            r.agent_session_id = req.session_id.clone();
        }
        if r.title.is_none() {
            r.title = title;
        }
        r.updated_at = now;
    });
    match updated {
        Ok(Some(_)) => state.events.emit(
            "terminal.session",
            json!({ "projectId": req.project, "sessionId": id }),
        ),
        Ok(None) => {}
        Err(e) => eprintln!("workbench: cannot update terminal session history: {e}"),
    }
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
    record_turn(&state, &req);
    // The agent may have started or switched changes this turn.
    state.events.emit("terminal.turn", json!({ "projectId": req.project }));

    let dir = project_dir(&state, &req.project).await?;
    if let Err(e) = checkpoints::require_change(&state, &req.project).await {
        // A turn that only talked (no edits) on `main` is fine.
        if crate::changes::is_dirty(&dir).await? {
            state.events.emit(
                "checkpoint.blocked",
                json!({
                    "projectId": req.project,
                    "reason": e.message,
                }),
            );
        }
        return Ok(Json(json!({ "committed": false })));
    }
    let changed = crate::git::changed_paths(&dir).await?;
    let message = commit_message(req.summary.as_deref(), &changed);
    let committed = match crate::git::checkpoint(&dir, &message).await? {
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
            true
        }
        None => false,
    };
    // The agent is idle and its edits are committed: safe to move the checkout now.
    tokio::spawn(changes::finish_pending_update(state.clone(), req.project.clone()));
    Ok(Json(json!({ "committed": committed })))
}

#[cfg(test)]
mod tests {
    use super::uuid_v4;

    #[test]
    fn uuid_v4_is_well_formed() {
        let id = uuid_v4();
        let groups: Vec<_> = id.split('-').map(str::len).collect();
        assert_eq!(groups, [8, 4, 4, 4, 12], "{id}");
        assert!(id.chars().all(|c| c == '-' || c.is_ascii_hexdigit()), "{id}");
        assert_eq!(&id[14..15], "4", "{id}");
        assert!("89ab".contains(&id[19..20]), "{id}");
        assert_ne!(id, uuid_v4());
    }
}
