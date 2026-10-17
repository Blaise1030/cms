//! Cursor CLI `--output-format stream-json`.

use std::collections::HashSet;

use serde_json::{json, Map, Value};

use crate::chat::chunk::UiChunk;
use crate::chat::translate::{agent_error, truncate_input, truncate_output, Translator};

#[derive(Debug, Default)]
pub struct CursorTranslator {
    session_id: Option<String>,
    model: Option<String>,
    summary: Option<String>,
    texts: usize,
    reasonings: usize,
    open_reasoning: Option<String>,
    announced_tools: HashSet<String>,
    /// `result` text of an `is_error` result.
    failure: Option<String>,
}

impl CursorTranslator {
    fn close_reasoning(&mut self, out: &mut Vec<UiChunk>) {
        if let Some(id) = self.open_reasoning.take() {
            out.push(UiChunk::ReasoningEnd { id });
        }
    }

    fn thinking(&mut self, event: &Value, out: &mut Vec<UiChunk>) {
        match event["subtype"].as_str() {
            Some("delta") => {
                let Some(text) = event["text"].as_str().filter(|t| !t.is_empty()) else {
                    return;
                };
                let id = match &self.open_reasoning {
                    Some(id) => id.clone(),
                    None => {
                        let id = format!("r{}", self.reasonings);
                        self.reasonings += 1;
                        out.push(UiChunk::ReasoningStart { id: id.clone() });
                        self.open_reasoning = Some(id.clone());
                        id
                    }
                };
                out.push(UiChunk::ReasoningDelta {
                    id,
                    delta: text.to_string(),
                });
            }
            Some("completed") => self.close_reasoning(out),
            _ => {}
        }
    }

    fn assistant(&mut self, event: &Value, out: &mut Vec<UiChunk>) {
        self.close_reasoning(out);
        let text: String = event["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|part| part["type"] == "text")
            .filter_map(|part| part["text"].as_str())
            .collect();
        if text.is_empty() {
            return;
        }
        let id = format!("t{}", self.texts);
        self.texts += 1;
        out.push(UiChunk::TextStart { id: id.clone() });
        out.push(UiChunk::TextDelta {
            id: id.clone(),
            delta: text.clone(),
        });
        out.push(UiChunk::TextEnd { id });
        self.summary = Some(text);
    }

    fn tool_call(&mut self, event: &Value, out: &mut Vec<UiChunk>) {
        self.close_reasoning(out);
        let Some(id) = event["call_id"].as_str() else {
            return;
        };
        let (key, body) = event["tool_call"]
            .as_object()
            .and_then(|call| {
                call.iter()
                    .find(|(key, value)| key.ends_with("ToolCall") && value.is_object())
            })
            .map_or(("", &Value::Null), |(key, body)| (key.as_str(), body));
        let name = tool_name(key);
        if self.announced_tools.insert(id.to_string()) {
            out.push(UiChunk::ToolInputAvailable {
                tool_call_id: id.to_string(),
                tool_name: name.to_string(),
                input: truncate_input(tool_input(&body["args"])),
                dynamic: true,
            });
        }
        if event["subtype"] == "completed" {
            out.push(tool_result(name, id, &body["result"]));
        }
    }

    fn result(&mut self, event: &Value, out: &mut Vec<UiChunk>) {
        self.close_reasoning(out);
        if event["is_error"] == true {
            self.failure = Some(event["result"].as_str().unwrap_or_default().to_string());
        }
        out.push(UiChunk::MessageMetadata {
            message_metadata: json!({
                "provider": "cursor",
                "model": self.model,
                "usage": event["usage"],
            }),
        });
    }
}

impl Translator for CursorTranslator {
    fn feed(&mut self, line: &str) -> Vec<UiChunk> {
        let Ok(event) = serde_json::from_str::<Value>(line.trim()) else {
            return Vec::new();
        };
        if self.session_id.is_none() {
            self.session_id = event["session_id"].as_str().map(str::to_string);
        }
        let mut out = Vec::new();
        match event["type"].as_str() {
            Some("system") => {
                if let Some(model) = event["model"].as_str() {
                    self.model = Some(model.to_string());
                }
            }
            Some("thinking") => self.thinking(&event, &mut out),
            Some("assistant") => self.assistant(&event, &mut out),
            Some("tool_call") => self.tool_call(&event, &mut out),
            Some("result") => self.result(&event, &mut out),
            _ => {}
        }
        out
    }

