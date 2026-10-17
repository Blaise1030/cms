//! Codex `exec --json`.

use std::collections::HashSet;

use serde_json::{json, Value};

use crate::chat::chunk::UiChunk;
use crate::chat::translate::{agent_error, truncate_input, truncate_output, Translator};

#[derive(Debug, Default)]
pub struct CodexTranslator {
    session_id: Option<String>,
    summary: Option<String>,
    texts: usize,
    reasonings: usize,
    announced_tools: HashSet<String>,
    /// Message of `turn.failed` or a top-level `error`.
    failure: Option<String>,
    /// Message of the last `error` item; non-fatal, so it only explains a failed exit.
    error_note: Option<String>,
}

impl CodexTranslator {
    fn item(&mut self, item: &Value, completed: bool, out: &mut Vec<UiChunk>) {
        match item["type"].as_str() {
            Some("agent_message") if completed => {
                let Some(text) = item["text"].as_str().filter(|t| !t.is_empty()) else {
                    return;
                };
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
            Some("reasoning") if completed => {
                let Some(text) = item["text"].as_str().filter(|t| !t.is_empty()) else {
                    return;
                };
                let id = format!("r{}", self.reasonings);
                self.reasonings += 1;
                out.push(UiChunk::ReasoningStart { id: id.clone() });
                out.push(UiChunk::ReasoningDelta {
                    id: id.clone(),
                    delta: text.to_string(),
                });
                out.push(UiChunk::ReasoningEnd { id });
            }
            Some("error") if completed => {
                self.error_note = item["message"].as_str().map(str::to_string);
            }
            _ => self.tool(item, completed, out),
        }
    }

    fn tool(&mut self, item: &Value, completed: bool, out: &mut Vec<UiChunk>) {
        let Some(id) = item["id"].as_str() else {
            return;
        };
        let Some((name, input)) = tool_input(item) else {
            return;
        };
        if self.announced_tools.insert(id.to_string()) {
            out.push(UiChunk::ToolInputAvailable {
                tool_call_id: id.to_string(),
                tool_name: name.to_string(),
                input: truncate_input(input),
                dynamic: true,
            });
        }
        if completed {
            out.push(tool_result(name, id, item));
        }
    }
}

impl Translator for CodexTranslator {
    fn feed(&mut self, line: &str) -> Vec<UiChunk> {
        let Ok(event) = serde_json::from_str::<Value>(line.trim()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        match event["type"].as_str() {
            Some("thread.started") => {
                self.session_id = event["thread_id"].as_str().map(str::to_string);
            }
            Some("item.started" | "item.updated") => self.item(&event["item"], false, &mut out),
            Some("item.completed") => self.item(&event["item"], true, &mut out),
            Some("turn.completed") => out.push(UiChunk::MessageMetadata {
                message_metadata: json!({
                    "provider": "codex",
                    "usage": event["usage"],
                }),
            }),
            Some("turn.failed") => {
                self.failure = Some(message(&event["error"]));
            }
            Some("error") => self.failure = Some(message(&event)),
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
                .chain(self.error_note.take())
                .find(|text| !text.trim().is_empty())
                .unwrap_or_else(|| stderr_tail.to_string());
            out.push(UiChunk::DataAgentError {
                data: agent_error("codex", exit, &text),
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

fn message(value: &Value) -> String {
    value["message"].as_str().unwrap_or_default().to_string()
}

/// The tool kind and `{ path?, command?, query?, raw }` input for a tool item.
fn tool_input(item: &Value) -> Option<(&'static str, Value)> {
    match item["type"].as_str()? {
        "command_execution" => {
            let command = &item["command"];
            Some((
                "shell",
                json!({ "command": command, "raw": { "command": command } }),
            ))
        }
        "file_change" => {
            let changes = &item["changes"];
            let mut input = json!({ "raw": changes });
            if let Some(path) = changes[0]["path"].as_str() {
                input["path"] = path.into();
            }
            Some(("edit", input))
        }
        "mcp_tool_call" => Some((
            "mcp",
            json!({
                "raw": {
                    "server": item["server"],
                    "tool": item["tool"],
                    "arguments": item["arguments"],
                },
            }),
        )),
        "web_search" => Some(("web", json!({ "query": item["query"], "raw": item }))),
        _ => None,
    }
}

fn tool_result(name: &str, id: &str, item: &Value) -> UiChunk {
    let tool_call_id = id.to_string();
    let status = item["status"].as_str().unwrap_or_default();
    let failed = matches!(status, "failed" | "declined");
    let error = |error_text: String| UiChunk::ToolOutputError {
        tool_call_id: tool_call_id.clone(),
        error_text: truncate_output(&error_text),
        dynamic: true,
    };
    let output = match name {
        "shell" => {
            let exit = &item["exit_code"];
            let stdout = item["aggregated_output"].as_str().unwrap_or_default();
            if failed || exit.as_i64().is_some_and(|code| code != 0) {
                let mut text = match exit.as_i64() {
                    Some(code) => format!("exit code {code}"),
                    None => format!("command {status}"),
                };
                if !stdout.trim().is_empty() {
                    text.push('\n');
                    text.push_str(stdout.trim_end());
                }
                return error(text);
            }
            json!({ "exitCode": exit, "stdout": truncate_output(stdout) })
        }
        "mcp" => {
            if let Some(text) = item["error"]["message"].as_str() {
                return error(text.to_string());
            }
            if failed {
                return error(format!("{name} {status}"));
            }
            truncate_input(item["result"].clone())
        }
        _ if failed => return error(format!("{name} {status}")),
        "edit" => truncate_input(item["changes"].clone()),
        _ => Value::Null,
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
    use crate::chat::chunk::AgentErrorCode;
    use crate::chat::translate::MAX_OUTPUT;
    use serde_json::{json, Value};

    const TURN: &str = include_str!("../../../tests/fixtures/chat/codex/turn.jsonl");
    const RESUME: &str = include_str!("../../../tests/fixtures/chat/codex/resume.jsonl");
    const TOOL_ERROR: &str = include_str!("../../../tests/fixtures/chat/codex/tool-error.jsonl");

    const TURN_TEXTS: [&str; 3] = [
        "I’ll append the line to `about.md`, then run `ls`.",
        "I’m using the using-superpowers skill to check the applicable workflow.",
        "Appended “Hello from the agent.” to `about.md` and ran `ls`.",
    ];

    fn run(t: &mut CodexTranslator, lines: &str) -> Vec<UiChunk> {
        lines.lines().flat_map(|line| t.feed(line)).collect()
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

    fn agent_error(chunks: &[UiChunk]) -> &crate::chat::chunk::AgentError {
        let [UiChunk::DataAgentError { data }] = chunks else {
            panic!("{chunks:#?}");
        };
        data
    }

    #[test]
    fn turn_messages_are_text_parts() {
        let mut t = CodexTranslator::default();
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
        let expected: Vec<UiChunk> = TURN_TEXTS
            .iter()
            .enumerate()
            .flat_map(|(n, text)| {
                let id = format!("t{n}");
                [
                    UiChunk::TextStart { id: id.clone() },
                    UiChunk::TextDelta {
                        id: id.clone(),
                        delta: text.to_string(),
                    },
                    UiChunk::TextEnd { id },
                ]
            })
            .collect();
        assert_eq!(text, expected.iter().collect::<Vec<_>>());
        assert_eq!(t.session_id(), Some("01a149cd-c3e5-7db2-a75c-86b4ed8e7928"));
        assert_eq!(t.summary(), Some(TURN_TEXTS[2]));
        assert_eq!(t.finish(Some(0), ""), []);
    }

    #[test]
    fn turn_commands_are_shell_tools() {
        let chunks = run(&mut CodexTranslator::default(), TURN);
        let shells = tool_inputs(&chunks, "shell");
        let ids: Vec<&str> = shells.iter().map(|(_, id, _)| *id).collect();
        assert_eq!(ids, ["item_1", "item_3"]);
        let (at, id, input) = shells
            .iter()
            .find(|(_, _, input)| input["command"].as_str().unwrap().contains("\nls\n"))
            .expect("a command running ls");
        assert_eq!(input["raw"]["command"], input["command"]);
        let output = tool_output(&chunks, *at, id);
        assert_eq!(output["exitCode"], 0);
        assert_eq!(output["stdout"], "about.md\n");
    }

    #[test]
    fn turn_completed_is_metadata() {
        let chunks = run(&mut CodexTranslator::default(), TURN);
        let metadata = chunks
            .iter()
            .find_map(|c| match c {
                UiChunk::MessageMetadata { message_metadata } => Some(message_metadata),
                _ => None,
            })
            .expect("metadata");
        assert_eq!(metadata["provider"], "codex");
        assert_eq!(metadata["usage"]["output_tokens"], 212);
    }

    #[test]
    fn resume_is_a_single_text_part() {
        let mut t = CodexTranslator::default();
        let chunks = run(&mut t, RESUME);
        assert_eq!(chunks[0], UiChunk::TextStart { id: "t0".into() });
        assert!(!chunks
            .iter()
            .any(|c| matches!(c, UiChunk::ToolInputAvailable { .. })));
        assert_eq!(
            t.summary(),
            Some("I added the line “Hello from the agent.”")
        );
    }

    #[test]
    fn failed_command_is_a_tool_error() {
        let chunks = run(&mut CodexTranslator::default(), TOOL_ERROR);
        let shells = tool_inputs(&chunks, "shell");
        let (_, id, input) = shells[0];
        assert_eq!(input["command"], "/bin/zsh -lc 'cat missing.txt'");
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
        let (at, ok_id, _) = shells[1];
        assert_eq!(tool_output(&chunks, at, ok_id)["exitCode"], 0);
    }

    #[test]
    fn turn_failed_is_an_agent_error() {
        let mut t = CodexTranslator::default();
        t.feed(r#"{"type":"turn.failed","error":{"message":"You've hit your usage limit"}}"#);
        let chunks = t.finish(Some(1), "");
        let data = agent_error(&chunks);
        assert_eq!(data.code, AgentErrorCode::Failed);
        assert!(data.message.contains("You've hit your usage limit"));
    }

    #[test]
    fn login_failure_is_login() {
        let mut t = CodexTranslator::default();
        t.feed(
            r#"{"type":"turn.failed","error":{"message":"Not logged in. Please run codex login"}}"#,
        );
        assert_eq!(
            agent_error(&t.finish(Some(1), "")).code,
            AgentErrorCode::Login
        );
    }

    #[test]
    fn top_level_error_fails_the_turn() {
        let mut t = CodexTranslator::default();
        t.feed(r#"{"type":"error","message":"stream disconnected"}"#);
        let chunks = t.finish(Some(0), "");
        assert!(agent_error(&chunks).message.contains("stream disconnected"));
    }

    #[test]
    fn error_item_explains_a_failed_exit() {
        let line = r#"{"type":"item.completed","item":{"id":"item_0","type":"error","message":"model overloaded"}}"#;
        let mut ok = CodexTranslator::default();
        assert_eq!(ok.feed(line), []);
        assert_eq!(ok.finish(Some(0), ""), []);
        let mut failed = CodexTranslator::default();
        failed.feed(line);
        let chunks = failed.finish(Some(1), "stderr text");
        assert!(agent_error(&chunks).message.contains("model overloaded"));
    }

    #[test]
    fn nonzero_exit_without_events_uses_stderr() {
        let mut t = CodexTranslator::default();
        let chunks = t.finish(Some(1), "Error: boom");
        assert!(agent_error(&chunks).message.contains("Error: boom"));
    }

    #[test]
    fn file_change_is_an_edit() {
        let changes = json!([
            {"path": "/work/project/about.md", "kind": "update"},
            {"path": "/work/project/new.md", "kind": "add"},
        ]);
        let item = json!({"id": "item_9", "type": "file_change", "changes": changes, "status": "completed"});
        let started = json!({"type": "item.started", "item": item}).to_string();
        let completed = json!({"type": "item.completed", "item": item}).to_string();

        for lines in [vec![completed.clone()], vec![started, completed]] {
            let mut t = CodexTranslator::default();
            let chunks: Vec<UiChunk> = lines.iter().flat_map(|l| t.feed(l)).collect();
            let edits = tool_inputs(&chunks, "edit");
            let [(at, id, input)] = edits.as_slice() else {
                panic!("{chunks:#?}");
            };
            assert_eq!(*id, "item_9");
            assert_eq!(input["path"], "/work/project/about.md");
            assert_eq!(input["raw"], changes);
            tool_output(&chunks, *at, id);
        }
    }

    #[test]
    fn reasoning_item_is_a_reasoning_part() {
        let mut t = CodexTranslator::default();
        let chunks = t.feed(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","text":"Thinking it over"}}"#,
        );
        assert_eq!(
            chunks,
            [
                UiChunk::ReasoningStart { id: "r0".into() },
                UiChunk::ReasoningDelta {
                    id: "r0".into(),
                    delta: "Thinking it over".into()
                },
                UiChunk::ReasoningEnd { id: "r0".into() },
            ]
        );
    }

    #[test]
    fn mcp_and_web_search_items_are_tools() {
        let mut t = CodexTranslator::default();
        let chunks = run(
            &mut t,
            concat!(
                r#"{"type":"item.started","item":{"id":"item_0","type":"mcp_tool_call","server":"docs","tool":"search","arguments":{"q":"x"},"result":null,"error":null,"status":"in_progress"}}"#,
                "\n",
                r#"{"type":"item.completed","item":{"id":"item_0","type":"mcp_tool_call","server":"docs","tool":"search","arguments":{"q":"x"},"result":{"content":[{"type":"text","text":"hit"}],"structured_content":null},"error":null,"status":"completed"}}"#,
                "\n",
                r#"{"type":"item.started","item":{"id":"item_1","type":"mcp_tool_call","server":"docs","tool":"fetch","arguments":{},"result":null,"error":null,"status":"in_progress"}}"#,
                "\n",
                r#"{"type":"item.completed","item":{"id":"item_1","type":"mcp_tool_call","server":"docs","tool":"fetch","arguments":{},"result":null,"error":{"message":"tool timed out"},"status":"failed"}}"#,
                "\n",
                r#"{"type":"item.completed","item":{"id":"item_2","type":"web_search","query":"rust floor_char_boundary"}}"#,
                "\n",
                r#"{"type":"item.completed","item":{"id":"item_3","type":"todo_list","items":[{"text":"a","completed":false}]}}"#,
            ),
        );
        let mcp = tool_inputs(&chunks, "mcp");
        assert_eq!(mcp.len(), 2);
        let (at, id, input) = mcp[0];
        assert_eq!(input["raw"]["server"], "docs");
        assert_eq!(input["raw"]["tool"], "search");
        assert_eq!(input["raw"]["arguments"], json!({"q": "x"}));
        assert_eq!(
            tool_output(&chunks, at, id)["content"][0]["text"],
            json!("hit")
        );
        assert!(chunks.iter().any(|c| matches!(
            c,
            UiChunk::ToolOutputError { tool_call_id, error_text, .. }
                if tool_call_id == "item_1" && error_text == "tool timed out"
        )));
        let web = tool_inputs(&chunks, "web");
        let [(at, id, input)] = web.as_slice() else {
            panic!("{chunks:#?}");
        };
        assert_eq!(input["query"], "rust floor_char_boundary");
        tool_output(&chunks, *at, id);
        assert_eq!(chunks.len(), 3 * 2, "todo_list ignored: {chunks:#?}");
    }

    #[test]
    fn long_command_and_output_are_truncated() {
        let long = "x".repeat(20_000);
        let item = json!({"id": "item_0", "type": "command_execution", "command": long, "aggregated_output": long, "exit_code": 0, "status": "completed"});
        let mut t = CodexTranslator::default();
        let chunks = t.feed(&json!({"type": "item.completed", "item": item}).to_string());
        let shells = tool_inputs(&chunks, "shell");
        let (at, id, input) = shells[0];
        for value in [&input["command"], &input["raw"]["command"]] {
            let s = value.as_str().unwrap();
            assert!(s.len() <= MAX_OUTPUT + "…".len() && s.ends_with('…'));
        }
        let stdout = tool_output(&chunks, at, id)["stdout"].as_str().unwrap();
        assert!(stdout.len() <= MAX_OUTPUT + "…".len());
    }

    #[test]
    fn noise_is_ignored() {
        let mut t = CodexTranslator::default();
        assert_eq!(t.feed("Reading additional input from stdin..."), []);
        assert_eq!(t.feed(""), []);
        assert_eq!(t.feed(r#"{"type":"turn.started"}"#), []);
    }
}
