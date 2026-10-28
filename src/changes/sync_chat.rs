//! Sync chats: a durable merge worktree, its record and intro, and verification
//! of every finished turn.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::json;
use tokio::process::Command;

use super::conflict_sessions::ConflictSession;
use super::conflicts;
use crate::chat::chunk::UiChunk;
use crate::chat::history::{self, RecordKind, SessionRecord, SyncPhase, SyncState};
use crate::chat::manager::TurnEnded;
use crate::config::random_token;
use crate::error::{ApiError, ErrorCode};
use crate::git;
use crate::http::chat::{acp_argv_for, ensure_agent, state_dir};
use crate::state::AppState;

/// The one message the writer sees when a sync begins.
pub const INTRO: &str = "You and a teammate both changed {pages}. I'll keep both changes and check the site still works.";
/// The site built and the draft now has both sides.
pub const DONE: &str = "Done. Your changes and the latest site are combined.";
/// A decision is still needed; `{n}` is the number of pages.
pub const MARKERS: &str = "{n} pages still need a decision.";
/// The decision-needed line for a single page.
pub const MARKERS_ONE: &str = "{n} page still needs a decision.";
/// The agent introduced something a site cannot serve.
pub const ONE_FILE: &str = "One file still needs another look.";
/// The build failed once; the agent gets one repair turn.
pub const BUILD_RETRY: &str = "The site didn't work after those changes. I'll try once more.";
/// The build failed again; the sync ends with the draft untouched.
pub const BUILD_FAILED_TWICE: &str = "I couldn't get your site working again. Nothing changed.";
/// The draft moved while combining; the sync ends with no land.
pub const DRAFT_MOVED: &str = "Your draft changed while combining. Get latest again.";
/// The writer ended the sync.
pub const ENDED: &str = "You ended this combine. Nothing changed.";
/// The sync outlived its time limit.
pub const EXPIRED: &str = "This combine ran out of time. Nothing changed.";
/// The assistant could not be started for this sync.
pub const UNAVAILABLE: &str = "Your assistant isn't available.";
/// A turn ran too long; the writer can ask it to continue.
pub const TOO_LONG: &str = "That took too long. Tell me to continue when you're ready.";
/// A restart could not restore the sync.
pub const RECOVERY_FAILED: &str = "I lost track of this combine. Nothing changed.";
/// An unexpected internal failure while starting or advancing a sync.
pub const UNEXPECTED: &str = "I couldn't finish combining. Nothing changed.";
/// The agent moved git itself; the chat stays open so the writer can redirect it.
pub const AGENT_MOVED: &str = "Still working on this one.";

/// How long a site build may take before the repair turn is offered as timed out.
const BUILD_LIMIT: Duration = Duration::from_secs(300);
/// How much of a failed build's output the repair prompt carries.
const LOG_TAIL_CHARS: usize = 1800;
/// The default a sync prompt may run before the watchdog stops it: ten minutes.
const TURN_LIMIT: Duration = Duration::from_secs(600);

/// The decision-needed line for `n` pages, singular when there is one.
pub fn pages_needing_decision(n: usize) -> String {
    let template = if n == 1 { MARKERS_ONE } else { MARKERS };
    template.replace("{n}", &n.to_string())
}

/// Replaces an internal error's message with the writer-facing copy, keeping its
/// status. The technical detail goes to the log; it must never reach the writer.
fn internal(what: &str, err: ApiError) -> ApiError {
    eprintln!("orso: cannot start a sync chat ({what}): {err}");
    ApiError::new(err.code, UNEXPECTED)
}

/// Opens a sync chat for the conflict session `session_id`: a worktree at the
/// draft tip mid-merge with the live site, a `Sync` record pointing at it, and
/// the agent's first turn. Returns the chat id.
pub async fn start(
    state: &AppState,
    project_id: &str,
    session_id: &str,
    provider: &str,
) -> Result<String, ApiError> {
    // Check and insert under one lock: two simultaneous requests must not both
    // create a sync chat for the same project.
    let (chat_id, site, record, st) = {
        let _guard = start_lock().lock().await;
        if active_for_project(state, project_id).is_some() {
            return Err(ApiError::conflict("Combining is in progress."));
        }
        let project_dir = crate::http::project_dir(state, project_id)
            .await
            .map_err(|e| internal("project", e))?;
        let session = state
            .conflicts
            .get(session_id)
            .await
            .map_err(|e| internal(&format!("session {session_id}"), e))?;
        if session.project_id != project_id {
            let reason = format!(
                "session {session_id} belongs to project {}, not {project_id}",
                session.project_id
            );
            return Err(internal(
                &reason,
                ApiError::enoent(format!("no such conflict: {session_id}")),
            ));
        }
        let known_provider = acp_argv_for(state, provider)
            .map_err(|e| internal(&format!("provider {provider}"), e))?;
        if known_provider.is_none() {
            return Err(ApiError::badreq(format!("unknown provider: {provider}")));
        }

        let chat_id = random_token()[..16].to_string();
        let chat_dir = state_dir(state).join("sync").join(&chat_id);
        let site = chat_dir.join("site");
        let st = SyncState {
            conflict_session_id: session.id.clone(),
            branch: session.branch.clone(),
            tip: session.tip.clone(),
            incoming: session.incoming.clone(),
            files: session.files.iter().map(|f| f.path.clone()).collect(),
            phase: SyncPhase::Working,
            started_at: history::now_ms(),
            ended_at: None,
            repair_attempted: false,
        };
        let now = history::now_ms();
        let record = SessionRecord {
            id: chat_id.clone(),
            provider: provider.to_string(),
            model: None,
            agent_session_id: None,
            title: Some(title(&st)),
            created_at: now,
            updated_at: now,
            kind: RecordKind::Sync,
            cwd: Some(site.clone()),
            sync: Some(st.clone()),
        };
        if let Err(e) = prepare(
            state,
            project_id,
            &project_dir,
            &chat_dir,
            &site,
            &session,
            &record,
        )
        .await
        {
            remove_worktree(&project_dir, &chat_dir);
            return Err(internal("worktree", e));
        }
        (chat_id, site, record, st)
    };

    let started = match ensure_agent(state, project_id, &chat_id, &site, &record).await {
        Ok(()) => {
            let prompt = opening_prompt(&st);
            let sent = state
                .chats
                .send_message_displayed(project_id, &chat_id, &prompt, &intro(&st))
                .await
                .is_ok();
            if sent {
                watch_turn(state, project_id, &chat_id);
            }
            sent
        }
        Err(e) => {
            eprintln!("orso: sync chat {chat_id} has no assistant: {e}");
            false
        }
    };
    if !started {
        end(state, project_id, &chat_id, SyncPhase::Ended, UNAVAILABLE)
            .await
            .map_err(|e| internal("end", e))?;
        return Ok(chat_id);
    }
    emit(state, &record);
    Ok(chat_id)
}

