//! OpenCode `run --format json --thinking`. Text and reasoning arrive as whole parts, tools
//! only once finished (`state.status` `completed` or `error`).

use serde_json::{json, Map, Value};

use crate::chat::chunk::UiChunk;
use crate::chat::translate::{agent_error, truncate_input, truncate_output, Translator};

#[derive(Debug, Default)]
pub struct OpencodeTranslator {
    session_id: Option<String>,
    summary: Option<String>,
    texts: usize,
    reasonings: usize,
    /// Token counts summed over the turn's steps.
    usage: Map<String, Value>,
    /// Message of the last `error` event.
    failure: Option<String>,
}

impl OpencodeTranslator {
    fn whole(&mut self, reasoning: bool, text: &str, out: &mut Vec<UiChunk>) {
        if text.is_empty() {
            return;
        }
        let counter = if reasoning {
            &mut self.reasonings
        } else {
            &mut self.texts
        };
        let id = format!("{}{counter}", if reasoning { "r" } else { "t" });
        *counter += 1;
        let delta = text.to_string();
        if reasoning {
            out.push(UiChunk::ReasoningStart { id: id.clone() });
            out.push(UiChunk::ReasoningDelta {
                id: id.clone(),
                delta,
            });
            out.push(UiChunk::ReasoningEnd { id });
        } else {
            out.push(UiChunk::TextStart { id: id.clone() });
            out.push(UiChunk::TextDelta {
                id: id.clone(),
                delta,
            });
            out.push(UiChunk::TextEnd { id });
            self.summary = Some(text.to_string());
        }
    }

    fn add_usage(&mut self, tokens: &Value) {
        for key in ["total", "input", "output", "reasoning"] {
            let sum = self.usage.get(key).and_then(Value::as_i64).unwrap_or(0)
                + tokens[key].as_i64().unwrap_or(0);
            self.usage.insert(key.into(), sum.into());
        }
    }
}

impl Translator for OpencodeTranslator {
    fn feed(&mut self, line: &str) -> Vec<UiChunk> {
        let Ok(event) = serde_json::from_str::<Value>(line.trim()) else {
            return Vec::new();
        };
        if let Some(id) = event["sessionID"].as_str() {
            self.session_id.get_or_insert_with(|| id.to_string());
        }
        let part = &event["part"];
        let mut out = Vec::new();
        match event["type"].as_str() {
            Some("text") => self.whole(false, part["text"].as_str().unwrap_or_default(), &mut out),
            Some("reasoning") => {
                self.whole(true, part["text"].as_str().unwrap_or_default(), &mut out)
            }
            Some("tool_use") => tool(part, &mut out),
            Some("step_finish") => {
                self.add_usage(&part["tokens"]);
                if part["reason"] != "tool-calls" {
                    out.push(UiChunk::MessageMetadata {
                        message_metadata: json!({
                            "provider": "opencode",
                            "usage": self.usage,
                        }),
                    });
                }
            }
            Some("error") => {
                let error = &event["error"];
                self.failure = error["data"]["message"]
                    .as_str()
                    .or(error["name"].as_str())
                    .map(str::to_string);
            }
            _ => {}
        }
        out
    }

    fn finish(&mut self, exit: Option<i32>, stderr_tail: &str) -> Vec<UiChunk> {
        let failure = self.failure.take();
        if failure.is_none() && exit.is_none_or(|code| code == 0) {
            return Vec::new();
        }
        let text = failure
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| stderr_tail.to_string());
        vec![UiChunk::DataAgentError {
            data: agent_error("opencode", exit, &text),
        }]
    }

    fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }
}

