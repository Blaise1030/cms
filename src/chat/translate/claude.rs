//! Claude Code `-p --output-format stream-json --verbose`.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::chat::chunk::UiChunk;
use crate::chat::translate::{agent_error, truncate_input, truncate_output, Translator};

#[derive(Debug, Default)]
pub struct ClaudeTranslator {
    session_id: Option<String>,
    model: Option<String>,
    summary: Option<String>,
    texts: usize,
    reasonings: usize,
    /// Tool kind per announced `tool_use` id.
    tools: HashMap<String, &'static str>,
    /// `result` text of a failed `result`.
    failure: Option<String>,
    /// Text of a `<synthetic>` assistant message; only explains a failure.
    synthetic: Option<String>,
}

impl ClaudeTranslator {
    fn part(&mut self, reasoning: bool, text: &str, out: &mut Vec<UiChunk>) {
        if reasoning {
            let id = format!("r{}", self.reasonings);
            self.reasonings += 1;
            out.push(UiChunk::ReasoningStart { id: id.clone() });
            out.push(UiChunk::ReasoningDelta {
                id: id.clone(),
                delta: text.to_string(),
            });
            out.push(UiChunk::ReasoningEnd { id });
        } else {
            let id = format!("t{}", self.texts);
            self.texts += 1;
            out.push(UiChunk::TextStart { id: id.clone() });
            out.push(UiChunk::TextDelta {
                id: id.clone(),
                delta: text.to_string(),
            });
            out.push(UiChunk::TextEnd { id });
            self.summary = Some(text.to_string());
        }
    }

    fn assistant(&mut self, message: &Value, out: &mut Vec<UiChunk>) {
        let blocks = message["content"].as_array().map_or(&[][..], Vec::as_slice);
        if message["model"] == "<synthetic>" {
            let text = text_blocks(blocks);
            if !text.is_empty() {
                self.synthetic = Some(text);
            }
            return;
        }
        for block in blocks {
            match block["type"].as_str() {
                Some("text") => {
                    if let Some(text) = block["text"].as_str().filter(|t| !t.is_empty()) {
                        self.part(false, text, out);
                    }
                }
                Some("thinking") => {
                    if let Some(text) = block["thinking"].as_str().filter(|t| !t.is_empty()) {
                        self.part(true, text, out);
                    }
                }
                Some("tool_use") => self.tool_use(block, out),
                _ => {}
            }
        }
    }

    fn tool_use(&mut self, block: &Value, out: &mut Vec<UiChunk>) {
        let Some(id) = block["id"].as_str() else {
            return;
        };
        if self.tools.contains_key(id) {
            return;
        }
        let name = tool_name(block["name"].as_str().unwrap_or_default());
        self.tools.insert(id.to_string(), name);
        out.push(UiChunk::ToolInputAvailable {
            tool_call_id: id.to_string(),
            tool_name: name.to_string(),
            input: truncate_input(tool_input(&block["input"])),
            dynamic: true,
        });
    }

    fn user(&mut self, message: &Value, out: &mut Vec<UiChunk>) {
        for block in message["content"].as_array().into_iter().flatten() {
            if block["type"] != "tool_result" {
                continue;
            }
            let Some(id) = block["tool_use_id"].as_str() else {
                continue;
            };
            if let Some(name) = self.tools.get(id) {
                out.push(tool_result(name, id, block));
            }
        }
    }

    fn result(&mut self, event: &Value, out: &mut Vec<UiChunk>) {
        if result_failed(event) {
            self.failure = Some(event["result"].as_str().unwrap_or_default().to_string());
        }
        out.push(UiChunk::MessageMetadata {
            message_metadata: json!({
                "provider": "claude",
                "model": self.model,
                "usage": event["usage"],
                "costUsd": event["total_cost_usd"],
            }),
        });
    }
}