/// Serializes the check-then-insert window of [`start`].
fn start_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// Creates the worktree at the draft tip, merges the live site into it, and
/// records the chat. Any error leaves the cleanup to [`start`].
async fn prepare(
    state: &AppState,
    project_id: &str,
    project_dir: &Path,
    chat_dir: &Path,
    site: &Path,
    session: &ConflictSession,
    record: &SessionRecord,
) -> Result<(), ApiError> {
    tokio::fs::create_dir_all(chat_dir).await?;
    let site_arg = site.to_string_lossy().into_owned();
    conflicts::checked(
        project_dir,
        &[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            &site_arg,
            &session.tip,
        ],
        &[],
        None,
    )
    .await?;
    let (status, _, stderr) = conflicts::git(
        site,
        &["merge", "--no-commit", "--no-ff", &session.incoming],
        &[],
        None,
    )
    .await?;
    // 0 is a clean (still uncommitted) merge, 1 is a collision; both are fine.
    if !matches!(status, 0 | 1) {
        return Err(ApiError::git(format!(
            "git merge failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        )));
    }
    history::insert(&state_dir(state), project_id, record.clone())?;
    Ok(())
}

/// Runs `change` on a sync record only while it is live, inside the history
/// write lock. `None` means there was nothing to change: the record is gone,
/// is not a sync, or already reached a terminal phase. Checking terminality in
/// here — not before the call — is what stops a finishing verification from
/// resurrecting a sync the writer just ended.
fn update_live(
    state: &AppState,
    project: &str,
    chat: &str,
    change: impl FnOnce(&mut SessionRecord),
) -> Result<Option<SessionRecord>, ApiError> {
    let mut skipped = false;
    let updated = history::update(&state_dir(state), project, chat, |record| {
        if !record
            .sync
            .as_ref()
            .is_some_and(|st| !st.phase.is_terminal())
        {
            skipped = true;
            return;
        }
        change(record);
    })?;
    Ok(match updated {
        Some(record) if !skipped => Some(record),
        _ => None,
    })
}

/// Ends a sync chat: no running turn, one final status line, and no worktree.
/// Ending an already-terminal chat is a no-op.
pub async fn end(
    state: &AppState,
    project: &str,
    chat: &str,
    phase: SyncPhase,
    copy: &str,
) -> Result<(), ApiError> {
    let dir = state_dir(state);
    let Some(record) = history::get(&dir, project, chat) else {
        return Ok(());
    };
    if record.sync.is_none() {
        return Ok(());
    }
    // The record first, so a turn that ends during cleanup sees a terminal phase.
    let now = history::now_ms();
    let Some(updated) = update_live(state, project, chat, |record| {
        if let Some(st) = record.sync.as_mut() {
            st.phase = phase;
            st.ended_at = Some(now);
        }
        record.updated_at = now;
    })?
    else {
        return Ok(());
    };
    emit(state, &updated);
    state.chats.record(
        project,
        chat,
        UiChunk::SyncStatus {
            phase: wire_phase(phase),
            text: copy.to_string(),
        },
    );
    let _ = state.chats.cancel(project, chat).await;
    state.chats.forget(project, chat);
    discard_worktree(state, project, &dir.join("sync").join(chat)).await;
    Ok(())
}

/// One verification at a time, process-wide: two syncs must never race on the
/// project checkout they both inspect.
fn advance_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// A finished `end_turn` on a sync chat: verify its worktree off the event pump.
pub fn on_turn_end(state: &AppState, ended: &TurnEnded) {
    if ended.stop_reason != "end_turn" {
        return;
    }
    let state = state.clone();
    let project = ended.project_id.clone();
    let chat = ended.chat_id.clone();
    tokio::spawn(async move {
        let _guard = advance_lock().lock().await;
        advance(&state, &project, &chat).await;
    });
}

/// Verifies the agent's work after a turn. A failed check posts one plain line
/// and returns to `working`; a moved draft ends the sync.
pub async fn advance(state: &AppState, project: &str, chat: &str) {
    let dir = state_dir(state);
    let Some(record) = history::get(&dir, project, chat) else {
        return;
    };
    let active = record.kind == RecordKind::Sync
        && record
            .sync
            .as_ref()
            .is_some_and(|st| !st.phase.is_terminal());
    if !active {
        return;
    }
    let Some(site) = record.cwd.clone() else {
        note(state, &record, UNEXPECTED);
        return;
    };
    let checking = match set_phase(state, project, chat, SyncPhase::Checking) {
        Ok(Some(record)) => record,
        // The record disappeared or ended between the read above and the write:
        // aborts before touching the worktree `end` may already be removing.
        Ok(None) => return,
        Err(e) => {
            eprintln!("orso: cannot check sync chat {chat}: {e}");
            return;
        }
    };
    let Some(st) = checking.sync.clone() else {
        return;
    };
    let project_dir = match crate::http::project_dir(state, project).await {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("orso: cannot verify sync chat {chat}: {e}");
            note(state, &checking, UNEXPECTED);
            return;
        }
    };
    match verify(&st, &project_dir, &site).await {
        Ok(Verdict::DraftMoved) => {
            if let Err(e) = end(state, project, chat, SyncPhase::Ended, DRAFT_MOVED).await {
                eprintln!("orso: cannot end sync chat {chat}: {e}");
            }
        }
        Ok(Verdict::AgentMovedGit) => note(state, &checking, AGENT_MOVED),
        Ok(Verdict::Markers(n)) => note(state, &checking, &pages_needing_decision(n)),
        Ok(Verdict::Symlink) => note(state, &checking, ONE_FILE),
        Ok(Verdict::Pass) => {
            if let Err(fail) = build(state, &checking).await {
                eprintln!("orso: sync chat {chat} did not build: {fail:?}");
                after_build_failure(state, project, chat).await;
                return;
            }
            // A build can take minutes. An End, delete, or expiry meanwhile must
            // never land the draft, so the record is read again before it moves.
            let Some(fresh) = history::get(&dir, project, chat) else {
                return;
            };
            if !is_active(&fresh) {
                return;
            }
            match commit_and_land(state, &project_dir, &site, &fresh).await {
                Ok(()) => combined(state, &fresh),
                // Only a moved (or busy) draft refuses the land; a hard failure
                // leaves the worktree and the conversation in place.
                Err(e) if e.code == ErrorCode::Conflict => {
                    if let Err(e) = end(state, project, chat, SyncPhase::Ended, DRAFT_MOVED).await {
                        eprintln!("orso: cannot end sync chat {chat}: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("orso: cannot land sync chat {chat}: {e}");
                    note(state, &fresh, UNEXPECTED);
                }
            }
        }
        Err(e) => {
            eprintln!("orso: cannot verify sync chat {chat}: {e}");
            note(state, &checking, UNEXPECTED);
        }
    }
}

/// A failed build: one repair turn, then the sync ends with the draft untouched.
async fn after_build_failure(state: &AppState, project: &str, chat: &str) {
    let Some(record) = history::get(&state_dir(state), project, chat) else {
        return;
    };
    if !is_active(&record) {
        return;
    }
    let Some(st) = record.sync.as_ref() else {
        return;
    };
    if st.repair_attempted {
        if let Err(e) = end(state, project, chat, SyncPhase::Ended, BUILD_FAILED_TWICE).await {
            eprintln!("orso: cannot end sync chat {chat}: {e}");
        }
        return;
    }
    let now = history::now_ms();
    let marked = match update_live(state, project, chat, |record| {
        if let Some(st) = record.sync.as_mut() {
            st.repair_attempted = true;
        }
        record.updated_at = now;
    }) {
        Ok(Some(record)) => record,
        Ok(None) => return,
        Err(e) => {
            eprintln!("orso: cannot mark sync chat {chat} for a repair: {e}");
            return;
        }
    };
    note(state, &marked, BUILD_RETRY);
    repair(state, project, chat).await;
}

/// Asks the agent once to fix the site, carrying the tail of the build output.
/// Hidden: the build log is an instruction, never the writer's message.
async fn repair(state: &AppState, project: &str, chat: &str) {
    let Some(record) = history::get(&state_dir(state), project, chat) else {
        return;
    };
    // A sync that ended while the build ran gets no prompt; only `working` sends.
    if !record
        .sync
        .as_ref()
        .is_some_and(|st| st.phase == SyncPhase::Working)
    {
        return;
    }
    let prompt = repair_prompt(&build_log_tail(state, chat).await);
    match state
        .chats
        .send_message_hidden(project, chat, &prompt)
        .await
    {
        Ok(()) => watch_turn(state, project, chat),
        Err(e) => eprintln!("orso: cannot ask sync chat {chat} to repair the site: {e}"),
    }
}

/// The one repair instruction: what broke, then the pages it may touch.
fn repair_prompt(tail: &str) -> String {
    format!(
        "The site didn't work after those changes. The end of the build output was:\n\n{tail}\n\n\
         Find what broke it and fix it, then reply when the site works again. \
         Leave every page you didn't need to change alone, and don't save, undo, or switch drafts yourself."
    )
}

/// Builds the sync's worktree as the writer's site: dependencies linked in,
/// `npm run build` with a 300 s cap, and the output appended to the chat's log.
pub async fn build(state: &AppState, record: &SessionRecord) -> Result<(), BuildFail> {
    let Some(site) = record.cwd.clone() else {
        return Err(BuildFail::Failed);
    };
    let Some(project) = project_of(&state_dir(state), &record.id) else {
        return Err(BuildFail::Failed);
    };
    let project_dir = match crate::http::project_dir(state, &project).await {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("orso: cannot build sync chat {}: {e}", record.id);
            return Err(BuildFail::Failed);
        }
    };
    // A repair build finds the link already there.
    match crate::conflict_preview::link_node_modules(&project_dir, &site) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => {
            eprintln!(
                "orso: cannot link dependencies for sync chat {}: {e}",
                record.id
            );
            return Err(BuildFail::Failed);
        }
    }
    let log = build_log(state, &record.id);
    if let Some(dir) = log.parent() {
        if let Err(e) = tokio::fs::create_dir_all(dir).await {
            eprintln!("orso: cannot create {}: {e}", dir.display());
            return Err(BuildFail::Failed);
        }
    }
    let mut file = match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .await
    {
        Ok(file) => file,
        Err(e) => {
            eprintln!("orso: cannot open {}: {e}", log.display());
            return Err(BuildFail::Failed);
        }
    };

    let mut command = Command::new("npm");
    command
        .args(["run", "build"])
        .current_dir(&site)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .process_group(0);
    let child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("orso: cannot build sync chat {}: {e}", record.id);
            return Err(BuildFail::Failed);
        }
    };
    // Dropping the group kills npm and everything it started, including after a timeout.
    let group = crate::conflict_preview::BuildGroup(child.id().map(|pid| pid as i32));
    let waited = tokio::time::timeout(BUILD_LIMIT, child.wait_with_output()).await;
    drop(group);
    let (failed, output) = match waited {
        Ok(Ok(out)) => {
            let mut output = out.stdout;
            output.extend_from_slice(&out.stderr);
            ((!out.status.success()).then_some(BuildFail::Failed), output)
        }
        Ok(Err(e)) => {
            eprintln!("orso: cannot build sync chat {}: {e}", record.id);
            (Some(BuildFail::Failed), Vec::new())
        }
        Err(_) => (
            Some(BuildFail::TimedOut),
            format!(
                "The build did not finish within {} seconds.\n",
                BUILD_LIMIT.as_secs()
            )
            .into_bytes(),
        ),
    };
    use tokio::io::AsyncWriteExt;
    let _ = file
        .write_all(format!("\n--- build {} ---\n", history::now_ms()).as_bytes())
        .await;
    let _ = file.write_all(&output).await;
    let _ = file.flush().await;
    match failed {
        Some(fail) => Err(fail),
        None => Ok(()),
    }
}

