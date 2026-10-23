//! Append-only chat transcripts: one JSONL file per chat at
//! `{state_dir}/{project}/chats/{chat_id}.jsonl`, one [`UiChunk`] per line.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::chunk::UiChunk;

/// Serializes appends so two chunks never interleave a line.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// `^[A-Za-z0-9_-]{1,64}$`. `..`, slashes, and over-long ids never name a file.
pub fn valid_chat_id(id: &str) -> bool {
    valid_token(id)
}

/// Project ids are slugs (`[a-z0-9-]`, at most 48). The same token rule keeps
/// a crafted id from escaping `{state_dir}`.
pub fn valid_project_id(id: &str) -> bool {
    valid_token(id)
}

fn valid_token(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Transcript file for one chat. `None` when either id is not a safe path segment.
pub fn path(state_dir: &Path, project_id: &str, chat_id: &str) -> Option<PathBuf> {
    if !valid_project_id(project_id) || !valid_chat_id(chat_id) {
        return None;
    }
    Some(
        state_dir
            .join(project_id)
            .join("chats")
            .join(format!("{chat_id}.jsonl")),
    )
}

/// Appends one chunk as a JSON line, creating the chats dir on first use.
pub fn append(
    state_dir: &Path,
    project_id: &str,
    chat_id: &str,
    chunk: &UiChunk,
) -> std::io::Result<()> {
    let Some(path) = path(state_dir, project_id, chat_id) else {
        return Err(invalid_id(project_id, chat_id));
    };
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    write_chunk(&path, chunk)
}

fn invalid_id(project_id: &str, chat_id: &str) -> std::io::Error {
    let (which, id) = if !valid_project_id(project_id) {
        ("project", project_id)
    } else {
        ("chat", chat_id)
    };
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("invalid {which} id: {id}"),
    )
}

fn write_chunk(path: &Path, chunk: &UiChunk) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut line = serde_json::to_string(chunk)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    line.push('\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())
}

/// Every chunk recorded for a chat, oldest first. A missing file or an invalid
/// id is an empty history; unreadable lines are skipped, never fatal.
///
/// A turn left open (daemon kill mid-prompt, agent hang without `PromptEnded`)
/// is sealed with `turn_end` / `cancelled` so the panel does not stay busy.
pub fn load(state_dir: &Path, project_id: &str, chat_id: &str) -> Vec<UiChunk> {
    let Some(path) = path(state_dir, project_id, chat_id) else {
        return Vec::new();
    };
    // Held across the read and the seal so two loads cannot both append a
    // `turn_end`. `append` takes the same lock; this does not call it.
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut chunks: Vec<UiChunk> = text
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                None
            } else {
                serde_json::from_str(line).ok()
            }
        })
        .collect();
    seal_open_turn(&path, &mut chunks);
    chunks
}

fn turn_open(chunks: &[UiChunk]) -> bool {
    let mut open = false;
    for chunk in chunks {
        match chunk {
            UiChunk::UserText { .. } => open = true,
            UiChunk::TurnEnd { .. } | UiChunk::Error { .. } => open = false,
            _ => {}
        }
    }
    open
}

fn last_session_id(chunks: &[UiChunk]) -> String {
    for chunk in chunks.iter().rev() {
        let id = match chunk {
            UiChunk::TextDelta { session_id, .. }
            | UiChunk::ThoughtDelta { session_id, .. }
            | UiChunk::UserText { session_id, .. }
            | UiChunk::ToolCall { session_id, .. }
            | UiChunk::ToolCallUpdate { session_id, .. }
            | UiChunk::Plan { session_id, .. }
            | UiChunk::Permission { session_id, .. }
            | UiChunk::TurnEnd { session_id, .. }
            | UiChunk::Unknown { session_id, .. } => session_id.as_str(),
            UiChunk::Error { .. } | UiChunk::Checkpoint { .. } => continue,
        };
        if !id.is_empty() {
            return id.to_string();
        }
    }
    String::new()
}

fn seal_open_turn(path: &Path, chunks: &mut Vec<UiChunk>) {
    if !turn_open(chunks) {
        return;
    }
    let end = UiChunk::TurnEnd {
        session_id: last_session_id(chunks),
        stop_reason: "cancelled".into(),
    };
    let _ = write_chunk(path, &end);
    chunks.push(end);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(text: &str) -> UiChunk {
        UiChunk::TextDelta {
            session_id: "s1".into(),
            text: text.into(),
        }
    }

    #[test]
    fn append_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let hi = chunk("hi");
        append(dir.path(), "proj", "chat-1", &hi).unwrap();
        let file = path(dir.path(), "proj", "chat-1").unwrap();
        assert!(file.ends_with("proj/chats/chat-1.jsonl"));
        // A corrupt line is skipped; the valid chunks on either side remain.
        let raw = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, format!("{raw}not-json\n{raw}")).unwrap();
        assert_eq!(load(dir.path(), "proj", "chat-1"), vec![hi.clone(), hi]);
        assert!(load(dir.path(), "proj", "missing").is_empty());
    }

    #[test]
    fn reject_traversal_chat_id() {
        let dir = tempfile::tempdir().unwrap();
        let hi = chunk("hi");
        for id in ["../evil", "a/b", "", "..", &"a".repeat(65)] {
            assert!(path(dir.path(), "proj", id).is_none(), "{id}");
            assert!(append(dir.path(), "proj", id, &hi).is_err(), "{id}");
            assert!(load(dir.path(), "proj", id).is_empty(), "{id}");
        }
        assert!(!dir.path().join("evil.jsonl").exists());
        assert!(!dir.path().join("proj").join("evil.jsonl").exists());
        assert!(path(dir.path(), "../proj", "chat-1").is_none());
        assert!(append(dir.path(), "../proj", "chat-1", &hi).is_err());
    }

    #[test]
    fn seal_open_turn_on_load() {
        let dir = tempfile::tempdir().unwrap();
        append(
            dir.path(),
            "proj",
            "chat-2",
            &UiChunk::UserText {
                session_id: "acp-1".into(),
                text: "hi".into(),
            },
        )
        .unwrap();
        append(
            dir.path(),
            "proj",
            "chat-2",
            &UiChunk::TextDelta {
                session_id: "acp-1".into(),
                text: "partial".into(),
            },
        )
        .unwrap();
        let chunks = load(dir.path(), "proj", "chat-2");
        assert!(
            matches!(
                chunks.last(),
                Some(UiChunk::TurnEnd {
                    stop_reason,
                    session_id
                }) if stop_reason == "cancelled" && session_id == "acp-1"
            ),
            "{chunks:?}"
        );
        // A second load must not append another seal.
        assert_eq!(load(dir.path(), "proj", "chat-2").len(), chunks.len());
    }
}
