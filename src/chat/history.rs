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
        }
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
