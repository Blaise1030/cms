//! Per-chat transcripts, `{state_dir}/{project}/chats/{chat}.jsonl`: one AI SDK `UIMessage` per line.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Map, Value};

use super::chunk::UiChunk;
use crate::terminal::history::now_ms;

/// Serializes appends to every transcript file.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

const TERMINAL_PART: &str = "data-terminal";

/// `^[A-Za-z0-9_-]{1,64}$`, except `turn`, which `DELETE /api/chat/{project}/turn` would shadow.
pub fn valid_chat_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        && id != "turn"
}

pub fn path(state_dir: &Path, project_id: &str, chat_id: &str) -> PathBuf {
    state_dir
        .join(project_id)
        .join("chats")
        .join(format!("{chat_id}.jsonl"))
}

fn read_file(file: &Path) -> Vec<Value> {
    std::fs::read_to_string(file)
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn append_file(file: &Path, messages: &[Value]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut lines = String::new();
    for message in messages {
        lines.push_str(&serde_json::to_string(message)?);
        lines.push('\n');
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)?
        .write_all(lines.as_bytes())
}

/// Oldest first. A missing file is an empty transcript; unparseable lines are skipped.
pub fn read(state_dir: &Path, project_id: &str, chat_id: &str) -> Vec<Value> {
    read_file(&path(state_dir, project_id, chat_id))
}

pub fn append(
    state_dir: &Path,
    project_id: &str,
    chat_id: &str,
    messages: &[Value],
) -> std::io::Result<()> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    append_file(&path(state_dir, project_id, chat_id), messages)
}

pub fn user_message(id: &str, text: &str) -> Value {
    json!({"id": id, "role": "user", "parts": [{"type": "text", "text": text}]})
}

/// The assistant `UIMessage` the AI SDK would build from `chunks`. Tools left without a
/// result (the turn was cut short) end as `output-error` "Interrupted".
pub fn assemble(message_id: &str, chunks: &[UiChunk]) -> Value {
    let mut parts: Vec<Value> = Vec::new();
    let mut text: HashMap<&str, usize> = HashMap::new();
    let mut reasoning: HashMap<&str, usize> = HashMap::new();
    let mut tools: HashMap<&str, usize> = HashMap::new();
    let mut metadata: Option<Value> = None;

    fn append_text(parts: &mut [Value], index: Option<&usize>, delta: &str) {
        if let Some(Value::String(text)) = index.and_then(|&i| parts[i].get_mut("text")) {
            text.push_str(delta);
        }
    }

    for chunk in chunks {
        match chunk {
            UiChunk::StartStep => parts.push(json!({"type": "step-start"})),
            UiChunk::TextStart { id } => {
                text.insert(id, parts.len());
                parts.push(json!({"type": "text", "text": ""}));
            }
            UiChunk::TextDelta { id, delta } => {
                append_text(&mut parts, text.get(id.as_str()), delta)
            }
            UiChunk::ReasoningStart { id } => {
                reasoning.insert(id, parts.len());
                parts.push(json!({"type": "reasoning", "text": ""}));
            }
            UiChunk::ReasoningDelta { id, delta } => {
                append_text(&mut parts, reasoning.get(id.as_str()), delta)
            }
            UiChunk::ToolInputAvailable {
                tool_call_id,
                tool_name,
                input,
                ..
            } => {
                let part = json!({
                    "type": "dynamic-tool",
                    "toolName": tool_name,
                    "toolCallId": tool_call_id,
                    "state": "input-available",
                    "input": input,
                });
                match tools.get(tool_call_id.as_str()) {
                    Some(&i) => parts[i] = part,
                    None => {
                        tools.insert(tool_call_id, parts.len());
                        parts.push(part);
                    }
                }
            }
            UiChunk::ToolOutputAvailable {
                tool_call_id,
                output,
                ..
            } => {
                if let Some(&i) = tools.get(tool_call_id.as_str()) {
                    parts[i]["state"] = json!("output-available");
                    parts[i]["output"] = output.clone();
                }
            }
            UiChunk::ToolOutputError {
                tool_call_id,
                error_text,
                ..
            } => {
                if let Some(&i) = tools.get(tool_call_id.as_str()) {
                    parts[i]["state"] = json!("output-error");
                    parts[i]["errorText"] = json!(error_text);
                }
            }
            UiChunk::DataCheckpoint { .. } | UiChunk::DataAgentError { .. } => {
                let value = serde_json::to_value(chunk).expect("UiChunk serializes");
                parts.push(json!({"type": value["type"], "data": value["data"]}));
            }
            UiChunk::MessageMetadata { message_metadata } => match &mut metadata {
                Some(existing) => merge(existing, message_metadata),
                None => metadata = Some(message_metadata.clone()),
            },
            UiChunk::Start { .. }
            | UiChunk::FinishStep
            | UiChunk::TextEnd { .. }
            | UiChunk::ReasoningEnd { .. }
            | UiChunk::Error { .. }
            | UiChunk::Abort
            | UiChunk::Finish => {}
        }
    }

    for &i in tools.values() {
        if parts[i]["state"] == "input-available" {
            parts[i]["state"] = json!("output-error");
            parts[i]["errorText"] = json!("Interrupted");
        }
    }

    let mut message = Map::new();
    message.insert("id".into(), json!(message_id));
    message.insert("role".into(), json!("assistant"));
    message.insert("parts".into(), Value::Array(parts));
    if let Some(metadata) = metadata {
        message.insert("metadata".into(), metadata);
    }
    Value::Object(message)
}

