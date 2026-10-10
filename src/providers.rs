//! Provider registry: which coding CLIs the daemon can launch in a PTY, and how to
//! inject each one's turn-end hook.
//!
//! Read from `~/.config/workbench/agents.json` (or `--agents-config`):
//!
//! ```json
//! { "claude": ["npx", "claude"], "opencode": ["opencode"] }
//! ```
//!
//! Each key is a provider id, each value the base argv. A missing file means the built-in
//! defaults; a present-but-invalid file is an error. Overriding the base argv keeps the
//! hook injection (keyed on the id). Unknown ids get no injection; they call
//! `$WORKBENCH_HOOK_EXE hook turn-end` themselves.
//!
//! # Hook mechanics (verified 2026-10-10 against claude 2.1.296, codex-cli 0.162.1,
//! opencode v2.0.25)
//!
//! - Claude: `--settings <json>` adds a `Stop` command hook; `--dangerously-skip-permissions`
//!   auto-approves.
//! - Codex: `-c notify=[exe,"hook","turn-end"]` fires with NO `--dangerously-bypass-hook-trust`.
//!   Verified by one `codex exec -c 'notify=["n.sh","hook","turn-end"]' "say ok"`: the script ran
//!   and received `hook turn-end <json payload>` (codex appends the payload as the last arg).
//!   Hook trust applies to hooks.json hooks, not `notify`. Open question: hooks.json trust
//!   was not exercised, so the bypass flag is not appended.
//! - OpenCode: the installed binary reads `OPENCODE_CONFIG_DIR` (replaces the global config
//!   dir, so `plugins/*.ts` there load per tabsh's setup.md, 2.x plugin shape
//!   `{ id, setup(ctx) }`). Idle event: `session.execution.succeeded|failed|interrupted`
//!   (`ctx.event.subscribe`). All-permissions: `{"permission":"allow"}` in `opencode.json` in
//!   that dir (not exercised at runtime; the CLI also has an `--auto` flag). Concern: if the CLI
//!   attaches to an already-running background service, that server does not see this env var.

use std::path::Path;

use serde::Serialize;

use crate::error::ApiError;

/// `{ id, command }` as exposed by `GET /api/terminal/providers`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provider {
    pub id: String,
    /// argv: program followed by its arguments.
    pub command: Vec<String>,
}

/// What to spawn: full argv plus extra environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
}

pub fn default_providers() -> Vec<Provider> {
    ["claude", "codex", "opencode"]
        .iter()
        .map(|id| Provider {
            id: (*id).into(),
            command: vec![(*id).into()],
        })
        .collect()
}

/// Load from `path`. `None` or a missing file gives the defaults.
pub fn load(path: Option<&Path>) -> Result<Vec<Provider>, ApiError> {
    let Some(path) = path else {
        return Ok(default_providers());
    };
    match std::fs::read_to_string(path) {
        Ok(text) => {
            parse(&text).map_err(|e| ApiError::agent(format!("invalid {}: {e}", path.display())))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(default_providers()),
        Err(e) => Err(ApiError::agent(format!(
            "cannot read {}: {e}",
            path.display()
        ))),
    }
}

/// Parse the JSON registry (see module docs).
pub fn parse(text: &str) -> Result<Vec<Provider>, String> {
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
    Ok(providers)
}

/// POSIX single-quote `s` so a shell reads it back as one word.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn opencode_plugin(exe: &str) -> String {
    let exe_js = serde_json::to_string(exe).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"// Written by workbench on every launch. Reports turn end to the daemon.
import {{ execFile }} from "node:child_process"

const exe = {exe_js}

export default {{
  id: "workbench.turn-end",
  setup: async (ctx: any) => {{
    const stop = new AbortController()
    void (async () => {{
      for await (const e of ctx.event.subscribe(undefined, {{ signal: stop.signal }})) {{
        const id = e?.data?.sessionID
        if (typeof id !== "string") continue
        if (e.type.startsWith("session.execution.") && e.type !== "session.execution.started") {{
          const child = execFile(exe, ["hook", "turn-end"], () => {{}})
          child.stdin?.end(JSON.stringify({{ session_id: id }}))
        }}
      }}
    }})().catch(() => {{}})
    return () => stop.abort()
  }},
}}
"#
    )
}

