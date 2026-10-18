//! AI SDK `UIMessageChunk`s and their SSE framing.

use serde::Serialize;
use serde_json::Value;

/// One AI SDK UI message stream chunk. Tool parts are always `dynamic`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum UiChunk {
    Start {
        message_id: String,
    },
    StartStep,
    FinishStep,
    TextStart {
        id: String,
    },
    TextDelta {
        id: String,
        delta: String,
    },
    TextEnd {
        id: String,
    },
    ReasoningStart {
        id: String,
    },
    ReasoningDelta {
        id: String,
        delta: String,
    },
    ReasoningEnd {
        id: String,
    },
    ToolInputAvailable {
        tool_call_id: String,
        tool_name: String,
        input: Value,
        dynamic: bool,
    },
    ToolOutputAvailable {
        tool_call_id: String,
        output: Value,
        dynamic: bool,
    },
    ToolOutputError {
        tool_call_id: String,
        error_text: String,
        dynamic: bool,
    },
    #[serde(rename = "data-checkpoint")]
    DataCheckpoint {
        data: CheckpointData,
    },
    #[serde(rename = "data-agent-error")]
    DataAgentError {
        data: AgentError,
    },
    MessageMetadata {
        message_metadata: Value,
    },
    Error {
        error_text: String,
    },
    Abort,
    Finish,
}

/// What the turn's checkpoint did: committed (`sha`/`summary`/`files`) or `blocked`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<String>,
}

/// A CLI failure shown to the user as a data part.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentError {
    pub code: AgentErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentErrorCode {
    Login,
    NotInstalled,
    Failed,
}

/// `data: {json}\n\n`.
pub fn sse_frame(chunk: &UiChunk) -> String {
    let json = serde_json::to_string(chunk).expect("UiChunk serializes");
    format!("data: {json}\n\n")
}

/// Terminates an AI SDK UI message stream.
pub const SSE_DONE: &str = "data: [DONE]\n\n";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn to_json(chunk: &UiChunk) -> String {
        serde_json::to_string(chunk).unwrap()
    }

    #[test]
    fn tool_input_available_json() {
        let chunk = UiChunk::ToolInputAvailable {
            tool_call_id: "c1".into(),
            tool_name: "edit".into(),
            input: json!({"path": "a.md"}),
            dynamic: true,
        };
        assert_eq!(
            to_json(&chunk),
            r#"{"type":"tool-input-available","toolCallId":"c1","toolName":"edit","input":{"path":"a.md"},"dynamic":true}"#
        );
    }

    #[test]
    fn checkpoint_skips_missing_fields() {
        let chunk = UiChunk::DataCheckpoint {
            data: CheckpointData {
                files: vec!["a.md".into()],
                ..Default::default()
            },
        };
        assert_eq!(
            to_json(&chunk),
            r#"{"type":"data-checkpoint","data":{"files":["a.md"]}}"#
        );
    }

    #[test]
    fn agent_error_json() {
        let chunk = UiChunk::DataAgentError {
            data: AgentError {
                code: AgentErrorCode::NotInstalled,
                message: "…".into(),
            },
        };
        assert_eq!(
            to_json(&chunk),
            r#"{"type":"data-agent-error","data":{"code":"not_installed","message":"…"}}"#
        );
    }

    #[test]
    fn start_json() {
        let chunk = UiChunk::Start {
            message_id: "m1".into(),
        };
        assert_eq!(to_json(&chunk), r#"{"type":"start","messageId":"m1"}"#);
    }

    #[test]
    fn unit_variants_are_kebab_case() {
        assert_eq!(to_json(&UiChunk::StartStep), r#"{"type":"start-step"}"#);
        assert_eq!(to_json(&UiChunk::Finish), r#"{"type":"finish"}"#);
    }

    #[test]
    fn sse_frame_wraps_json() {
        let frame = sse_frame(&UiChunk::Abort);
        assert_eq!(frame, "data: {\"type\":\"abort\"}\n\n");
        assert!(frame.ends_with("\n\n"));
    }
}