    fn finish(&mut self, exit: Option<i32>, stderr_tail: &str) -> Vec<UiChunk> {
        let mut out = Vec::new();
        self.close_reasoning(&mut out);
        let failure = self.failure.take();
        if failure.is_some() || exit.is_some_and(|code| code != 0) {
            let text = failure
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| stderr_tail.to_string());
            out.push(UiChunk::DataAgentError {
                data: agent_error("cursor", exit, &text),
            });
        }
        out
    }

    fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }
}

/// The tool kind for the `*ToolCall` key under `tool_call`.
fn tool_name(key: &str) -> &'static str {
    match key {
        "readToolCall" => "read",
        "editToolCall" | "writeToolCall" | "deleteToolCall" => "edit",
        "shellToolCall" => "shell",
        "grepToolCall" | "globToolCall" | "lsToolCall" => "search",
        "webSearchToolCall" | "webFetchToolCall" => "web",
        "mcpToolCall" => "mcp",
        _ => "other",
    }
}

/// `{ path?, command?, query?, raw: args }`.
fn tool_input(args: &Value) -> Value {
    let mut input = Map::new();
    if let Some(path) = args["path"].as_str() {
        input.insert("path".into(), path.into());
    }
    if let Some(command) = args["command"].as_str() {
        input.insert("command".into(), command.into());
    }
    let query = ["query", "pattern", "globPattern", "searchTerm", "url"]
        .iter()
        .find_map(|key| args[key].as_str());
    if let Some(query) = query {
        input.insert("query".into(), query.into());
    }
    input.insert("raw".into(), args.clone());
    Value::Object(input)
}

fn tool_result(name: &str, id: &str, result: &Value) -> UiChunk {
    let tool_call_id = id.to_string();
    let Some(success) = result.get("success") else {
        let error = result
            .get("failure")
            .or_else(|| result.get("error"))
            .unwrap_or(result);
        return UiChunk::ToolOutputError {
            tool_call_id,
            error_text: error_text(error),
            dynamic: true,
        };
    };
    let output = match name {
        "edit" => success["diffString"]
            .as_str()
            .map(|diff| truncate_output(diff).into()),
        "shell" => Some(json!({
            "exitCode": success["exitCode"],
            "stdout": truncate_output(success["stdout"].as_str().unwrap_or_default()),
            "stderr": truncate_output(success["stderr"].as_str().unwrap_or_default()),
        })),
        "read" => Some(json!({ "lines": success["totalLines"] })),
        _ => None,
    };
    UiChunk::ToolOutputAvailable {
        tool_call_id,
        output: output.unwrap_or_else(|| truncate_output(&success.to_string()).into()),
        dynamic: true,
    }
}