impl Translator for ClaudeTranslator {
    fn feed(&mut self, line: &str) -> Vec<UiChunk> {
        let Ok(event) = serde_json::from_str::<Value>(line.trim()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        match event["type"].as_str() {
            Some("system") if event["subtype"] == "init" => {
                self.session_id = event["session_id"].as_str().map(str::to_string);
                self.model = event["model"].as_str().map(str::to_string);
            }
            Some("assistant") => self.assistant(&event["message"], &mut out),
            Some("user") => self.user(&event["message"], &mut out),
            Some("result") => self.result(&event, &mut out),
            _ => {}
        }
        out
    }

    fn finish(&mut self, exit: Option<i32>, stderr_tail: &str) -> Vec<UiChunk> {
        let mut out = Vec::new();
        let failure = self.failure.take();
        if failure.is_some() || exit.is_some_and(|code| code != 0) {
            let text = failure
                .into_iter()
                .chain(self.synthetic.take())
                .find(|text| !text.trim().is_empty())
                .unwrap_or_else(|| stderr_tail.to_string());
            out.push(UiChunk::DataAgentError {
                data: agent_error("claude", exit, &text),
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

/// Whether a `result` event ends a failed turn. The success `terminal_reason` is unverified, so
/// only reasons naming an error count.
fn result_failed(result: &Value) -> bool {
    result["is_error"] == true
        || result["terminal_reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("error"))
}

/// The tool kind for a Claude Code tool name.
fn tool_name(name: &str) -> &'static str {
    match name {
        "Read" => "read",
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => "edit",
        "Bash" => "shell",
        "Grep" | "Glob" | "LS" => "search",
        "WebFetch" | "WebSearch" => "web",
        _ if name.starts_with("mcp__") => "mcp",
        _ => "other",
    }
}

/// `{ path?, command?, query?, raw: input }`.
fn tool_input(args: &Value) -> Value {
    let mut input = Map::new();
    let path = ["file_path", "notebook_path", "path"]
        .iter()
        .find_map(|key| args[key].as_str());
    if let Some(path) = path {
        input.insert("path".into(), path.into());
    }
    if let Some(command) = args["command"].as_str() {
        input.insert("command".into(), command.into());
    }
    let query = ["pattern", "query", "url"]
        .iter()
        .find_map(|key| args[key].as_str());
    if let Some(query) = query {
        input.insert("query".into(), query.into());
    }
    input.insert("raw".into(), args.clone());
    Value::Object(input)
}

/// The text of `{type:"text"}` blocks, one per line.
fn text_blocks(blocks: &[Value]) -> String {
    blocks
        .iter()
        .filter(|block| block["type"] == "text")
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool_result(name: &str, id: &str, block: &Value) -> UiChunk {
    let tool_call_id = id.to_string();
    let content = &block["content"];
    let text = match content.as_array() {
        Some(blocks) => text_blocks(blocks),
        None => content.as_str().unwrap_or_default().to_string(),
    };
    let text = truncate_output(&text);
    if block["is_error"] == true {
        return UiChunk::ToolOutputError {
            tool_call_id,
            error_text: text,
            dynamic: true,
        };
    }
    let output = match name {
        "shell" => json!({ "stdout": text }),
        _ => text.into(),
    };
    UiChunk::ToolOutputAvailable {
        tool_call_id,
        output,
        dynamic: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::chunk::{AgentError, AgentErrorCode};
    use crate::chat::translate::MAX_OUTPUT;
    use serde_json::{json, Value};

    const TURN: &str = include_str!("../../../tests/fixtures/chat/claude/turn.jsonl");
    const RESUME: &str = include_str!("../../../tests/fixtures/chat/claude/resume.jsonl");
    const TOOL_ERROR: &str = include_str!("../../../tests/fixtures/chat/claude/tool-error.jsonl");
    const LOGIN_EXPIRED: &str =
        include_str!("../../../tests/fixtures/chat/claude/login-expired.jsonl");

    const TURN_TEXT: &str =
        "I appended \"Hello from the agent.\" to about.md, and `ls` shows it is the only file.";

    fn run(t: &mut ClaudeTranslator, fixture: &str) -> Vec<UiChunk> {
        fixture.lines().flat_map(|line| t.feed(line)).collect()
    }

    fn tool_inputs<'a>(chunks: &'a [UiChunk], name: &str) -> Vec<(usize, &'a str, &'a Value)> {
        chunks
            .iter()
            .enumerate()
            .filter_map(|(i, c)| match c {
                UiChunk::ToolInputAvailable {
                    tool_call_id,
                    tool_name,
                    input,
                    dynamic: true,
                } if tool_name == name => Some((i, tool_call_id.as_str(), input)),
                _ => None,
            })
            .collect()
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

    fn tool_error<'a>(chunks: &'a [UiChunk], id: &str) -> &'a str {
        chunks
            .iter()
            .find_map(|c| match c {
                UiChunk::ToolOutputError {
                    tool_call_id,
                    error_text,
                    dynamic: true,
                } if tool_call_id == id => Some(error_text.as_str()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no tool error for {id} in {chunks:#?}"))
    }

    fn agent_error(chunks: &[UiChunk]) -> &AgentError {
        let [UiChunk::DataAgentError { data }] = chunks else {
            panic!("{chunks:#?}");
        };
        data
    }

    fn is_text(c: &UiChunk) -> bool {
        matches!(
            c,
            UiChunk::TextStart { .. } | UiChunk::TextDelta { .. } | UiChunk::TextEnd { .. }
        )
    }

    #[test]
    fn hook_lines_produce_nothing() {
        let mut t = ClaudeTranslator::default();
        for line in TURN.lines().take(2).chain(LOGIN_EXPIRED.lines().take(14)) {
            assert_eq!(t.feed(line), [], "{line}");
        }
        assert_eq!(t.session_id(), None);
    }

    #[test]
    fn turn_text_and_session() {
        let mut t = ClaudeTranslator::default();
        let chunks = run(&mut t, TURN);
        let text: Vec<&UiChunk> = chunks.iter().filter(|c| is_text(c)).collect();
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
        assert_eq!(t.session_id(), Some("5b0e6f1a-2c3d-4e5f-8a9b-0c1d2e3f4a5b"));
        assert_eq!(t.summary(), Some(TURN_TEXT));
        assert_eq!(t.finish(Some(0), ""), []);
    }

    #[test]
    fn turn_thinking_is_a_reasoning_part() {
        let chunks = run(&mut ClaudeTranslator::default(), TURN);
        let thinking = "I need to read about.md, append the line, then run ls.";
        assert_eq!(
            chunks[..3],
            [
                UiChunk::ReasoningStart { id: "r0".into() },
                UiChunk::ReasoningDelta {
                    id: "r0".into(),
                    delta: thinking.into()
                },
                UiChunk::ReasoningEnd { id: "r0".into() },
            ]
        );
    }

    #[test]
    fn turn_tools_have_inputs_then_outputs() {
        let chunks = run(&mut ClaudeTranslator::default(), TURN);
        let inputs = chunks
            .iter()
            .filter(|c| matches!(c, UiChunk::ToolInputAvailable { .. }))
            .count();
        assert_eq!(inputs, 3);

        let [(at, id, input)] = tool_inputs(&chunks, "read")[..] else {
            panic!("{chunks:#?}");
        };
        assert_eq!(id, "toolu_01ReadAAAAAAAAAAAAAAAAAA");
        assert_eq!(input["path"], "/work/project/about.md");
        assert_eq!(input["raw"]["file_path"], "/work/project/about.md");
        assert_eq!(tool_output(&chunks, at, id), "     1\t# About\n");

        let [(at, id, input)] = tool_inputs(&chunks, "edit")[..] else {
            panic!("{chunks:#?}");
        };
        assert_eq!(input["path"], "/work/project/about.md");
        assert_eq!(
            input["raw"]["new_string"],
            "# About\nHello from the agent.\n"
        );
        assert!(tool_output(&chunks, at, id)
            .as_str()
            .unwrap()
            .contains("has been updated"));

        let [(at, id, input)] = tool_inputs(&chunks, "shell")[..] else {
            panic!("{chunks:#?}");
        };
        assert_eq!(input["command"], "ls");
        assert_eq!(input["raw"]["description"], "List files in the project");
        assert_eq!(
            tool_output(&chunks, at, id),
            &json!({ "stdout": "about.md" })
        );
    }

    #[test]
    fn turn_result_is_metadata() {
        let chunks = run(&mut ClaudeTranslator::default(), TURN);
        let metadata = chunks
            .iter()
            .find_map(|c| match c {
                UiChunk::MessageMetadata { message_metadata } => Some(message_metadata),
                _ => None,
            })
            .expect("metadata");
        assert_eq!(metadata["provider"], "claude");
        assert_eq!(metadata["model"], "claude-sonnet-4-5");
        assert_eq!(metadata["usage"]["output_tokens"], 167);
        assert_eq!(metadata["costUsd"], 0.01);
    }

    #[test]
    fn resume_is_a_single_text_part() {
        let mut t = ClaudeTranslator::default();
        let chunks = run(&mut t, RESUME);
        assert_eq!(chunks[0], UiChunk::TextStart { id: "t0".into() });
        assert!(!chunks
            .iter()
            .any(|c| matches!(c, UiChunk::ToolInputAvailable { .. })));
        assert_eq!(
            t.summary(),
            Some("I added the line \"Hello from the agent.\" to about.md.")
        );
        assert_eq!(t.finish(Some(0), ""), []);
    }

    #[test]
    fn failed_tool_result_is_a_tool_error() {
        let mut t = ClaudeTranslator::default();
        let chunks = run(&mut t, TOOL_ERROR);
        let [(_, id, input)] = tool_inputs(&chunks, "shell")[..] else {
            panic!("{chunks:#?}");
        };
        assert_eq!(input["command"], "cat missing.txt");
        let error = tool_error(&chunks, id);
        assert!(error.contains("Exit code 1"), "{error}");
        assert!(error.contains("No such file or directory"), "{error}");
        assert_eq!(t.finish(Some(0), ""), []);
    }

    #[test]
    fn login_expired_is_one_login_error() {
        let mut t = ClaudeTranslator::default();
        let chunks = run(&mut t, LOGIN_EXPIRED);
        assert!(!chunks.iter().any(is_text), "{chunks:#?}");
        assert_eq!(t.summary(), None);
        let finished = t.finish(Some(1), "");
        let data = agent_error(&finished);
        assert_eq!(data.code, AgentErrorCode::Login);
        assert_eq!(
            data.message,
            "Your Claude Code login has expired. Log in again from your terminal, then try again."
        );
    }

    #[test]
    fn error_terminal_reason_fails_the_turn() {
        let mut t = ClaudeTranslator::default();
        t.feed(r#"{"type":"result","subtype":"success","is_error":false,"terminal_reason":"api_error","result":"Overloaded","session_id":"s"}"#);
        let chunks = t.finish(Some(0), "");
        let data = agent_error(&chunks);
        assert_eq!(data.code, AgentErrorCode::Failed);
        assert_eq!(
            data.message,
            "Claude Code stopped with an error: Overloaded"
        );
    }

    #[test]
    fn synthetic_text_explains_an_error_result() {
        let mut t = ClaudeTranslator::default();
        let chunks = run(
            &mut t,
            concat!(
                r#"{"type":"assistant","message":{"model":"<synthetic>","content":[{"type":"text","text":"API Error: 529 overloaded"}]},"session_id":"s"}"#,
                "\n",
                r#"{"type":"result","is_error":true,"result":"","session_id":"s"}"#,
            ),
        );
        assert!(!chunks.iter().any(is_text));
        let data = agent_error(&t.finish(Some(1), "stderr text")).clone();
        assert!(
            data.message.contains("API Error: 529 overloaded"),
            "{data:?}"
        );
    }

    #[test]
    fn nonzero_exit_without_result_uses_stderr() {
        let mut t = ClaudeTranslator::default();
        let chunks = t.finish(Some(1), "Error: boom");
        assert!(agent_error(&chunks).message.contains("Error: boom"));
    }

    #[test]
    fn tool_result_text_blocks_are_joined() {
        let mut t = ClaudeTranslator::default();
        let chunks = run(
            &mut t,
            concat!(
                r#"{"type":"assistant","message":{"model":"m","content":[{"type":"tool_use","id":"g1","name":"Grep","input":{"pattern":"Hello","path":"/work"}},{"type":"tool_use","id":"m1","name":"mcp__docs__search","input":{"q":"x"}}]},"session_id":"s"}"#,
                "\n",
                r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"g1","content":[{"type":"text","text":"a.md"},{"type":"image","source":{}},{"type":"text","text":"b.md"}]}]},"session_id":"s"}"#,
                "\n",
                r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"m1","content":[{"type":"text","text":"timed out"}],"is_error":true}]},"session_id":"s"}"#,
            ),
        );
        let [(at, id, input)] = tool_inputs(&chunks, "search")[..] else {
            panic!("{chunks:#?}");
        };
        assert_eq!(input["query"], "Hello");
        assert_eq!(input["path"], "/work");
        assert_eq!(tool_output(&chunks, at, id), "a.md\nb.md");
        let [(_, id, input)] = tool_inputs(&chunks, "mcp")[..] else {
            panic!("{chunks:#?}");
        };
        assert_eq!(input["raw"], json!({"q": "x"}));
        assert_eq!(tool_error(&chunks, id), "timed out");
    }

    #[test]
    fn long_input_and_output_are_truncated() {
        let long = "x".repeat(20_000);
        let mut t = ClaudeTranslator::default();
        let use_line = json!({"type": "assistant", "message": {"model": "m", "content": [{"type": "tool_use", "id": "w1", "name": "Write", "input": {"file_path": "/a.md", "content": long}}]}}).to_string();
        let result_line = json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "w1", "content": long}]}}).to_string();
        let chunks: Vec<UiChunk> = [use_line, result_line]
            .iter()
            .flat_map(|line| t.feed(line))
            .collect();
        let [(at, id, input)] = tool_inputs(&chunks, "edit")[..] else {
            panic!("{chunks:#?}");
        };
        let content = input["raw"]["content"].as_str().unwrap();
        assert!(content.len() <= MAX_OUTPUT + "…".len() && content.ends_with('…'));
        let output = tool_output(&chunks, at, id).as_str().unwrap();
        assert!(output.len() <= MAX_OUTPUT + "…".len() && output.ends_with('…'));
    }

    #[test]
    fn tool_names() {
        for (name, kind) in [
            ("Read", "read"),
            ("Edit", "edit"),
            ("Write", "edit"),
            ("MultiEdit", "edit"),
            ("NotebookEdit", "edit"),
            ("Bash", "shell"),
            ("Grep", "search"),
            ("Glob", "search"),
            ("LS", "search"),
            ("WebFetch", "web"),
            ("WebSearch", "web"),
            ("mcp__docs__search", "mcp"),
            ("TodoWrite", "other"),
        ] {
            assert_eq!(tool_name(name), kind, "{name}");
        }
    }

    #[test]
    fn noise_is_ignored() {
        let mut t = ClaudeTranslator::default();
        assert_eq!(t.feed("not json"), []);
        assert_eq!(t.feed(""), []);
        assert_eq!(
            t.feed(r#"{"type":"system","subtype":"compact_boundary"}"#),
            []
        );
    }
}
