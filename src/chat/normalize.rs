//! ACP events → the [`UiChunk`]s the browser renders.

use serde_json::Value;

use super::acp::AcpEvent;
use super::chunk::UiChunk;

fn without_discriminator(update: &Value) -> Value {
    let mut update = update.clone();
    if let Some(obj) = update.as_object_mut() {
        obj.remove("sessionUpdate");
    }
    update
}

pub fn to_chunks(ev: &AcpEvent) -> Vec<UiChunk> {
    let chunk = match ev {
        AcpEvent::AgentMessageChunk { session_id, text } => UiChunk::TextDelta {
            session_id: session_id.clone(),
            text: text.clone(),
        },
        AcpEvent::AgentThoughtChunk { session_id, text } => UiChunk::ThoughtDelta {
            session_id: session_id.clone(),
            text: text.clone(),
        },
        AcpEvent::UserMessageChunk { session_id, text } => UiChunk::UserText {
            session_id: session_id.clone(),
            text: text.clone(),
        },
        AcpEvent::ToolCall { session_id, update } => UiChunk::ToolCall {
            session_id: session_id.clone(),
            tool_call: without_discriminator(update),
        },
        AcpEvent::ToolCallUpdate { session_id, update } => UiChunk::ToolCallUpdate {
            session_id: session_id.clone(),
            tool_call: without_discriminator(update),
        },
        AcpEvent::Plan { session_id, update } => UiChunk::Plan {
            session_id: session_id.clone(),
            entries: update.get("entries").cloned().unwrap_or(Value::Null),
        },
        AcpEvent::PermissionRequest {
            session_id,
            request_id,
            tool_call,
            options,
        } => UiChunk::Permission {
            session_id: session_id.clone(),
            request_id: request_id.clone(),
            tool_call: tool_call.clone(),
            options: options.clone(),
        },
        AcpEvent::PromptEnded {
            session_id,
            stop_reason,
        } => UiChunk::TurnEnd {
            session_id: session_id.clone(),
            stop_reason: stop_reason.clone(),
        },
        // Status mapping consumes this; the transcript has no chunk for it.
        AcpEvent::SessionEnded { .. } | AcpEvent::AgentExited => return Vec::new(),
        AcpEvent::Unknown { session_id, update } => UiChunk::Unknown {
            session_id: session_id.clone(),
            update: update.clone(),
        },
    };
    vec![chunk]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_message_chunk_serializes_with_type_tag() {
        let chunks = to_chunks(&AcpEvent::AgentMessageChunk {
            session_id: "s1".into(),
            text: "hi".into(),
        });
        assert_eq!(
            serde_json::to_value(&chunks[0]).unwrap(),
            serde_json::json!({"type": "text_delta", "session_id": "s1", "text": "hi"})
        );
    }

    #[test]
    fn prompt_ended_becomes_turn_end() {
        let chunks = to_chunks(&AcpEvent::PromptEnded {
            session_id: "s1".into(),
            stop_reason: "cancelled".into(),
        });
        assert_eq!(
            serde_json::to_value(&chunks[0]).unwrap(),
            serde_json::json!({"type": "turn_end", "session_id": "s1", "stop_reason": "cancelled"})
        );
    }
}
