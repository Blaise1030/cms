//! Per-project chat session records, `{state_dir}/{project}/sessions.json`.
//!
//! One record per chat. ACP chats do not store an [`crate::providers::AgentSession`]:
//! the live process calls `session/new` itself. `agent_session_id` is kept so records
//! written by the old terminal stay readable. `model` is the optional model id chosen
//! when the chat was created or last set.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

const FILE: &str = "sessions.json";
/// Oldest records beyond this are dropped.
const MAX_RECORDS: usize = 50;

/// Serializes read-modify-write of every history file.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// What a record describes: an ordinary chat, or the chat that resolves a sync
/// conflict. Old records predate this field and read as [`RecordKind::Chat`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    #[default]
    Chat,
    Sync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    Working,
    Checking,
    Combined,
    Ended,
    Expired,
}

impl SyncPhase {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Combined | Self::Ended | Self::Expired)
    }
}

/// Where a sync chat is: the conflict it fixes, the two sides, and its phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub conflict_session_id: String,
    pub branch: String,
    pub tip: String,
    pub incoming: String,
    pub files: Vec<String>,
    pub phase: SyncPhase,
    pub started_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<u64>,
    #[serde(default)]
    pub repair_attempted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: String,
    pub provider: String,
    /// Model id from create or `POST …/model`. Absent on older terminal records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Unix milliseconds.
    pub created_at: u64,
    /// Unix milliseconds of the last start, resume or turn end.
    pub updated_at: u64,
    /// Absent on records written before sync chats; such records are chats.
    #[serde(default)]
    pub kind: RecordKind,
    /// The sync worktree the chat runs in; absent for ordinary chats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<SyncState>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn path(state_dir: &Path, project_id: &str) -> PathBuf {
    state_dir.join(project_id).join(FILE)
}

fn read(file: &Path) -> Vec<SessionRecord> {
    std::fs::read(file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// By start time, so resuming or finishing a turn doesn't reorder the list.
fn newest_first(records: &mut [SessionRecord]) {
    records.sort_by_key(|r| std::cmp::Reverse((r.created_at, r.id.clone())));
}

fn write(file: &Path, mut records: Vec<SessionRecord>) -> std::io::Result<()> {
    newest_first(&mut records);
    records.truncate(MAX_RECORDS);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&records)?)?;
    std::fs::rename(tmp, file)
}

/// Most recently started first. A missing or unreadable file is an empty history.
pub fn list(state_dir: &Path, project_id: &str) -> Vec<SessionRecord> {
    let mut records = read(&path(state_dir, project_id));
    newest_first(&mut records);
    records
}

pub fn get(state_dir: &Path, project_id: &str, id: &str) -> Option<SessionRecord> {
    read(&path(state_dir, project_id))
        .into_iter()
        .find(|r| r.id == id)
}

pub fn insert(state_dir: &Path, project_id: &str, record: SessionRecord) -> std::io::Result<()> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let file = path(state_dir, project_id);
    let mut records = read(&file);
    records.retain(|r| r.id != record.id);
    records.push(record);
    write(&file, records)
}

/// Apply `change` to the record `id`; `Ok(None)` if there is no such record.
pub fn update(
    state_dir: &Path,
    project_id: &str,
    id: &str,
    change: impl FnOnce(&mut SessionRecord),
) -> std::io::Result<Option<SessionRecord>> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let file = path(state_dir, project_id);
    let mut records = read(&file);
    let Some(record) = records.iter_mut().find(|r| r.id == id) else {
        return Ok(None);
    };
    change(record);
    let updated = record.clone();
    write(&file, records)?;
    Ok(Some(updated))
}