/// The worktree's staged tree, committed with both sides, then landed on the draft.
async fn commit_and_land(
    state: &AppState,
    project_dir: &Path,
    site: &Path,
    record: &SessionRecord,
) -> Result<(), ApiError> {
    let Some(st) = record.sync.as_ref() else {
        return Err(ApiError::git("the sync record has no state"));
    };
    let tree = conflicts::text(site, &["write-tree"]).await?;
    let author = state.github.identity().await;
    let commit = conflicts::merge_commit(
        project_dir,
        &tree,
        &st.tip,
        &st.incoming,
        &author,
        "Sync with live site",
    )
    .await?;
    land(state, record, &commit).await
}

/// Moves the draft branch to `commit`, only while its tip is still the sync's.
/// This is the one place a sync moves the draft, and it runs under the project's
/// git lock; `advance` already holds the process-wide sync lock, never the reverse.
pub async fn land(state: &AppState, record: &SessionRecord, commit: &str) -> Result<(), ApiError> {
    let Some(st) = record.sync.as_ref() else {
        return Err(ApiError::git("the sync record has no state"));
    };
    let project = project_of(&state_dir(state), &record.id)
        .ok_or_else(|| ApiError::enoent(format!("no such chat: {}", record.id)))?;
    let project_dir = crate::http::project_dir(state, &project).await?;
    let lock = git::lock_for(&project_dir);
    let _guard = lock.lock().await;
    conflicts::advance_branch(&project_dir, &st.branch, &st.tip, commit).await
}

