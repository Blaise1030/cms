//! Tabsh-shaped chat chunks and their SSE framing.
//!
//! One flat object per transcript line / SSE `data:` payload, discriminated
//! by `type` (snake_case). CMS adds `checkpoint`; `blocked` is the reason a
//! commit did not happen, not a boolean.

use serde::{Deserialize, Serialize};

/// One line of the chat transcript / one SSE `data:` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiChunk {
    /// Streaming agent text (`agent_message_chunk`).
    TextDelta { session_id: String, text: String },
    /// Streaming agent reasoning text (`agent_thought_chunk`).
    ThoughtDelta { session_id: String, text: String },
    /// Echo of a user message chunk.
    UserText { session_id: String, text: String },
    /// A tool call; `tool_call` is the raw ACP update minus its
    /// `sessionUpdate` discriminator.
    ToolCall {
        session_id: String,
        tool_call: serde_json::Value,
    },
    ToolCallUpdate {
        session_id: String,
        tool_call: serde_json::Value,
    },
    /// The agent's plan; `entries` is the raw ACP plan entries array.
    Plan {
        session_id: String,
        entries: serde_json::Value,
    },
    /// A permission request. `request_id` is the JSON-RPC id (stringified).
    Permission {
        session_id: String,
        request_id: String,
        tool_call: serde_json::Value,
        options: serde_json::Value,
    },
    /// A prompt turn finished; `stop_reason` is the ACP `stopReason`
    /// (e.g. `end_turn`, `cancelled`).
    TurnEnd {
        session_id: String,
        stop_reason: String,
    },
    /// Anything else, kept raw so no agent output is silently dropped.
    Unknown {
        session_id: String,
        update: serde_json::Value,
    },
    /// A spawn, auth, or protocol failure.
    Error { message: String },
    /// CMS checkpoint after a turn. `blocked` is why the commit did not happen.
    Checkpoint {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sha: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
        #[serde(default)]
        files: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blocked: Option<String>,
    },
}

/// `data: {json}\n\n`.
pub fn sse_frame(chunk: &UiChunk) -> String {
    let json = serde_json::to_string(chunk).expect("UiChunk serializes");
    format!("data: {json}\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn checkpoint_chunk_round_trips() {
        let chunk = UiChunk::Checkpoint {
            sha: Some("abc".into()),
            summary: Some("saved".into()),
            files: vec!["a.md".into()],
            blocked: Some("protected branch".into()),
        };
        let value = serde_json::to_value(&chunk).unwrap();
        assert_eq!(
            value,
            json!({
                "type": "checkpoint",
                "sha": "abc",
                "summary": "saved",
                "files": ["a.md"],
                "blocked": "protected branch"
            })
        );
        let back: UiChunk = serde_json::from_value(value).unwrap();
        assert_eq!(back, chunk);

        let empty = UiChunk::Checkpoint {
            sha: None,
            summary: None,
            files: vec![],
            blocked: None,
        };
        let json = serde_json::to_string(&empty).unwrap();
        assert_eq!(json, r#"{"type":"checkpoint","files":[]}"#);
        assert_eq!(serde_json::from_str::<UiChunk>(&json).unwrap(), empty);

        // `blocked` is a reason string. A boolean is not this field.
        assert!(serde_json::from_value::<UiChunk>(json!({
            "type": "checkpoint",
            "files": [],
            "blocked": true
        }))
        .is_err());
    }

    #[test]
    fn sse_frame_is_a_data_event() {
        let frame = sse_frame(&UiChunk::Error {
            message: "nope".into(),
        });
        assert_eq!(frame, "data: {\"type\":\"error\",\"message\":\"nope\"}\n\n");
    }
}
