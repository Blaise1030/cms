//! Which models each chat CLI offers, as `{ id, name }` where `id` is what its model flag takes.
//!
//! Cursor and OpenCode are asked (`<cli> models`) once per daemon run; Codex's own
//! `models_cache.json` is read each time; Claude has no list command, so it gets its aliases.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::providers::Provider;
use crate::terminal::login_command;

const LIST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Model {
    pub id: String,
    pub name: String,
}

/// Non-empty lists from `<cli> models`, by provider id.
static ASKED: LazyLock<Mutex<HashMap<String, Vec<Model>>>> = LazyLock::new(Mutex::default);

/// The models `provider` offers; empty when unknown or the CLI could not list them.
pub async fn list(provider: &Provider) -> Vec<Model> {
    match provider.id.as_str() {
        "claude" => claude_models(),
        "codex" => std::fs::read_to_string(codex_home().join("models_cache.json"))
            .map(|text| parse_codex_cache(&text))
            .unwrap_or_default(),
        "cursor" | "opencode" => {
            if let Some(models) = ASKED.lock().unwrap().get(&provider.id) {
                return models.clone();
            }
            let stdout = ask(provider).await.unwrap_or_default();
            let models = if provider.id == "cursor" {
                parse_cursor(&stdout)
            } else {
                parse_opencode(&stdout)
            };
            if !models.is_empty() {
                ASKED
                    .lock()
                    .unwrap()
                    .insert(provider.id.clone(), models.clone());
            }
            models
        }
        _ => Vec::new(),
    }
}

/// Stdout of `<command> models` run through the login shell, if it succeeded in time.
async fn ask(provider: &Provider) -> Option<String> {
    let mut argv = provider.command.clone();
    argv.push("models".into());
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut cmd = login_command(&argv, home.as_deref().unwrap_or(Path::new("/")));
    cmd.stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let out = tokio::time::timeout(LIST_TIMEOUT, cmd.output())
        .await
        .ok()?
        .ok()
        .filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn codex_home() -> PathBuf {
    match std::env::var_os("CODEX_HOME") {
        Some(dir) => dir.into(),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".codex"),
    }
}

/// `s` without ANSI escape sequences (`ESC [ … letter`).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next() == Some('[') {
                chars.by_ref().find(|c| c.is_ascii_alphabetic());
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `cursor-agent models`: `id - Name` lines, the default one marked `(current, default)`.
pub fn parse_cursor(stdout: &str) -> Vec<Model> {
    strip_ansi(stdout)
        .lines()
        .filter_map(|line| {
            let (id, name) = line.trim().split_once(" - ")?;
            if id.is_empty() || id.contains(char::is_whitespace) {
                return None;
            }
            let name = match name.rsplit_once(" (") {
                Some((base, mark)) if mark.contains("current") || mark.contains("default") => base,
                _ => name,
            };
            Some(Model {
                id: id.into(),
                name: name.trim().into(),
            })
        })
        .collect()
}

/// `opencode models`: one `provider/model` per line.
pub fn parse_opencode(stdout: &str) -> Vec<Model> {
    strip_ansi(stdout)
        .lines()
        .map(str::trim)
        .filter(|line| line.contains('/') && !line.contains(char::is_whitespace))
        .map(|id| Model {
            id: id.into(),
            name: id.into(),
        })
        .collect()
}

/// Codex's `models_cache.json`: the models it shows in its own picker (`visibility: "list"`).
pub fn parse_codex_cache(json: &str) -> Vec<Model> {
    let Ok(cache) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    cache["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["visibility"] == "list")
        .filter_map(|m| {
            let id = m["slug"].as_str()?;
            Some(Model {
                id: id.into(),
                name: m["display_name"].as_str().unwrap_or(id).into(),
            })
        })
        .collect()
}

/// `claude --model` aliases for the latest of each family.
pub fn claude_models() -> Vec<Model> {
    [("sonnet", "Sonnet"), ("opus", "Opus"), ("haiku", "Haiku")]
        .into_iter()
        .map(|(id, name)| Model {
            id: id.into(),
            name: name.into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, name: &str) -> Model {
        Model {
            id: id.into(),
            name: name.into(),
        }
    }

    #[test]
    fn cursor_lists_id_and_name_without_the_current_marker() {
        let out = "Available models\n\n\
                   auto - Auto (current, default)\n\
                   gpt-5.3-codex - Codex 5.3\n\
                   composer-2.5 - Composer 2.5\n\n\
                   Tip: use --model <id> (or /model <id> in interactive mode) to switch.\n";
        assert_eq!(
            parse_cursor(out),
            [
                m("auto", "Auto"),
                m("gpt-5.3-codex", "Codex 5.3"),
                m("composer-2.5", "Composer 2.5"),
            ]
        );
    }

    #[test]
    fn cursor_strips_ansi_colours() {
        assert_eq!(
            parse_cursor("\x1b[1mgpt-5.2\x1b[0m - GPT-5.2\n"),
            [m("gpt-5.2", "GPT-5.2")]
        );
    }

    #[test]
    fn opencode_lists_provider_slash_model_lines() {
        let out = "opencode/big-pickle\nanthropic/claude-sonnet-5\n\nsome warning text\n";
        assert_eq!(
            parse_opencode(out),
            [
                m("opencode/big-pickle", "opencode/big-pickle"),
                m("anthropic/claude-sonnet-5", "anthropic/claude-sonnet-5"),
            ]
        );
    }

    #[test]
    fn codex_cache_keeps_listed_models_only() {
        let cache = r#"{"fetched_at":"x","models":[
            {"slug":"gpt-6.1-sol","display_name":"GPT-6.1-Sol","visibility":"list"},
            {"slug":"gpt-reserve","display_name":"GPT-Reserve","visibility":"hide"},
            {"slug":"gpt-5.6-luna","display_name":"GPT-5.6-Luna","visibility":"list"}
        ]}"#;
        assert_eq!(
            parse_codex_cache(cache),
            [
                m("gpt-6.1-sol", "GPT-6.1-Sol"),
                m("gpt-5.6-luna", "GPT-5.6-Luna")
            ]
        );
        assert_eq!(parse_codex_cache("not json"), []);
    }

    #[test]
    fn claude_offers_its_aliases() {
        let ids: Vec<String> = claude_models().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, ["sonnet", "opus", "haiku"]);
    }
}