/// Deep-merges objects like the AI SDK's metadata handling; anything else replaces.
fn merge(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, overlay) => *base = overlay.clone(),
    }
}

fn is_terminal_marker(message: &Value) -> bool {
    message["parts"]
        .as_array()
        .is_some_and(|parts| parts.iter().any(|p| p["type"] == TERMINAL_PART))
}

/// Appends a `data-terminal` marker to an existing transcript, unless it already ends with one.
pub fn mark_terminal(state_dir: &Path, project_id: &str, chat_id: &str) -> std::io::Result<()> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let file = path(state_dir, project_id, chat_id);
    if !file.exists() {
        return Ok(());
    }
    if read_file(&file).last().is_some_and(is_terminal_marker) {
        return Ok(());
    }
    let marker = json!({
        "id": format!("term-{}", now_ms()),
        "role": "assistant",
        "parts": [{"type": TERMINAL_PART, "data": {}}],
    });
    append_file(&file, &[marker])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::chunk::{AgentError, AgentErrorCode};
    use serde_json::json;

    #[test]
    fn chat_ids() {
        assert!(!valid_chat_id(""));
        assert!(!valid_chat_id("../x"));
        assert!(!valid_chat_id("a/b"));
        assert!(!valid_chat_id(&"a".repeat(65)));
        assert!(
            !valid_chat_id("turn"),
            "shadowed by DELETE /api/chat/{{project}}/turn"
        );
        assert!(valid_chat_id(&"a".repeat(64)));
        assert!(valid_chat_id("6f9619ff-8b86-d011-b42d-00cf4fc964ff"));
        assert!(valid_chat_id("chat_1"));
    }

    #[test]
    fn path_layout() {
        assert_eq!(
            path(Path::new("/s"), "p", "c"),
            Path::new("/s/p/chats/c.jsonl")
        );
    }

    #[test]
    fn user_message_shape() {
        assert_eq!(
            user_message("u1", "hello"),
            json!({"id": "u1", "role": "user", "parts": [{"type": "text", "text": "hello"}]})
        );
    }

    #[test]
    fn assembles_text_and_tools() {
        let chunks = [
            UiChunk::Start {
                message_id: "ignored".into(),
            },
            UiChunk::StartStep,
            UiChunk::TextStart { id: "t0".into() },
            UiChunk::TextDelta {
                id: "t0".into(),
                delta: "Hi".into(),
            },
            UiChunk::TextEnd { id: "t0".into() },
            UiChunk::ToolInputAvailable {
                tool_call_id: "c1".into(),
                tool_name: "edit".into(),
                input: json!({"path": "a.md"}),
                dynamic: true,
            },
            UiChunk::ToolOutputAvailable {
                tool_call_id: "c1".into(),
                output: json!("ok"),
                dynamic: true,
            },
            UiChunk::ToolInputAvailable {
                tool_call_id: "c2".into(),
                tool_name: "shell".into(),
                input: json!({"command": "ls"}),
                dynamic: true,
            },
        ];
        assert_eq!(
            assemble("m1", &chunks),
            json!({
                "id": "m1",
                "role": "assistant",
                "parts": [
                    {"type": "step-start"},
                    {"type": "text", "text": "Hi"},
                    {
                        "type": "dynamic-tool",
                        "toolName": "edit",
                        "toolCallId": "c1",
                        "state": "output-available",
                        "input": {"path": "a.md"},
                        "output": "ok"
                    },
                    {
                        "type": "dynamic-tool",
                        "toolName": "shell",
                        "toolCallId": "c2",
                        "state": "output-error",
                        "input": {"command": "ls"},
                        "errorText": "Interrupted"
                    }
                ]
            })
        );
    }

    #[test]
    fn assembles_reasoning_errors_data_and_metadata() {
        let chunks = [
            UiChunk::ReasoningStart { id: "r0".into() },
            UiChunk::ReasoningDelta {
                id: "r0".into(),
                delta: "think".into(),
            },
            UiChunk::ReasoningDelta {
                id: "r0".into(),
                delta: "ing".into(),
            },
            UiChunk::ReasoningEnd { id: "r0".into() },
            UiChunk::ToolInputAvailable {
                tool_call_id: "c1".into(),
                tool_name: "shell".into(),
                input: json!({}),
                dynamic: true,
            },
            UiChunk::ToolOutputError {
                tool_call_id: "c1".into(),
                error_text: "boom".into(),
                dynamic: true,
            },
            UiChunk::DataAgentError {
                data: AgentError {
                    code: AgentErrorCode::Failed,
                    message: "exit 1".into(),
                },
            },
            UiChunk::MessageMetadata {
                message_metadata: json!({"model": "x"}),
            },
            UiChunk::FinishStep,
            UiChunk::Finish,
        ];
        assert_eq!(
            assemble("m2", &chunks),
            json!({
                "id": "m2",
                "role": "assistant",
                "parts": [
                    {"type": "reasoning", "text": "thinking"},
                    {
                        "type": "dynamic-tool",
                        "toolName": "shell",
                        "toolCallId": "c1",
                        "state": "output-error",
                        "input": {},
                        "errorText": "boom"
                    },
                    {"type": "data-agent-error", "data": {"code": "failed", "message": "exit 1"}}
                ],
                "metadata": {"model": "x"}
            })
        );
    }

    #[test]
    fn append_and_read_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(dir.path(), "p", "c").is_empty());
        let first = user_message("u1", "a");
        let second = assemble("m1", &[UiChunk::StartStep]);
        append(dir.path(), "p", "c", std::slice::from_ref(&first)).unwrap();
        append(dir.path(), "p", "c", std::slice::from_ref(&second)).unwrap();
        assert_eq!(read(dir.path(), "p", "c"), vec![first, second]);
    }

    #[test]
    fn mark_terminal_once() {
        let dir = tempfile::tempdir().unwrap();
        mark_terminal(dir.path(), "p", "missing").unwrap();
        assert!(!path(dir.path(), "p", "missing").exists());

        append(dir.path(), "p", "c", &[user_message("u1", "a")]).unwrap();
        mark_terminal(dir.path(), "p", "c").unwrap();
        mark_terminal(dir.path(), "p", "c").unwrap();
        let messages = read(dir.path(), "p", "c");
        assert_eq!(messages.len(), 2);
        let marker = &messages[1];
        assert!(marker["id"].as_str().unwrap().starts_with("term-"));
        assert_eq!(marker["role"], "assistant");
        assert_eq!(
            marker["parts"],
            json!([{"type": "data-terminal", "data": {}}])
        );
    }
}