/// Finishes a landed sync: its conflict session and previews are done, then the
/// chat ends `combined` and loses its worktree. Best effort: a leftover session
/// costs the writer a manual path, never the land.
pub fn combined(state: &AppState, record: &SessionRecord) {
    let state = state.clone();
    let record = record.clone();
    tokio::spawn(async move {
        let Some(project) = project_of(&state_dir(&state), &record.id) else {
            return;
        };
        if let Some(st) = record.sync.as_ref() {
            if let Err(e) = state.conflicts.abandon(&st.conflict_session_id).await {
                eprintln!(
                    "orso: cannot finish conflict session {}: {e}",
                    st.conflict_session_id
                );
            }
            state
                .conflict_previews
                .remove_session(&st.conflict_session_id)
                .await;
        }
        if let Err(e) = end(&state, &project, &record.id, SyncPhase::Combined, DONE).await {
            eprintln!("orso: cannot finish sync chat {}: {e}", record.id);
        }
    });
}

/// How long a sync prompt may run. `ORSO_SYNC_TURN_LIMIT_SECS` is a test seam.
fn turn_limit() -> Duration {
    std::env::var("ORSO_SYNC_TURN_LIMIT_SECS")
        .ok()
        .and_then(|secs| secs.trim().parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .map(Duration::from_secs)
        .unwrap_or(TURN_LIMIT)
}

/// The watchdog every sync prompt gets: after the limit, a turn that still runs
/// on a working sync is cancelled and the writer is told how to continue.
/// Ordinary chats have no watchdog.
pub fn watch_turn(state: &AppState, project: &str, chat: &str) {
    let state = state.clone();
    let project = project.to_string();
    let chat = chat.to_string();
    tokio::spawn(async move {
        tokio::time::sleep(turn_limit()).await;
        let Some(record) = history::get(&state_dir(&state), &project, &chat) else {
            return;
        };
        if !state.chats.is_running(&project, &chat)
            || !record
                .sync
                .as_ref()
                .is_some_and(|st| st.phase == SyncPhase::Working)
        {
            return;
        }
        if let Err(e) = state.chats.cancel(&project, &chat).await {
            eprintln!("orso: cannot stop an overlong sync turn: {e}");
        }
        note(&state, &record, TOO_LONG);
    });
}

/// Where a sync chat's build output goes, one file per chat, across attempts.
fn build_log(state: &AppState, chat: &str) -> PathBuf {
    state_dir(state)
        .join("sync-logs")
        .join(format!("{chat}.log"))
}

/// The last [`LOG_TAIL_CHARS`] characters of a chat's build log.
async fn build_log_tail(state: &AppState, chat: &str) -> String {
    let text = tokio::fs::read_to_string(build_log(state, chat))
        .await
        .unwrap_or_default();
    let mut tail: Vec<char> = text.trim_end().chars().rev().take(LOG_TAIL_CHARS).collect();
    tail.reverse();
    tail.into_iter().collect()
}

/// One plain line from the assistant: records the status chunk, reopens the
/// chat, and tells the UI. A terminal sync is left alone.
pub fn note(state: &AppState, record: &SessionRecord, text: &str) {
    let dir = state_dir(state);
    let Some(project) = project_of(&dir, &record.id) else {
        return;
    };
    let now = history::now_ms();
    let updated = update_live(state, &project, &record.id, |record| {
        if let Some(st) = record.sync.as_mut() {
            st.phase = SyncPhase::Working;
        }
        record.updated_at = now;
    });
    let Ok(Some(updated)) = updated else {
        return;
    };
    state.chats.record(
        &project,
        &record.id,
        UiChunk::SyncStatus {
            phase: wire_phase(SyncPhase::Working),
            text: text.to_string(),
        },
    );
    emit(state, &updated);
}

/// Moves a live sync to `phase` and tells the UI. `Ok(None)` when the record is
/// gone or already terminal: the write was skipped under the lock.
fn set_phase(
    state: &AppState,
    project: &str,
    chat: &str,
    phase: SyncPhase,
) -> Result<Option<SessionRecord>, ApiError> {
    let now = history::now_ms();
    let updated = update_live(state, project, chat, |record| {
        if let Some(st) = record.sync.as_mut() {
            st.phase = phase;
        }
        record.updated_at = now;
    })?;
    if let Some(record) = &updated {
        emit(state, record);
    }
    Ok(updated)
}

/// Removes a sync chat's worktree and its directory. Failures are logged: a
/// leftover is recoverable, a failed end is not.
pub fn remove_worktree(project_dir: &Path, chat_dir: &Path) {
    let site = chat_dir.join("site");
    if site.exists() {
        let removed = std::process::Command::new("git")
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "worktree",
                "remove",
                "--force",
            ])
            .arg(&site)
            .current_dir(project_dir)
            .output();
        match removed {
            Ok(out) if !out.status.success() => eprintln!(
                "orso: cannot remove the sync worktree {}: {}",
                site.display(),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            Err(e) => eprintln!(
                "orso: cannot remove the sync worktree {}: {e}",
                site.display()
            ),
            Ok(_) => {}
        }
    }
    if let Err(e) = std::fs::remove_dir_all(chat_dir) {
        if e.kind() != std::io::ErrorKind::NotFound {
            eprintln!("orso: cannot remove {}: {e}", chat_dir.display());
        }
    }
}

