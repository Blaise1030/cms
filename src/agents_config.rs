//! Provider registry: which ACP adapters the daemon can launch.
//!
//! Read from `~/.config/workbench/agents.json` (or `--agents-config`):
//!
//! ```json
//! { "claude": ["npx", "-y", "@agentclientprotocol/claude-agent-acp"],
//!   "opencode": ["opencode", "acp"] }
//! ```
//!
//! Each key is a provider id, each value the argv that starts an ACP-speaking agent on
//! stdio. Adding an agent is config-only. A missing file means the built-in defaults; a
//! present-but-invalid file is an error (never silently ignored). The file is re-read on
//! every use, so edits apply without a daemon restart.

use std::path::Path;

use serde::Serialize;

use crate::error::ApiError;

/// `{ id, command }` as exposed by `GET /api/agents`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provider {
    pub id: String,
    /// argv: program followed by its arguments.
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsConfig {
    providers: Vec<Provider>,
}

/// Built-in defaults. Adapter packages verified against npm (2026-09): Claude Code and
/// Codex ship as `@agentclientprotocol/*` adapters; OpenCode speaks ACP natively.
pub fn default_providers() -> Vec<Provider> {
    let p = |id: &str, cmd: &[&str]| Provider {
        id: id.into(),
        command: cmd.iter().map(|s| s.to_string()).collect(),
    };
    vec![
        p(
            "claude",
            &["npx", "-y", "@agentclientprotocol/claude-agent-acp"],
        ),
        p("codex", &["npx", "-y", "@agentclientprotocol/codex-acp"]),
        p("opencode", &["opencode", "acp"]),
    ]
}

impl AgentsConfig {
    pub fn defaults() -> Self {
        Self {
            providers: default_providers(),
        }
    }

    /// Load from `path`. `None` or a missing file gives the defaults.
    pub fn load(path: Option<&Path>) -> Result<Self, ApiError> {
        let Some(path) = path else {
            return Ok(Self::defaults());
        };
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text)
                .map_err(|e| ApiError::agent(format!("invalid {}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::defaults()),
            Err(e) => Err(ApiError::agent(format!(
                "cannot read {}: {e}",
                path.display()
            ))),
        }
    }

    /// Parse the JSON registry (see module docs).
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let obj = value
            .as_object()
            .ok_or("expected a JSON object mapping provider id to command array")?;
        let mut providers = Vec::new();
        for (id, cmd) in obj {
            if id.is_empty() || id.chars().any(char::is_control) {
                return Err(format!("invalid provider id {id:?}"));
            }
            let argv = cmd
                .as_array()
                .ok_or_else(|| format!("provider {id:?}: command must be an array of strings"))?;
            let command: Vec<String> = argv
                .iter()
                .map(|a| a.as_str().map(str::to_string))
                .collect::<Option<_>>()
                .ok_or_else(|| format!("provider {id:?}: command must be an array of strings"))?;
            if command.first().is_none_or(|p| p.is_empty()) {
                return Err(format!("provider {id:?}: command must not be empty"));
            }
            providers.push(Provider {
                id: id.clone(),
                command,
            });
        }
        Ok(Self { providers })
    }

    pub fn providers(&self) -> &[Provider] {
        &self.providers
    }

    pub fn get(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_the_three_agents() {
        let c = AgentsConfig::defaults();
        let ids: Vec<_> = c.providers().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["claude", "codex", "opencode"]);
        assert_eq!(c.get("opencode").unwrap().command, ["opencode", "acp"]);
    }

    #[test]
    fn parses_custom_registry() {
        let c = AgentsConfig::parse(r#"{"mine": ["/bin/agent", "--acp"]}"#).unwrap();
        assert_eq!(c.providers().len(), 1);
        assert_eq!(c.get("mine").unwrap().command, ["/bin/agent", "--acp"]);
        assert!(c.get("claude").is_none());
    }

    #[test]
    fn rejects_bad_shapes() {
        for bad in [
            "[]",
            "not json",
            r#"{"a": "npx x"}"#,
            r#"{"a": []}"#,
            r#"{"a": [""]}"#,
            r#"{"a": ["x", 1]}"#,
            r#"{"": ["x"]}"#,
        ] {
            assert!(AgentsConfig::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn missing_file_falls_back_to_defaults_but_bad_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.json");
        assert_eq!(
            AgentsConfig::load(Some(&missing)).unwrap(),
            AgentsConfig::defaults()
        );
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "{").unwrap();
        assert!(AgentsConfig::load(Some(&bad)).is_err());
        assert_eq!(AgentsConfig::load(None).unwrap(), AgentsConfig::defaults());
    }
}
