//! Sync chats: a durable merge worktree, its record and intro, and verification
//! of every finished turn.

use std::path::Path;

use serde_json::json;

use super::conflict_sessions::ConflictSession;
use super::conflicts;
use crate::chat::chunk::UiChunk;
use crate::chat::history::{self, RecordKind, SessionRecord, SyncPhase, SyncState};
use crate::config::random_token;
use crate::error::ApiError;
use crate::http::chat::{acp_argv_for, ensure_agent, state_dir};
use crate::state::AppState;

/// The one message the writer sees when a sync begins.
pub const INTRO: &str = "You and a teammate both changed {pages}. I'll keep both changes and check the site still works.";
/// The assistant could not be started for this sync.
pub const UNAVAILABLE: &str = "Your assistant isn't available.";
/// An unexpected internal failure while starting or advancing a sync.
pub const UNEXPECTED: &str = "I couldn't finish combining. Nothing changed.";

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
            state
                .chats
                .send_message_displayed(project_id, &chat_id, &prompt, &intro(&st))
                .await
                .is_ok()
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
    let Some(st) = record.sync.as_ref() else {
        return Ok(());
    };
    if st.phase.is_terminal() {
        return Ok(());
    }
    // The record first, so a turn that ends during cleanup sees a terminal phase.
    let now = history::now_ms();
    let Some(updated) = history::update(&dir, project, chat, |record| {
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

/// `sync.changed` for the UI: creation, every phase change, and ending.
pub fn emit(state: &AppState, record: &SessionRecord) {
    let Some(st) = record.sync.as_ref() else {
        return;
    };
    let dir = state_dir(state);
    let Some(project) = project_ids(&dir)
        .into_iter()
        .find(|project| history::get(&dir, project, &record.id).is_some())
    else {
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
    serde_json::to_value(phase)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
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
        for text in [
            INTRO,
            UNAVAILABLE,
            UNEXPECTED,
            intro.as_str(),
            prompt.as_str(),
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
}