/// [`remove_worktree`] for a chat, using the project checkout to unregister it.
async fn discard_worktree(state: &AppState, project: &str, chat_dir: &Path) {
    match state.projects.get(project).await {
        Some(p) => {
            let project_dir = p.dir;
            let chat_dir = chat_dir.to_path_buf();
            let _ =
                tokio::task::spawn_blocking(move || remove_worktree(&project_dir, &chat_dir)).await;
        }
        None => {
            if let Err(e) = tokio::fs::remove_dir_all(chat_dir).await {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("orso: cannot remove {}: {e}", chat_dir.display());
                }
            }
        }
    }
}

/// The sync chat still running for `project_id`, if any.
pub fn active_for_project(state: &AppState, project_id: &str) -> Option<SessionRecord> {
    history::list(&state_dir(state), project_id)
        .into_iter()
        .find(is_active)
}

/// The sync chat still running for the conflict session `session_id`, if any.
pub fn active_for_session(state: &AppState, session_id: &str) -> Option<SessionRecord> {
    let dir = state_dir(state);
    project_ids(&dir).into_iter().find_map(|project| {
        history::list(&dir, &project).into_iter().find(|record| {
            is_active(record)
                && record
                    .sync
                    .as_ref()
                    .is_some_and(|st| st.conflict_session_id == session_id)
        })
    })
}

/// A `Sync` record that has not reached a terminal phase.
fn is_active(record: &SessionRecord) -> bool {
    record.kind == RecordKind::Sync
        && record
            .sync
            .as_ref()
            .is_some_and(|st| !st.phase.is_terminal())
}

