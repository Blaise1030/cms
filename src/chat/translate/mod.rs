//! Per-CLI translators from JSON lines to [`UiChunk`]s.
//!
//! Translators never emit `Start`/`StartStep`/`FinishStep`/`Finish`; the manager does. Text part
//! ids are `t{n}` and reasoning ids `r{n}`, counting from 0 per turn. Whole messages are emitted
//! as start + one delta + end.

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod opencode;

use serde_json::Value;

use crate::chat::chunk::{AgentError, AgentErrorCode, UiChunk};
use crate::providers::display_name;

/// Turns one CLI's stdout lines into UI chunks for a single turn.
pub trait Translator: Send {
    /// One stdout line; unparsable or irrelevant lines give `[]`.
    fn feed(&mut self, line: &str) -> Vec<UiChunk>;
    /// The process exited; closes open parts and reports failures.
    fn finish(&mut self, exit: Option<i32>, stderr_tail: &str) -> Vec<UiChunk>;
    /// The CLI's own session id, once seen.
    fn session_id(&self) -> Option<&str>;
    /// The last assistant text, used as the checkpoint summary.
    fn summary(&self) -> Option<&str>;
}

/// A fresh translator for `provider_id`, or `None` if it has no headless chat support.
pub fn for_provider(provider_id: &str) -> Option<Box<dyn Translator>> {
    match provider_id {
        "cursor" => Some(Box::new(cursor::CursorTranslator::default())),
        "claude" => Some(Box::new(claude::ClaudeTranslator::default())),
        "codex" => Some(Box::new(codex::CodexTranslator::default())),
        "opencode" => Some(Box::new(opencode::OpencodeTranslator::default())),
        _ => None,
    }
}

/// Whether `provider_id` can run in the chat panel.
pub fn is_chat_provider(provider_id: &str) -> bool {
    matches!(provider_id, "cursor" | "claude" | "codex" | "opencode")
}

/// Classify a failed run: a missing binary, an expired login, or anything else.
pub fn agent_error(provider_id: &str, exit: Option<i32>, text: &str) -> AgentError {
    let name = display_name(provider_id);
    let lower = text.to_lowercase();
    // Most shells exit 127 for a missing program; bash 3.2 (macOS /bin/bash) `exec` exits 126
    // with "No such file or directory", where 126 otherwise means "not executable".
    let missing =
        exit == Some(127) || (exit == Some(126) && lower.contains("no such file or directory"));
    let (code, message) = if missing {
        (
            AgentErrorCode::NotInstalled,
            format!("{name} is not installed or not on your PATH."),
        )
    } else if ["authenticate", "oauth", "log in", "login"]
        .iter()
        .any(|w| lower.contains(w))
    {
        (
            AgentErrorCode::Login,
            format!(
                "Your {name} login has expired. Log in again from your terminal, then try again."
            ),
        )
    } else {
        let text = match (text.trim(), exit) {
            ("", Some(code)) => format!("exit code {code}"),
            ("", None) => "no output".to_string(),
            (text, _) => text.to_string(),
        };
        (
            AgentErrorCode::Failed,
            format!("{name} stopped with an error: {text}"),
        )
    };
    AgentError { code, message }
}

/// Tool output cap, in bytes.
pub const MAX_OUTPUT: usize = 16 * 1024;

/// `s` cut to [`MAX_OUTPUT`] bytes on a char boundary, with `…` appended when cut.
pub fn truncate_output(s: &str) -> String {
    if s.len() <= MAX_OUTPUT {
        return s.to_string();
    }
    format!("{}…", &s[..s.floor_char_boundary(MAX_OUTPUT)])
}