/// The UI tool kind and `{ path?, command?, query?, raw }` input for an OpenCode tool.
fn tool_input(tool: &str, input: &Value) -> (&'static str, Value) {
    let raw = truncate_input(input.clone());
    let with = |key: &str, value: &Value| {
        let mut v = json!({ "raw": raw });
        if !value.is_null() {
            v[key] = truncate_input(value.clone());
        }
        v
    };
    match tool {
        "bash" => ("shell", with("command", &input["command"])),
        "read" => ("read", with("path", &input["filePath"])),
        "edit" | "write" | "patch" | "multiedit" | "apply_patch" => {
            ("edit", with("path", &input["filePath"]))
        }
        "grep" | "glob" => ("search", with("query", &input["pattern"])),
        "list" => ("search", with("path", &input["path"])),
        "webfetch" => ("web", with("query", &input["url"])),
        "websearch" | "codesearch" => ("web", with("query", &input["query"])),
        _ => ("tool", json!({ "raw": { "tool": tool, "input": raw } })),
    }
}

fn tool(part: &Value, out: &mut Vec<UiChunk>) {
    let (Some(id), Some(tool)) = (part["callID"].as_str(), part["tool"].as_str()) else {
        return;
    };
    let state = &part["state"];
    let status = state["status"].as_str().unwrap_or_default();
    if !matches!(status, "completed" | "error") {
        return;
    }
    let (name, input) = tool_input(tool, &state["input"]);
    let tool_call_id = id.to_string();
    out.push(UiChunk::ToolInputAvailable {
        tool_call_id: tool_call_id.clone(),
        tool_name: name.to_string(),
        input,
        dynamic: true,
    });
    let error = |text: String| UiChunk::ToolOutputError {
        tool_call_id: tool_call_id.clone(),
        error_text: truncate_output(&text),
        dynamic: true,
    };
    let text = state["output"].as_str().unwrap_or_default();
    if status == "error" {
        out.push(error(
            state["error"].as_str().unwrap_or("failed").to_string(),
        ));
        return;
    }
    let metadata = &state["metadata"];
    let output = match name {
        "shell" => {
            let exit = &metadata["exit"];
            if let Some(code) = exit.as_i64().filter(|code| *code != 0) {
                let mut message = format!("exit code {code}");
                if !text.trim().is_empty() {
                    message.push('\n');
                    message.push_str(text.trim_end());
                }
                out.push(error(message));
                return;
            }
            json!({ "exitCode": exit, "stdout": truncate_output(text) })
        }
        "read" => json!({ "lines": metadata["display"]["totalLines"] }),
        "edit" => truncate_output(metadata["diff"].as_str().unwrap_or(text)).into(),
        _ => truncate_output(text).into(),
    };
    out.push(UiChunk::ToolOutputAvailable {
        tool_call_id,
        output,
        dynamic: true,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::chunk::AgentErrorCode;
    use crate::chat::translate::MAX_OUTPUT;
    use serde_json::{json, Value};

    const TURN: &str = include_str!("../../../tests/fixtures/chat/opencode/turn.jsonl");
    const RESUME: &str = include_str!("../../../tests/fixtures/chat/opencode/resume.jsonl");
    const TOOL_ERROR: &str = include_str!("../../../tests/fixtures/chat/opencode/tool-error.jsonl");
    const ERROR: &str = include_str!("../../../tests/fixtures/chat/opencode/error.jsonl");

    fn run(t: &mut OpencodeTranslator, lines: &str) -> Vec<UiChunk> {
        lines.lines().flat_map(|line| t.feed(line)).collect()
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
            .unwrap_or_else(|| panic!("no {name} tool in {chunks:#?}"))
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

    fn texts(chunks: &[UiChunk]) -> Vec<&str> {
        chunks
            .iter()
            .filter_map(|c| match c {
                UiChunk::TextDelta { delta, .. } => Some(delta.as_str()),
                _ => None,
            })
            .collect()
    }

    fn agent_error(chunks: &[UiChunk]) -> &crate::chat::chunk::AgentError {
        let [UiChunk::DataAgentError { data }] = chunks else {
            panic!("{chunks:#?}");
        };
        data
    }

    #[test]
    fn turn_text_is_a_whole_text_part() {
        let mut t = OpencodeTranslator::default();
        let chunks = run(&mut t, TURN);
        let reply = "Appended the line to about.md, and `ls` shows just `about.md`.";
        let at = chunks
            .iter()
            .position(|c| *c == UiChunk::TextStart { id: "t0".into() })
            .expect("text start");
        assert_eq!(
            chunks[at + 1..at + 3],
            [
                UiChunk::TextDelta {
                    id: "t0".into(),
                    delta: reply.into()
                },
                UiChunk::TextEnd { id: "t0".into() },
            ]
        );
        assert_eq!(t.session_id(), Some("ses_eb0ec7890ffePGSndItdTRpY09"));
        assert_eq!(t.summary(), Some(reply));
        assert_eq!(t.finish(Some(0), ""), []);
    }

    #[test]
    fn turn_reasoning_parts_count_from_zero() {
        let chunks = run(&mut OpencodeTranslator::default(), TURN);
        let ids: Vec<&str> = chunks
            .iter()
            .filter_map(|c| match c {
                UiChunk::ReasoningStart { id } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, ["r0", "r1", "r2", "r3"]);
    }

    #[test]
    fn turn_read_reports_line_count_not_content() {
        let chunks = run(&mut OpencodeTranslator::default(), TURN);
        let (at, id, input) = tool_input(&chunks, "read");
        assert_eq!(input["path"], "/work/project/about.md");
        assert_eq!(input["raw"]["filePath"], "/work/project/about.md");
        assert_eq!(tool_output(&chunks, at, id), &json!({ "lines": 1 }));
    }

    #[test]
    fn turn_edit_outputs_the_diff() {
        let chunks = run(&mut OpencodeTranslator::default(), TURN);
        let (at, id, input) = tool_input(&chunks, "edit");
        assert_eq!(id, "call_46c5928974934a01b1a1b025");
        assert_eq!(input["path"], "/work/project/about.md");
        let diff = tool_output(&chunks, at, id).as_str().unwrap();
        assert!(diff.contains("+Hello from the agent."), "{diff}");
    }

    #[test]
    fn turn_bash_is_a_shell_tool() {
        let chunks = run(&mut OpencodeTranslator::default(), TURN);
        let (at, id, input) = tool_input(&chunks, "shell");
        assert_eq!(input["command"], "ls");
        let output = tool_output(&chunks, at, id);
        assert_eq!(output["exitCode"], 0);
        assert_eq!(output["stdout"], "about.md\n");
    }

    #[test]
    fn final_step_reports_usage_summed_over_steps() {
        let chunks = run(&mut OpencodeTranslator::default(), TURN);
        let metadata: Vec<&Value> = chunks
            .iter()
            .filter_map(|c| match c {
                UiChunk::MessageMetadata { message_metadata } => Some(message_metadata),
                _ => None,
            })
            .collect();
        let [metadata] = metadata.as_slice() else {
            panic!("{metadata:#?}");
        };
        assert_eq!(metadata["provider"], "opencode");
        let steps: Vec<Value> = TURN
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .filter(|e| e["type"] == "step_finish")
            .collect();
        let output: i64 = steps
            .iter()
            .map(|e| e["part"]["tokens"]["output"].as_i64().unwrap())
            .sum();
        assert_eq!(metadata["usage"]["output"], output);
    }

    #[test]
    fn resume_is_a_single_text_part() {
        let mut t = OpencodeTranslator::default();
        let chunks = run(&mut t, RESUME);
        assert_eq!(
            texts(&chunks),
            ["I added the line \"Hello from the agent.\" to about.md."]
        );
        assert!(!chunks
            .iter()
            .any(|c| matches!(c, UiChunk::ToolInputAvailable { .. })));
    }

    #[test]
    fn nonzero_shell_exit_is_a_tool_error() {
        let chunks = run(&mut OpencodeTranslator::default(), TOOL_ERROR);
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
    fn errored_tool_state_is_a_tool_error() {
        let line = json!({"type": "tool_use", "sessionID": "s", "part": {"type": "tool", "tool": "read", "callID": "c1",
            "state": {"status": "error", "input": {"filePath": "/x"}, "error": "File not found: /x"}}})
        .to_string();
        let chunks = OpencodeTranslator::default().feed(&line);
        assert!(chunks.iter().any(|c| matches!(
            c,
            UiChunk::ToolOutputError { tool_call_id, error_text, .. }
                if tool_call_id == "c1" && error_text == "File not found: /x"
        )));
    }

    #[test]
    fn other_tools_keep_their_name() {
        let line = json!({"type": "tool_use", "sessionID": "s", "part": {"type": "tool", "tool": "todowrite", "callID": "c2",
            "state": {"status": "completed", "input": {"todos": []}, "output": "ok"}}})
        .to_string();
        let chunks = OpencodeTranslator::default().feed(&line);
        let (at, id, input) = tool_input(&chunks, "tool");
        assert_eq!(input["raw"]["tool"], "todowrite");
        assert_eq!(tool_output(&chunks, at, id), "ok");
    }

    #[test]
    fn search_and_web_tools_are_mapped() {
        for (tool, name, field, value) in [
            ("grep", "search", "query", "TODO"),
            ("glob", "search", "query", "*.md"),
            ("webfetch", "web", "query", "https://example.com"),
        ] {
            let input = match tool {
                "webfetch" => json!({ "url": value }),
                _ => json!({ "pattern": value }),
            };
            let line = json!({"type": "tool_use", "sessionID": "s", "part": {"type": "tool", "tool": tool, "callID": "c",
                "state": {"status": "completed", "input": input, "output": "x"}}})
            .to_string();
            let chunks = OpencodeTranslator::default().feed(&line);
            let (_, _, got) = tool_input(&chunks, name);
            assert_eq!(got[field], value, "{tool}");
        }
    }

    #[test]
    fn long_tool_output_is_truncated() {
        let long = "x".repeat(20_000);
        let line = json!({"type": "tool_use", "sessionID": "s", "part": {"type": "tool", "tool": "bash", "callID": "c",
            "state": {"status": "completed", "input": {"command": long}, "output": long, "metadata": {"output": long, "exit": 0}}}})
        .to_string();
        let chunks = OpencodeTranslator::default().feed(&line);
        let (at, id, input) = tool_input(&chunks, "shell");
        assert!(input["command"].as_str().unwrap().len() <= MAX_OUTPUT + "…".len());
        let stdout = tool_output(&chunks, at, id)["stdout"].as_str().unwrap();
        assert!(stdout.len() <= MAX_OUTPUT + "…".len());
    }

    #[test]
    fn error_event_is_an_agent_error() {
        let mut t = OpencodeTranslator::default();
        assert_eq!(run(&mut t, ERROR), []);
        let data = agent_error(&t.finish(Some(1), "")).clone();
        assert_eq!(data.code, AgentErrorCode::Failed);
        assert!(
            data.message.contains("Unexpected server error"),
            "{}",
            data.message
        );
    }

    #[test]
    fn auth_error_is_login() {
        let mut t = OpencodeTranslator::default();
        t.feed(r#"{"type":"error","sessionID":"s","error":{"name":"ProviderAuthError","data":{"providerID":"anthropic","message":"Please log in again"}}}"#);
        assert_eq!(
            agent_error(&t.finish(Some(1), "")).code,
            AgentErrorCode::Login
        );
    }

    #[test]
    fn nonzero_exit_without_events_uses_stderr() {
        let mut t = OpencodeTranslator::default();
        assert!(agent_error(&t.finish(Some(1), "Error: boom"))
            .message
            .contains("Error: boom"));
    }

    #[test]
    fn noise_is_ignored() {
        let mut t = OpencodeTranslator::default();
        assert_eq!(t.feed("not json"), []);
        assert_eq!(t.feed(""), []);
    }
}