fn error_text(error: &Value) -> String {
    if let Some(text) = error.as_str() {
        return truncate_output(text);
    }
    if let Some(code) = error.get("exitCode") {
        let mut text = format!("exit code {code}");
        for stream in ["stderr", "stdout"] {
            if let Some(s) = error[stream].as_str().filter(|s| !s.trim().is_empty()) {
                text.push('\n');
                text.push_str(s.trim_end());
            }
        }
        return truncate_output(&text);
    }
    let message = ["errorMessage", "error", "message"]
        .iter()
        .find_map(|key| error[key].as_str());
    truncate_output(message.unwrap_or(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::chunk::AgentErrorCode;
    use serde_json::{json, Value};

    const TURN: &str = include_str!("../../../tests/fixtures/chat/cursor/turn.jsonl");
    const RESUME: &str = include_str!("../../../tests/fixtures/chat/cursor/resume.jsonl");
    const TOOL_ERROR: &str = include_str!("../../../tests/fixtures/chat/cursor/tool-error.jsonl");

    const TURN_TEXT: &str = "I added the line to `about.md`, and `ls` shows two files in the directory: `about.md` and `CLAUDE.md`.";

    fn run(t: &mut CursorTranslator, fixture: &str) -> Vec<UiChunk> {
        fixture.lines().flat_map(|line| t.feed(line)).collect()
    }

    fn tool_input<'a>(chunks: &'a [UiChunk], name: &str) -> (usize, &'a str, &'a Value) {
        chunks
            .iter()
            .enumerate()
            .find_map(|(i, c)| match c {
                UiChunk::ToolInputAvailable {
                    tool_call_id,
                    tool_name,
                    input,
                    dynamic: true,
                } if tool_name == name => Some((i, tool_call_id.as_str(), input)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no {name} tool input in {chunks:#?}"))
    }

    fn tool_output<'a>(chunks: &'a [UiChunk], after: usize, id: &str) -> &'a Value {
        chunks[after + 1..]
            .iter()
            .find_map(|c| match c {
                UiChunk::ToolOutputAvailable {
                    tool_call_id,
                    output,
                    dynamic: true,
                } if tool_call_id == id => Some(output),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no output for {id} in {chunks:#?}"))
    }

    #[test]
    fn turn_edit_has_input_then_diff_output() {
        let chunks = run(&mut CursorTranslator::default(), TURN);
        let (at, id, input) = tool_input(&chunks, "edit");
        assert!(input["path"].as_str().unwrap().ends_with("about.md"));
        assert!(input["raw"]["streamContent"].is_string());
        let output = tool_output(&chunks, at, id);
        assert!(output.as_str().unwrap().contains("+Hello from the agent."));
    }

    #[test]
    fn long_edit_content_is_truncated() {
        let line = json!({
            "type": "tool_call",
            "subtype": "started",
            "call_id": "c1",
            "session_id": "s",
            "tool_call": {
                "editToolCall": { "args": { "path": "/a.md", "streamContent": "x".repeat(20_000) } }
            },
        })
        .to_string();
        let chunks = CursorTranslator::default().feed(&line);
        let (_, _, input) = tool_input(&chunks, "edit");
        let content = input["raw"]["streamContent"].as_str().unwrap();
        assert!(content.len() <= crate::chat::translate::MAX_OUTPUT + "…".len());
        assert!(content.ends_with('…'));
        assert_eq!(input["path"], "/a.md");
    }

    #[test]
    fn turn_shell_has_command_and_exit_code() {
        let chunks = run(&mut CursorTranslator::default(), TURN);
        let (at, id, input) = tool_input(&chunks, "shell");
        assert_eq!(input["command"], "ls");
        let output = tool_output(&chunks, at, id);
        assert_eq!(output["exitCode"], 0);
        assert!(output["stdout"].as_str().unwrap().contains("about.md"));
    }

    #[test]
    fn turn_read_reports_line_count_not_content() {
        let chunks = run(&mut CursorTranslator::default(), TURN);
        let (at, id, input) = tool_input(&chunks, "read");
        assert!(input["path"].as_str().unwrap().ends_with("about.md"));
        assert_eq!(tool_output(&chunks, at, id), &json!({"lines": 2}));
    }

    #[test]
    fn turn_text_session_and_metadata() {
        let mut t = CursorTranslator::default();
        let chunks = run(&mut t, TURN);
        let text: Vec<&UiChunk> = chunks
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    UiChunk::TextStart { .. } | UiChunk::TextDelta { .. } | UiChunk::TextEnd { .. }
                )
            })
            .collect();
        assert_eq!(
            text,
            [
                &UiChunk::TextStart { id: "t0".into() },
                &UiChunk::TextDelta {
                    id: "t0".into(),
                    delta: TURN_TEXT.into()
                },
                &UiChunk::TextEnd { id: "t0".into() },
            ]
        );
        let metadata = chunks
            .iter()
            .find_map(|c| match c {
                UiChunk::MessageMetadata { message_metadata } => Some(message_metadata),
                _ => None,
            })
            .expect("metadata");
        assert_eq!(metadata["provider"], "cursor");
        assert_eq!(metadata["model"], "Auto");
        assert_eq!(metadata["usage"]["outputTokens"], 327);
        assert_eq!(t.session_id(), Some("34f63cf5-7e32-4205-9b34-72582ec9f80e"));
        assert_eq!(t.summary(), Some(TURN_TEXT));
        assert_eq!(t.finish(Some(0), ""), []);
    }

    #[test]
    fn resume_is_a_single_text_part() {
        let mut t = CursorTranslator::default();
        let chunks = run(&mut t, RESUME);
        assert_eq!(chunks[0], UiChunk::TextStart { id: "t0".into() });
        assert!(!chunks
            .iter()
            .any(|c| matches!(c, UiChunk::ToolInputAvailable { .. })));
        assert!(t.summary().unwrap().contains("Hello from the agent."));
    }

    #[test]
    fn thinking_deltas_become_one_reasoning_part() {
        let mut t = CursorTranslator::default();
        let chunks: Vec<UiChunk> = [
            r#"{"type":"thinking","subtype":"delta","text":"Let me ","session_id":"s"}"#,
            r#"{"type":"thinking","subtype":"delta","text":"read it.","session_id":"s"}"#,
            r#"{"type":"thinking","subtype":"completed","session_id":"s"}"#,
        ]
        .iter()
        .flat_map(|line| t.feed(line))
        .collect();
        assert_eq!(
            chunks,
            [
                UiChunk::ReasoningStart { id: "r0".into() },
                UiChunk::ReasoningDelta {
                    id: "r0".into(),
                    delta: "Let me ".into()
                },
                UiChunk::ReasoningDelta {
                    id: "r0".into(),
                    delta: "read it.".into()
                },
                UiChunk::ReasoningEnd { id: "r0".into() },
            ]
        );
    }

    #[test]
    fn unfinished_reasoning_is_closed_by_finish() {
        let mut t = CursorTranslator::default();
        t.feed(r#"{"type":"thinking","subtype":"delta","text":"hmm","session_id":"s"}"#);
        assert_eq!(
            t.finish(Some(0), ""),
            [UiChunk::ReasoningEnd { id: "r0".into() }]
        );
    }

    #[test]
    fn shell_failure_is_a_tool_error() {
        let chunks = run(&mut CursorTranslator::default(), TOOL_ERROR);
        let (_, id, input) = tool_input(&chunks, "shell");
        assert_eq!(input["command"], "cat missing.txt");
        let error = chunks
            .iter()
            .find_map(|c| match c {
                UiChunk::ToolOutputError {
                    tool_call_id,
                    error_text,
                    dynamic: true,
                } if tool_call_id == id => Some(error_text),
                _ => None,
            })
            .expect("tool error");
        assert!(error.contains("exit code 1"), "{error}");
        assert!(error.contains("No such file or directory"), "{error}");
    }

    #[test]
    fn noise_is_ignored() {
        let mut t = CursorTranslator::default();
        assert_eq!(t.feed("not json"), []);
        assert_eq!(t.feed(""), []);
    }

    #[test]
    fn error_result_fails_the_turn() {
        let mut t = CursorTranslator::default();
        t.feed(r#"{"type":"result","subtype":"error","is_error":true,"result":"boom","session_id":"s"}"#);
        let chunks = t.finish(Some(1), "");
        let [UiChunk::DataAgentError { data }] = chunks.as_slice() else {
            panic!("{chunks:#?}");
        };
        assert_eq!(data.code, AgentErrorCode::Failed);
        assert!(data.message.contains("boom"));
    }

    #[test]
    fn nonzero_exit_without_result_uses_stderr() {
        let mut t = CursorTranslator::default();
        let chunks = t.finish(Some(1), "Error: not authenticated, run cursor-agent login");
        let [UiChunk::DataAgentError { data }] = chunks.as_slice() else {
            panic!("{chunks:#?}");
        };
        assert_eq!(data.code, AgentErrorCode::Login);
    }
}