/// Drop the record `id`; `Ok(false)` if there was none.
pub fn remove(state_dir: &Path, project_id: &str, id: &str) -> std::io::Result<bool> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let file = path(state_dir, project_id);
    let mut records = read(&file);
    let before = records.len();
    records.retain(|r| r.id != id);
    if records.len() == before {
        return Ok(false);
    }
    write(&file, records)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: &str, updated_at: u64) -> SessionRecord {
        SessionRecord {
            id: id.into(),
            provider: "claude".into(),
            model: None,
            agent_session_id: None,
            title: None,
            created_at: updated_at,
            updated_at,
            kind: RecordKind::Chat,
            cwd: None,
            sync: None,
        }
    }

    #[test]
    fn old_records_default_to_chat() {
        let json = r#"{"id":"a","provider":"claude","createdAt":1,"updatedAt":2}"#;
        let r: SessionRecord = serde_json::from_str(json).unwrap();
        assert_eq!(r.kind, RecordKind::Chat);
        assert!(r.cwd.is_none() && r.sync.is_none());
    }

    #[test]
    fn sync_fields_round_trip() {
        let r = SessionRecord {
            id: "s1".into(),
            provider: "fake".into(),
            model: None,
            agent_session_id: None,
            title: Some("Combining About".into()),
            created_at: 1,
            updated_at: 2,
            kind: RecordKind::Sync,
            cwd: Some("/tmp/wb".into()),
            sync: Some(SyncState {
                conflict_session_id: "c1".into(),
                branch: "content/page".into(),
                tip: "a".into(),
                incoming: "b".into(),
                files: vec!["page.md".into()],
                phase: SyncPhase::Working,
                started_at: 1,
                ended_at: None,
                repair_attempted: false,
            }),
        };
        let back: SessionRecord =
            serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn sync_state_wire_shape() {
        let state = SyncState {
            conflict_session_id: "c1".into(),
            branch: "content/page".into(),
            tip: "a".into(),
            incoming: "b".into(),
            files: vec!["page.md".into()],
            phase: SyncPhase::Working,
            started_at: 1,
            ended_at: Some(2),
            repair_attempted: true,
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"conflictSessionId\":\"c1\""), "{json}");
        assert!(json.contains("\"startedAt\":1"), "{json}");
        assert!(json.contains("\"endedAt\":2"), "{json}");
        assert!(json.contains("\"phase\":\"working\""), "{json}");
        assert!(json.contains("\"repairAttempted\":true"), "{json}");
        let record = SessionRecord {
            id: "s1".into(),
            provider: "fake".into(),
            model: None,
            agent_session_id: None,
            title: None,
            created_at: 1,
            updated_at: 2,
            kind: RecordKind::Sync,
            cwd: None,
            sync: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"kind\":\"sync\""), "{json}");
        assert!(
            !json.contains("\"cwd\":") && !json.contains("\"sync\":"),
            "{json}"
        );
        assert!(!SyncPhase::Working.is_terminal());
        assert!(SyncPhase::Combined.is_terminal() && SyncPhase::Ended.is_terminal());
        assert!(SyncPhase::Expired.is_terminal() && !SyncPhase::Checking.is_terminal());
    }

    #[test]
    fn insert_list_update_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(dir.path(), "p").is_empty());
        insert(dir.path(), "p", rec("a", 1)).unwrap();
        insert(dir.path(), "p", rec("b", 2)).unwrap();
        let ids: Vec<_> = list(dir.path(), "p").into_iter().map(|r| r.id).collect();
        assert_eq!(ids, ["b", "a"]);

        let updated = update(dir.path(), "p", "a", |r| {
            r.title = Some("t".into());
            r.updated_at = 3;
        })
        .unwrap()
        .unwrap();
        assert_eq!(updated.title.as_deref(), Some("t"));
        let ids: Vec<_> = list(dir.path(), "p").into_iter().map(|r| r.id).collect();
        assert_eq!(ids, ["b", "a"], "updating a record keeps its place");
        assert_eq!(
            get(dir.path(), "p", "a").unwrap().title.as_deref(),
            Some("t")
        );
        assert!(update(dir.path(), "p", "nope", |_| {}).unwrap().is_none());
        assert!(list(dir.path(), "other").is_empty());
    }

    #[test]
    fn remove_drops_only_that_record() {
        let dir = tempfile::tempdir().unwrap();
        insert(dir.path(), "p", rec("a", 1)).unwrap();
        insert(dir.path(), "p", rec("b", 2)).unwrap();
        assert!(remove(dir.path(), "p", "a").unwrap());
        let ids: Vec<_> = list(dir.path(), "p").into_iter().map(|r| r.id).collect();
        assert_eq!(ids, ["b"]);
        assert!(!remove(dir.path(), "p", "a").unwrap());
        assert!(!remove(dir.path(), "other", "b").unwrap());
    }

    #[test]
    fn keeps_only_the_newest_records() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..(MAX_RECORDS as u64 + 5) {
            insert(dir.path(), "p", rec(&i.to_string(), i)).unwrap();
        }
        let records = list(dir.path(), "p");
        assert_eq!(records.len(), MAX_RECORDS);
        assert_eq!(records[0].id, (MAX_RECORDS as u64 + 4).to_string());
    }
}