/// `v` with every string leaf passed through [`truncate_output`].
pub fn truncate_input(v: Value) -> Value {
    match v {
        Value::String(s) if s.len() > MAX_OUTPUT => truncate_output(&s).into(),
        Value::Array(items) => items.into_iter().map(truncate_input).collect(),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, truncate_input(value)))
                .collect(),
        ),
        v => v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::chunk::AgentErrorCode;

    #[test]
    fn oauth_failure_is_login() {
        let err = agent_error(
            "claude",
            Some(1),
            "Failed to authenticate: OAuth token expired",
        );
        assert_eq!(err.code, AgentErrorCode::Login);
        assert_eq!(
            err.message,
            "Your Claude Code login has expired. Log in again from your terminal, then try again."
        );
    }

    #[test]
    fn log_in_wording_is_login() {
        let err = agent_error("cursor", Some(1), "Please LOG IN first");
        assert_eq!(err.code, AgentErrorCode::Login);
    }

    #[test]
    fn exit_127_is_not_installed() {
        let err = agent_error("codex", Some(127), "");
        assert_eq!(err.code, AgentErrorCode::NotInstalled);
        assert_eq!(err.message, "Codex is not installed or not on your PATH.");
    }

    #[test]
    fn bash_3_exec_of_a_missing_program_is_not_installed() {
        let stderr = "/bin/bash: /nonexistent/cursor-agent: No such file or directory\n\
                      /bin/bash: line 0: exec: /nonexistent/cursor-agent: cannot execute: No such file or directory";
        let err = agent_error("cursor", Some(126), stderr);
        assert_eq!(err.code, AgentErrorCode::NotInstalled);
        assert_eq!(
            err.message,
            "Cursor CLI is not installed or not on your PATH."
        );
    }

    #[test]
    fn exit_126_for_another_reason_is_a_failure() {
        let err = agent_error(
            "cursor",
            Some(126),
            "/usr/local/bin/cursor-agent: Permission denied",
        );
        assert_eq!(err.code, AgentErrorCode::Failed);
    }

    #[test]
    fn other_failures_carry_the_text() {
        let err = agent_error("cursor", Some(1), "boom");
        assert_eq!(err.code, AgentErrorCode::Failed);
        assert_eq!(err.message, "Cursor CLI stopped with an error: boom");
    }

    #[test]
    fn empty_failure_text_names_the_exit_code() {
        let err = agent_error("cursor", Some(2), "  ");
        assert_eq!(err.message, "Cursor CLI stopped with an error: exit code 2");
    }

    #[test]
    fn truncate_output_cuts_on_a_char_boundary() {
        let long = "é".repeat(20_000);
        let cut = truncate_output(&long);
        assert!(cut.len() <= MAX_OUTPUT + "…".len());
        assert!(cut.ends_with('…'));
        assert!(cut.trim_end_matches('…').chars().all(|c| c == 'é'));
    }

    #[test]
    fn truncate_output_keeps_short_text() {
        assert_eq!(truncate_output("ok"), "ok");
    }

    #[test]
    fn truncate_input_cuts_long_string_leaves() {
        let long = "x".repeat(20_000);
        let input = serde_json::json!({
            "path": "a.md",
            "raw": { "lines": 2, "ok": true, "content": long, "list": ["short", long] },
        });
        let out = truncate_input(input);
        let cut = out["raw"]["content"].as_str().unwrap();
        assert!(cut.len() <= MAX_OUTPUT + "…".len());
        assert!(cut.ends_with('…'));
        assert_eq!(out["raw"]["list"][1], cut);
        assert_eq!(out["path"], "a.md");
        assert_eq!(out["raw"]["lines"], 2);
        assert_eq!(out["raw"]["ok"], true);
        assert_eq!(out["raw"]["list"][0], "short");
    }

    #[test]
    fn chat_providers() {
        for id in ["cursor", "claude", "codex", "opencode"] {
            assert!(is_chat_provider(id), "{id}");
            assert!(for_provider(id).is_some(), "{id}");
        }
        assert!(!is_chat_provider("fake"));
        assert!(for_provider("fake").is_none());
    }
}