/// Resolve the argv/env for launching `provider`, injecting its turn-end hook.
/// `hook_exe` is the daemon executable (`<exe> hook turn-end`). For OpenCode this writes
/// the plugin (and permission config) under `{state_dir}/opencode` on every call.
pub fn launch_spec(
    provider: &Provider,
    hook_exe: &Path,
    auto_approve: bool,
    state_dir: &Path,
) -> LaunchSpec {
    let exe = hook_exe.to_string_lossy();
    let mut argv = provider.command.clone();
    let mut env = Vec::new();
    match provider.id.as_str() {
        "claude" => {
            let settings = serde_json::json!({"hooks": {"Stop": [{"hooks": [{
                "type": "command",
                "command": format!("{} hook turn-end", sh_quote(&exe)),
            }]}]}});
            argv.push("--settings".into());
            argv.push(settings.to_string());
            if auto_approve {
                argv.push("--dangerously-skip-permissions".into());
            }
        }
        "codex" => {
            let exe_json = serde_json::to_string(&*exe).unwrap_or_default();
            argv.push("-c".into());
            argv.push(format!(r#"notify=[{exe_json},"hook","turn-end"]"#));
            if auto_approve {
                argv.push("--dangerously-bypass-approvals-and-sandbox".into());
            }
        }
        "opencode" => {
            let dir = state_dir.join("opencode");
            match write_opencode_config(&dir, &exe, auto_approve) {
                Ok(()) => env.push((
                    "OPENCODE_CONFIG_DIR".to_string(),
                    dir.to_string_lossy().into_owned(),
                )),
                Err(e) => eprintln!(
                    "workbench: opencode turn-end hook unavailable; checkpoints disabled ({e})"
                ),
            }
        }
        _ => {}
    }
    LaunchSpec { argv, env }
}

fn write_opencode_config(dir: &Path, exe: &str, auto_approve: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("plugins"))?;
    std::fs::write(dir.join("plugins/workbench.ts"), opencode_plugin(exe))?;
    let cfg = dir.join("opencode.json");
    if auto_approve {
        std::fs::write(cfg, r#"{"permission":"allow"}"#)
    } else {
        match std::fs::remove_file(cfg) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_the_three_agents() {
        let c = default_providers();
        let ids: Vec<_> = c.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["claude", "codex", "opencode"]);
        assert_eq!(c[2].command, ["opencode"]);
    }

    #[test]
    fn parses_custom_registry() {
        let c = parse(r#"{"mine": ["/bin/agent", "--custom"]}"#).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].command, ["/bin/agent", "--custom"]);
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
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn missing_file_falls_back_to_defaults_but_bad_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.json");
        assert_eq!(load(Some(&missing)).unwrap(), default_providers());
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "{").unwrap();
        assert!(load(Some(&bad)).is_err());
        assert_eq!(load(None).unwrap(), default_providers());
    }

    fn p(id: &str) -> Provider {
        Provider {
            id: id.into(),
            command: vec![id.into()],
        }
    }
    fn spec(id: &str, exe: &str, auto: bool, dir: &Path) -> LaunchSpec {
        launch_spec(&p(id), Path::new(exe), auto, dir)
    }

    #[test]
    fn claude_gets_stop_hook_settings() {
        let s = spec("claude", "/x/it's", false, Path::new("/nope"));
        assert_eq!(s.argv.len(), 3);
        assert_eq!(
            (s.argv[0].as_str(), s.argv[1].as_str()),
            ("claude", "--settings")
        );
        let v: serde_json::Value = serde_json::from_str(&s.argv[2]).unwrap();
        let cmd = v["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["type"], "command");
        assert!(cmd.starts_with(r"'/x/it'\''s'"), "{cmd}");
        assert!(cmd.ends_with(" hook turn-end"), "{cmd}");
        assert!(s.env.is_empty());
    }

    #[test]
    fn claude_auto_approve_appends_skip_permissions() {
        let s = spec("claude", "/x", true, Path::new("/nope"));
        assert_eq!(s.argv.last().unwrap(), "--dangerously-skip-permissions");
    }

    #[test]
    fn codex_gets_notify_override() {
        let s = spec("codex", "/x/work bench", false, Path::new("/nope"));
        assert_eq!(s.argv.len(), 3);
        assert_eq!(s.argv[1], "-c");
        assert_eq!(s.argv[2], r#"notify=["/x/work bench","hook","turn-end"]"#);
    }

    #[test]
    fn codex_auto_approve_appends_bypass() {
        let s = spec("codex", "/x", true, Path::new("/nope"));
        assert_eq!(
            s.argv.last().unwrap(),
            "--dangerously-bypass-approvals-and-sandbox"
        );
        assert!(!s
            .argv
            .iter()
            .any(|a| a == "--dangerously-bypass-hook-trust"));
    }

    #[test]
    fn opencode_writes_plugin_and_sets_config_env() {
        let dir = tempfile::tempdir().unwrap();
        let s = spec("opencode", "/x/wb", true, dir.path());
        assert_eq!(s.argv, ["opencode"]);
        let cfg = dir.path().join("opencode");
        assert_eq!(
            s.env,
            [(
                "OPENCODE_CONFIG_DIR".to_string(),
                cfg.to_string_lossy().into_owned()
            )]
        );
        let plugin = std::fs::read_to_string(cfg.join("plugins/workbench.ts")).unwrap();
        assert!(plugin.contains(r#""/x/wb""#) && plugin.contains("turn-end"));
        assert_eq!(
            std::fs::read_to_string(cfg.join("opencode.json")).unwrap(),
            r#"{"permission":"allow"}"#
        );
        spec("opencode", "/x/wb", false, dir.path());
        assert!(!cfg.join("opencode.json").exists());
    }

    #[test]
    fn unknown_provider_gets_base_argv_only() {
        let s = launch_spec(
            &Provider {
                id: "mine".into(),
                command: vec!["/bin/a".into(), "-v".into()],
            },
            Path::new("/x"),
            true,
            Path::new("/nope"),
        );
        assert_eq!(s.argv, ["/bin/a", "-v"]);
        assert!(s.env.is_empty());
    }

    #[test]
    fn override_replaces_base_argv_but_keeps_hook() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("agents.json");
        std::fs::write(&f, r#"{"claude":["npx","claude"]}"#).unwrap();
        let provs = load(Some(&f)).unwrap();
        let s = launch_spec(&provs[0], Path::new("/x"), false, dir.path());
        assert_eq!(&s.argv[..3], ["npx", "claude", "--settings"]);
    }
}