/// Every project id with a state dir. Records carry no project id, so the
/// synchronous lookups scan the projects the daemon keeps state for.
fn project_ids(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

/// The project holding record `id`. Records carry no project id, so the
/// synchronous lookups scan the projects the daemon keeps state for.
fn project_of(dir: &Path, id: &str) -> Option<String> {
    project_ids(dir)
        .into_iter()
        .find(|project| history::get(dir, project, id).is_some())
}

/// `sync.changed` for the UI: creation, every phase change, and ending.
pub fn emit(state: &AppState, record: &SessionRecord) {
    let Some(st) = record.sync.as_ref() else {
        return;
    };
    let dir = state_dir(state);
    let Some(project) = project_of(&dir, &record.id) else {
        return;
    };
    state.events.emit(
        "sync.changed",
        json!({
            "projectId": project,
            "conflictSessionId": &st.conflict_session_id,
            "chatId": &record.id,
            "phase": st.phase,
        }),
    );
}

/// The writer-facing opening message: the pages, and what happens next.
pub fn intro(st: &SyncState) -> String {
    INTRO.replace("{pages}", &pages(&st.files))
}

/// The hidden prompt: one paragraph of instructions, then the intro shown.
pub fn opening_prompt(st: &SyncState) -> String {
    format!(
        "You're combining two versions of a website. The pages to combine are {pages}. \
         Work only on those pages and leave every other page exactly as it is. \
         For each page, read both versions and keep what each side meant. \
         Talk about pages and paragraphs, and quote both versions word for word when you ask the writer a question. \
         Offer choices the way a person would, and if the writer says they don't understand, explain it another way. \
         Never save a version, undo the combine, or switch drafts yourself. \
         Before you finish the last page, ask any last question you have, not after.\n\n{intro}",
        pages = pages(&st.files),
        intro = intro(st),
    )
}

/// The collided pages as a writer reads them: "About", "About and Pricing",
/// "About, Pricing and Home".
fn pages(files: &[String]) -> String {
    let labels: Vec<String> = files.iter().map(|f| label(f)).collect();
    match labels.as_slice() {
        [] => "some pages".to_string(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The snake_case phase name a [`UiChunk::SyncStatus`] carries.
fn wire_phase(phase: SyncPhase) -> String {
    // A unit enum always serializes to a string; an empty fallback would hide
    // the transition from the UI instead.
    serde_json::to_value(phase)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .expect("a SyncPhase serializes to a string")
}

/// Why a sync's build did not pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFail {
    /// The site build exited non-zero.
    Failed,
    /// The site build did not finish inside the 300 s cap.
    TimedOut,
}

/// What the agent's work in the sync worktree looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Clean: staged, marker-free, and no symlinks the live site didn't already have.
    Pass,
    /// Conflicts markers remain in this many of the collided files.
    Markers(usize),
    /// The agent added a symlink the live site didn't have.
    Symlink,
    /// The agent committed, or replaced the merge, instead of only editing files.
    AgentMovedGit,
    /// The draft branch moved (or vanished) while combining.
    DraftMoved,
}

/// Checks the agent's work in `worktree` against the sync recorded in `state`.
pub async fn verify(
    state: &SyncState,
    project_dir: &Path,
    worktree: &Path,
) -> Result<Verdict, ApiError> {
    if draft_moved(state, project_dir).await? {
        return Ok(Verdict::DraftMoved);
    }
    let head = conflicts::text(worktree, &["rev-parse", "HEAD"]).await?;
    if head != state.tip {
        return Ok(Verdict::AgentMovedGit);
    }
    let (status, merge_head, _) = conflicts::git(
        worktree,
        &["rev-parse", "-q", "--verify", "MERGE_HEAD"],
        &[],
        None,
    )
    .await?;
    if status != 0 || String::from_utf8_lossy(&merge_head).trim() != state.incoming {
        return Ok(Verdict::AgentMovedGit);
    }
    conflicts::checked(
        worktree,
        &["add", "-A", "--", ".", ":!node_modules"],
        &[],
        None,
    )
    .await?;
    if let Some(markers) = marker_files(worktree, &state.files).await? {
        return Ok(Verdict::Markers(markers));
    }
    if new_symlink(worktree, &state.tip, &state.incoming).await? {
        return Ok(Verdict::Symlink);
    }
    Ok(Verdict::Pass)
}

/// The draft branch must still be the one the sync started from.
async fn draft_moved(state: &SyncState, project_dir: &Path) -> Result<bool, ApiError> {
    let (status, branch, _) = conflicts::git(
        project_dir,
        &["rev-parse", &format!("refs/heads/{}", state.branch)],
        &[],
        None,
    )
    .await?;
    Ok(status != 0 || String::from_utf8_lossy(&branch).trim() != state.tip)
}

/// How many of `files` still have conflict markers, or `None` when none do.
async fn marker_files(worktree: &Path, files: &[String]) -> Result<Option<usize>, ApiError> {
    if files.is_empty() {
        return Ok(None);
    }
    let mut args = vec![
        "--literal-pathspecs",
        "grep",
        "--cached",
        "-l",
        "-E",
        "^(<<<<<<< |=======$|>>>>>>> )",
        "--",
    ];
    args.extend(files.iter().map(String::as_str));
    let (status, stdout, stderr) = conflicts::git(worktree, &args, &[], None).await?;
    match status {
        1 => Ok(None),
        0 => Ok(Some(
            String::from_utf8_lossy(&stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
        )),
        _ => Err(ApiError::git(format!(
            "git grep failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        ))),
    }
}

/// Whether the agent added a symlink the incoming side did not already have.
async fn new_symlink(worktree: &Path, tip: &str, incoming: &str) -> Result<bool, ApiError> {
    let raw = conflicts::checked(
        worktree,
        &["diff", "--cached", "--raw", "-z", "--no-renames", tip],
        &[],
        None,
    )
    .await?;
    let raw = String::from_utf8_lossy(&raw);
    let mut fields = raw.split('\0');
    while let (Some(header), Some(path)) = (fields.next(), fields.next()) {
        if header.split_whitespace().nth(1) != Some("120000") {
            continue;
        }
        let entry = conflicts::text(
            worktree,
            &["--literal-pathspecs", "ls-tree", incoming, "--", path],
        )
        .await?;
        if !entry.starts_with("120000 ") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// "src/content/pricing-plans.md" → "Pricing plans".
pub fn label(path: &str) -> String {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .replace(['-', '_'], " ");
    let mut chars = stem.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => path.to_string(),
    }
}

/// The chat title: the collided pages, at most three, kept short for the sidebar.
pub fn title(state: &SyncState) -> String {
    let labels: Vec<String> = state.files.iter().take(3).map(|f| label(f)).collect();
    let title = format!("Combining {}", labels.join(", "));
    title.chars().take(72).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::history::SyncPhase;
    use std::process::Command;

    fn sh(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args([
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@e",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Repo mid-merge with a conflict in `page.md`; `live` adds extra files on the incoming side.
    /// Returns (dir, tip, incoming).
    fn conflicted_with(live: impl Fn(&Path)) -> (tempfile::TempDir, String, String) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        sh(dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("page.md"), "title: base\n").unwrap();
        std::fs::write(dir.join("notes.md"), "Notes\n=======\n").unwrap();
        sh(dir, &["add", "."]);
        sh(dir, &["commit", "-qm", "base"]);
        sh(dir, &["checkout", "-qb", "draft"]);
        std::fs::write(dir.join("page.md"), "title: draft\n").unwrap();
        sh(dir, &["commit", "-qam", "draft"]);
        let tip = sh(dir, &["rev-parse", "HEAD"]);
        sh(dir, &["checkout", "-q", "main"]);
        std::fs::write(dir.join("page.md"), "title: live\n").unwrap();
        live(dir);
        sh(dir, &["add", "-A"]);
        sh(dir, &["commit", "-qm", "live"]);
        let incoming = sh(dir, &["rev-parse", "HEAD"]);
        sh(dir, &["checkout", "-q", "draft"]);
        sh(dir, &["merge", "--no-commit", "--no-ff", &incoming]);
        let page = std::fs::read_to_string(dir.join("page.md")).unwrap();
        assert!(page.contains("<<<<<<< "), "fixture must conflict");
        (tmp, tip, incoming)
    }

    fn conflicted() -> (tempfile::TempDir, String, String) {
        conflicted_with(|_| {})
    }

    fn resolve(dir: &Path) {
        std::fs::write(dir.join("page.md"), "title: draft and live\n").unwrap();
    }

    fn record(branch: &str, tip: &str, incoming: &str, files: &[&str]) -> SyncState {
        SyncState {
            conflict_session_id: "session".into(),
            branch: branch.into(),
            tip: tip.into(),
            incoming: incoming.into(),
            files: files.iter().map(|f| f.to_string()).collect(),
            phase: SyncPhase::Checking,
            started_at: 0,
            ended_at: None,
            repair_attempted: false,
        }
    }

    fn session_record(id: &str, sync: SyncState) -> SessionRecord {
        SessionRecord {
            id: id.into(),
            provider: "fake".into(),
            model: None,
            agent_session_id: None,
            title: None,
            created_at: 0,
            updated_at: 0,
            kind: RecordKind::Sync,
            cwd: None,
            sync: Some(sync),
        }
    }

    /// Once the writer ended a sync, a verification that read it a moment
    /// earlier must not move it back to checking or working, and must not leave
    /// a status chunk behind. The guard runs under the history write lock, at
    /// the exact point where an `end` can otherwise interleave.
    #[tokio::test]
    async fn terminal_sync_ignores_later_phase_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let state = AppState::with_scaffold_install(
            crate::config::Config {
                port: 0,
                projects_dir: tmp.path().join("projects"),
                no_auth: true,
                ..Default::default()
            },
            false,
        );
        let dir = state_dir(&state);
        let mut st = record("content/page", "tip", "incoming", &["page.md"]);
        st.phase = SyncPhase::Ended;
        st.ended_at = Some(1);
        history::insert(&dir, "p", session_record("c1", st)).unwrap();
        let mut events = state.events.subscribe();

        assert!(set_phase(&state, "p", "c1", SyncPhase::Checking)
            .unwrap()
            .is_none());
        note(&state, &history::get(&dir, "p", "c1").unwrap(), AGENT_MOVED);

        let got = history::get(&dir, "p", "c1").unwrap();
        let got = got.sync.unwrap();
        assert_eq!(got.phase, SyncPhase::Ended);
        assert_eq!(got.ended_at, Some(1));
        assert_eq!(
            history::get(&dir, "p", "c1").unwrap().updated_at,
            0,
            "the skipped write must not touch the record"
        );
        assert!(
            state.chats.history("p", "c1").is_empty(),
            "the skipped note must not append a status chunk"
        );
        assert!(
            events.try_recv().is_err(),
            "no sync.changed for a skipped write"
        );
    }

    /// A sync state whose `branch` is the fixture's current HEAD — the draft it started from.
    fn state(dir: &Path, branch: &str, tip: &str, incoming: &str, files: &[&str]) -> SyncState {
        sh(dir, &["branch", "-f", branch, "HEAD"]);
        record(branch, tip, incoming, files)
    }

    #[tokio::test]
    async fn verify_rejects_markers() {
        let (tmp, tip, incoming) = conflicted();
        let st = state(tmp.path(), "content/page", &tip, &incoming, &["page.md"]);
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::Markers(1)
        );
    }

    #[tokio::test]
    async fn verify_ignores_markers_outside_conflicted_files() {
        let (tmp, tip, incoming) = conflicted();
        let st = state(tmp.path(), "content/page", &tip, &incoming, &["page.md"]);
        resolve(tmp.path());
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::Pass
        );
    }

    #[tokio::test]
    async fn verify_rejects_agent_commit() {
        let (tmp, tip, incoming) = conflicted();
        let st = state(tmp.path(), "content/page", &tip, &incoming, &["page.md"]);
        resolve(tmp.path());
        sh(tmp.path(), &["commit", "-qam", "agent"]);
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::AgentMovedGit
        );
    }

    #[tokio::test]
    async fn verify_rejects_new_symlink() {
        let (tmp, tip, incoming) = conflicted();
        let st = state(tmp.path(), "content/page", &tip, &incoming, &["page.md"]);
        resolve(tmp.path());
        std::os::unix::fs::symlink("/etc/passwd", tmp.path().join("link")).unwrap();
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::Symlink
        );
    }

    #[tokio::test]
    async fn verify_allows_symlink_from_incoming() {
        let (tmp, tip, incoming) = conflicted_with(|dir| {
            std::os::unix::fs::symlink("page.md", dir.join("link")).unwrap();
        });
        let st = state(tmp.path(), "content/page", &tip, &incoming, &["page.md"]);
        resolve(tmp.path());
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::Pass
        );
    }

    #[tokio::test]
    async fn moved_draft_is_reported() {
        let (tmp, tip, incoming) = conflicted();
        sh(tmp.path(), &["reset", "-q", "--hard", "HEAD~0"]);
        // move the branch tip the sync was created from
        let st = state(
            tmp.path(),
            "content/page",
            &format!("{tip}0"),
            &incoming,
            &["page.md"],
        );
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::DraftMoved
        );
    }

    #[tokio::test]
    async fn marker_verdict_counts_files() {
        let (tmp, tip, incoming) = conflicted();
        let st = state(
            tmp.path(),
            "content/page",
            &tip,
            &incoming,
            &["page.md", "other.md"],
        );
        assert_eq!(
            verify(&st, tmp.path(), tmp.path()).await.unwrap(),
            Verdict::Markers(1)
        );
    }

    #[test]
    fn labels_from_file_stems() {
        assert_eq!(label("src/content/pricing-plans.md"), "Pricing plans");
        assert_eq!(label("about_us.yaml"), "About us");
        assert_eq!(label(".md"), ".md");
    }

    #[test]
    fn title_names_up_to_three_files_within_72_chars() {
        let st = record(
            "content/page",
            "tip",
            "incoming",
            &[
                "src/content/pricing-plans.md",
                "about_us.md",
                "home.md",
                "extra.md",
            ],
        );
        assert_eq!(title(&st), "Combining Pricing plans, About us, Home");

        let long = vec![format!("{}.md", "x".repeat(60)); 2];
        let files: Vec<&str> = long.iter().map(String::as_str).collect();
        let st = record("content/page", "tip", "incoming", &files);
        let title = title(&st);
        assert_eq!(title.chars().count(), 72, "{title}");
        assert!(title.starts_with("Combining "), "{title}");
    }

    #[test]
    fn intro_names_the_pages_and_promises_to_keep_both() {
        let st = record("content/page", "tip", "incoming", &["src/content/about.md"]);
        assert_eq!(
            intro(&st),
            "You and a teammate both changed About. I'll keep both changes and check the site still works."
        );

        let st = record(
            "content/page",
            "tip",
            "incoming",
            &["about.md", "pricing.md", "home.md"],
        );
        assert_eq!(
            intro(&st),
            "You and a teammate both changed About, Pricing and Home. I'll keep both changes and check the site still works."
        );
    }

    /// The vocabulary ban from the spec: no writer-visible sync text names git.
    #[test]
    fn visible_copy_has_no_git_vocabulary() {
        let st = record(
            "content/page",
            "tip",
            "incoming",
            &["about.md", "pricing.md"],
        );
        let intro = intro(&st);
        let prompt = opening_prompt(&st);
        assert!(prompt.contains(intro.as_str()), "{prompt}");
        assert_eq!(UNEXPECTED, "I couldn't finish combining. Nothing changed.");
        let one = pages_needing_decision(1);
        let many = pages_needing_decision(2);
        assert_eq!(one, "1 page still needs a decision.");
        assert_eq!(many, "2 pages still need a decision.");
        for text in [
            INTRO,
            DONE,
            MARKERS,
            MARKERS_ONE,
            ONE_FILE,
            BUILD_RETRY,
            BUILD_FAILED_TWICE,
            DRAFT_MOVED,
            ENDED,
            EXPIRED,
            UNAVAILABLE,
            TOO_LONG,
            RECOVERY_FAILED,
            UNEXPECTED,
            AGENT_MOVED,
            intro.as_str(),
            prompt.as_str(),
            one.as_str(),
            many.as_str(),
        ] {
            let lower = text.to_ascii_lowercase();
            for word in [
                "branch", "commit", "merge", "conflict", "worktree", "checkout", "clone", "git",
                "origin", "main", "markers", "npm", "<<<<<<<",
            ] {
                assert!(!lower.contains(word), "{word:?} in {text:?}");
            }
        }
    }

    #[test]
    fn every_wire_phase_is_a_nonempty_name() {
        for (phase, wire) in [
            (SyncPhase::Working, "working"),
            (SyncPhase::Checking, "checking"),
            (SyncPhase::Combined, "combined"),
            (SyncPhase::Ended, "ended"),
            (SyncPhase::Expired, "expired"),
        ] {
            assert_eq!(wire_phase(phase), wire);
        }
    }
}
