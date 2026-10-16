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
//! defaults (Cursor CLI via `cursor-agent`, Claude, and Codex); a present-but-invalid file
//! is an error. Overriding the base argv keeps the
//! hook injection (keyed on the id). Unknown ids get no injection; they call
//! `$WORKBENCH_HOOK_EXE hook turn-end` themselves.
//!
//! # Hook mechanics (verified 2026-10-10 against claude 2.1.296, codex-cli 0.162.1,
//! opencode v2.0.25)
//!
//! - Cursor CLI: `--force` and `--trust` when auto-approve is on. No turn-end hook. No
//!   system-prompt flag: the git guide goes in a git-excluded `.cursor/rules/*.mdc` rule
//!   (rule loading not exercised at runtime here).
//! - Claude: `--settings <json>` adds a `Stop` command hook; `--dangerously-skip-permissions`
//!   auto-approves.
//! - Codex: `-c notify=[exe,"hook","turn-end"]` fires with NO `--dangerously-bypass-hook-trust`.
//!   Verified by one `codex exec -c 'notify=["n.sh","hook","turn-end"]' "say ok"`: the script ran
//!   and received `hook turn-end <json payload>` (codex appends the payload as the last arg).
//!   Hook trust applies to hooks.json hooks, not `notify`. Open question: hooks.json trust
//!   was not exercised, so the bypass flag is not appended. The git guide goes in
//!   `-c developer_instructions=...` (verified 2026-10-13: `codex exec` obeyed it).
//! - OpenCode: the installed binary reads `OPENCODE_CONFIG_DIR` (replaces the global config
//!   dir; this daemon never reads or writes `~/.config/opencode`). Each project gets its own
//!   directory `{state_dir}/{project_id}/opencode`, so one project's `auto_approve` cannot
//!   rewrite another's. `plugins/*.ts` there load per tabsh's setup.md, 2.x plugin shape
//!   `{ id, setup(ctx) }`. Idle event: `session.execution.succeeded|failed|interrupted`
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

/// Which agent conversation a launch belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentSession {
    /// A new conversation whose id the CLI picks (reported later by the hook, if at all).
    Fresh,
    /// A new conversation with this id: Claude `--session-id`, Cursor `--resume` of a chat
    /// made by `create-chat`.
    Assign(String),
    /// Continue this conversation.
    Resume(String),
}

/// Whether the provider's conversations can be resumed by id.
pub fn can_resume(provider_id: &str) -> bool {
    matches!(provider_id, "cursor" | "claude" | "codex" | "opencode")
}

/// What to spawn: full argv plus extra environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
}

pub fn default_providers() -> Vec<Provider> {
    [
        ("cursor", "cursor-agent"),
        ("claude", "claude"),
        ("codex", "codex"),
    ]
    .into_iter()
    .map(|(id, program)| Provider {
        id: id.into(),
        command: vec![program.into()],
    })
    .collect()
}

/// What people call the provider; unknown ids are shown as is.
pub fn display_name(provider_id: &str) -> &str {
    match provider_id {
        "cursor" => "Cursor CLI",
        "claude" => "Claude Code",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        other => other,
    }
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

/// Given to every agent in [`gets_git_guide`] so it manages changes (branches) and syncing
/// itself, in the shape the app lists them (`content/*` + branch description as the title).
pub const GIT_GUIDE: &str = r#"# Changes and syncing (this project is edited through a no-code app)

The person you are helping is usually not technical. Do the git work for them and describe it in plain words ("I started a new draft called Pricing page", "I brought in the latest version of the site") — never ask them to run git.

A "change" (a draft) is a git branch named `content/<kebab-case-slug>`; its human title is stored with `git config branch.<branch>.description "<Title>"`. The app lists these as the user's changes.

- Never edit or commit on `main`. Before editing while on `main`, start a change.
- Start a change: commit any pending work on the current change first (`git add -A && git commit -m "<summary>"`), then branch from the latest main — `git fetch origin main && git checkout -b content/<slug> origin/main` when an `origin` remote exists, otherwise `git checkout -b content/<slug> main` — and set its description. Pick a short title from what the user asked for; if the slug exists, add `-2`, `-3`, ….
- Do a new, unrelated request in a new change; keep follow-ups on the current one.
- Get the latest / sync with main: commit pending work, then `git fetch origin main && git merge origin/main` (just `git merge main` without a remote). If it conflicts, resolve it keeping the intent of both sides, run nothing destructive, and ask the user in plain words when the right result is unclear.
- Never rebase, force-push, `reset --hard`, delete branches, or push to `main`. Do not push at all unless asked — the app shares changes for review.
- The app also commits your work automatically at the end of each turn; that is expected."#;

/// Agents that receive [`GIT_GUIDE`] and so may start on `main`: they branch before editing.
/// Claude via `--append-system-prompt`, Codex via `developer_instructions`, Cursor via the
/// project rule [`write_cursor_rule`] writes.
pub fn gets_git_guide(provider_id: &str) -> bool {
    matches!(provider_id, "claude" | "codex" | "cursor")
}

/// Where [`write_cursor_rule`] puts [`GIT_GUIDE`], relative to the project.
pub const CURSOR_RULE_PATH: &str = ".cursor/rules/workbench-changes.mdc";

/// Cursor CLI has no system-prompt flag but loads `.cursor/rules/*.mdc`: write the guide as
/// an always-applied rule, and list it in `.git/info/exclude` so it never lands in a commit.
pub fn write_cursor_rule(project_dir: &Path) -> std::io::Result<()> {
    let rule = project_dir.join(CURSOR_RULE_PATH);
    if let Some(parent) = rule.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &rule,
        format!(
            "---\ndescription: How to start changes and sync in this app\n\
             alwaysApply: true\n---\n{GIT_GUIDE}\n"
        ),
    )?;
    let git = project_dir.join(".git");
    if !git.is_dir() {
        return Ok(());
    }
    let info = git.join("info");
    std::fs::create_dir_all(&info)?;
    let exclude = info.join("exclude");
    let line = format!("/{CURSOR_RULE_PATH}");
    let current = match std::fs::read_to_string(&exclude) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    if !current.lines().any(|l| l.trim() == line) {
        let sep = if current.is_empty() || current.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        std::fs::write(&exclude, format!("{current}{sep}{line}\n"))?;
    }
    Ok(())
}

/// Resolve the argv/env for launching `provider`, injecting its turn-end hook.
/// `hook_exe` is the daemon executable (`<exe> hook turn-end`). For OpenCode this writes
/// the plugin (and permission config) under `{state_dir}/{project_id}/opencode` on every call.
/// `session` is ignored by providers that cannot take it (see [`can_resume`]).
pub fn launch_spec(
    provider: &Provider,
    hook_exe: &Path,
    auto_approve: bool,
    state_dir: &Path,
    project_id: &str,
    session: &AgentSession,
) -> LaunchSpec {
    let exe = hook_exe.to_string_lossy();
    let mut argv = provider.command.clone();
    let mut env = Vec::new();
    match provider.id.as_str() {
        "cursor" => {
            if let AgentSession::Assign(id) | AgentSession::Resume(id) = session {
                argv.push("--resume".into());
                argv.push(id.clone());
            }
            if auto_approve {
                argv.push("--force".into());
                argv.push("--trust".into());
            }
        }
        "claude" => {
            let settings = serde_json::json!({"hooks": {"Stop": [{"hooks": [{
                "type": "command",
                "command": format!("{} hook turn-end", sh_quote(&exe)),
            }]}]}});
            argv.push("--settings".into());
            argv.push(settings.to_string());
            argv.push("--append-system-prompt".into());
            argv.push(GIT_GUIDE.into());
            match session {
                AgentSession::Fresh => {}
                AgentSession::Assign(id) => argv.extend(["--session-id".into(), id.clone()]),
                AgentSession::Resume(id) => argv.extend(["--resume".into(), id.clone()]),
            }
            if auto_approve {
                argv.push("--dangerously-skip-permissions".into());
            }
        }
        "codex" => {
            // `codex resume [OPTIONS] <id>`: the subcommand right after the base argv.
            if let AgentSession::Resume(_) = session {
                argv.push("resume".into());
            }
            let exe_json = serde_json::to_string(&*exe).unwrap_or_default();
            argv.push("-c".into());
            argv.push(format!(r#"notify=[{exe_json},"hook","turn-end"]"#));
            // A JSON string is a valid TOML basic string, which is what `-c` parses.
            let guide = serde_json::to_string(GIT_GUIDE).unwrap_or_default();
            argv.push("-c".into());
            argv.push(format!("developer_instructions={guide}"));
            if auto_approve {
                argv.push("--dangerously-bypass-approvals-and-sandbox".into());
            }
            if let AgentSession::Resume(id) = session {
                argv.push(id.clone());
            }
        }
        "opencode" => {
            if let AgentSession::Resume(id) = session {
                argv.push("--session".into());
                argv.push(id.clone());
            }
            let dir = state_dir.join(project_id).join("opencode");
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
        assert_eq!(ids, ["cursor", "claude", "codex"]);
        assert_eq!(c[0].command, ["cursor-agent"]);
        assert_eq!(c[1].command, ["claude"]);
        assert_eq!(c[2].command, ["codex"]);
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
        launch_spec(&p(id), Path::new(exe), auto, dir, "p", &AgentSession::Fresh)
    }

    fn session_argv(id: &str, session: AgentSession) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        launch_spec(&p(id), Path::new("/x"), true, dir.path(), "p", &session).argv
    }

    #[test]
    fn claude_assigns_or_resumes_its_session() {
        let new = session_argv("claude", AgentSession::Assign("u1".into()));
        assert_eq!(
            new[5..],
            ["--session-id", "u1", "--dangerously-skip-permissions"]
        );
        let resumed = session_argv("claude", AgentSession::Resume("u1".into()));
        assert_eq!(
            resumed[5..],
            ["--resume", "u1", "--dangerously-skip-permissions"]
        );
        assert_eq!(session_argv("claude", AgentSession::Fresh).len(), 6);
    }

    #[test]
    fn cursor_resumes_assigned_and_past_chats() {
        for s in [
            AgentSession::Assign("c1".into()),
            AgentSession::Resume("c1".into()),
        ] {
            assert_eq!(
                session_argv("cursor", s),
                ["cursor", "--resume", "c1", "--force", "--trust"]
            );
        }
    }

    #[test]
    fn codex_resume_is_a_subcommand_with_the_id_last() {
        let argv = session_argv("codex", AgentSession::Resume("t1".into()));
        assert_eq!(argv[..3], ["codex", "resume", "-c"]);
        assert_eq!(
            argv[6..],
            ["--dangerously-bypass-approvals-and-sandbox", "t1"]
        );
        assert_eq!(
            session_argv("codex", AgentSession::Assign("t1".into()))[1],
            "-c"
        );
    }

    #[test]
    fn opencode_resumes_with_session_flag() {
        let argv = session_argv("opencode", AgentSession::Resume("o1".into()));
        assert_eq!(argv, ["opencode", "--session", "o1"]);
    }

    #[test]
    fn cursor_auto_approve_appends_force_and_trust() {
        let s = spec("cursor", "/x", true, Path::new("/nope"));
        assert_eq!(s.argv, ["cursor", "--force", "--trust"]);
        assert!(s.env.is_empty());
        let plain = spec("cursor", "/x", false, Path::new("/nope"));
        assert_eq!(plain.argv, ["cursor"]);
    }

    #[test]
    fn claude_gets_stop_hook_settings() {
        let s = spec("claude", "/x/it's", false, Path::new("/nope"));
        assert_eq!(s.argv.len(), 5);
        assert_eq!(s.argv[3..], ["--append-system-prompt", GIT_GUIDE]);
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
        assert_eq!(s.argv.len(), 5);
        assert_eq!(s.argv[1], "-c");
        assert_eq!(s.argv[2], r#"notify=["/x/work bench","hook","turn-end"]"#);
    }

    #[test]
    fn codex_gets_git_guide_as_developer_instructions() {
        let s = spec("codex", "/x", false, Path::new("/nope"));
        assert_eq!(s.argv[3], "-c");
        let value = s.argv[4].strip_prefix("developer_instructions=").unwrap();
        assert_eq!(serde_json::from_str::<String>(value).unwrap(), GIT_GUIDE);
    }

    #[test]
    fn guided_agents_are_the_ones_given_the_guide() {
        for id in ["claude", "codex", "cursor"] {
            assert!(gets_git_guide(id), "{id}");
        }
        assert!(!gets_git_guide("opencode"));
        assert!(!gets_git_guide("fake"));
    }

    #[test]
    fn cursor_rule_is_written_and_git_excluded_once() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git/info")).unwrap();
        std::fs::write(dir.path().join(".git/info/exclude"), "# keep\n*.log").unwrap();
        write_cursor_rule(dir.path()).unwrap();
        write_cursor_rule(dir.path()).unwrap();

        let rule = std::fs::read_to_string(dir.path().join(CURSOR_RULE_PATH)).unwrap();
        assert!(rule.starts_with("---\n"), "{rule}");
        assert!(rule.contains("alwaysApply: true"), "{rule}");
        assert!(rule.contains(GIT_GUIDE));
        let exclude = std::fs::read_to_string(dir.path().join(".git/info/exclude")).unwrap();
        assert_eq!(exclude, format!("# keep\n*.log\n/{CURSOR_RULE_PATH}\n"));
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
        let fresh = &AgentSession::Fresh;
        let s = launch_spec(
            &p("opencode"),
            Path::new("/x/wb"),
            true,
            dir.path(),
            "alpha",
            fresh,
        );
        assert_eq!(s.argv, ["opencode"]);
        let cfg = dir.path().join("alpha").join("opencode");
        assert_ne!(cfg, dir.path().join("opencode"));
        assert_eq!(
            s.env,
            [(
                "OPENCODE_CONFIG_DIR".to_string(),
                cfg.to_string_lossy().into_owned()
            )]
        );
        assert!(!s.env.iter().any(|(_, v)| v.contains(".config/opencode")));
        let plugin = std::fs::read_to_string(cfg.join("plugins/workbench.ts")).unwrap();
        assert!(plugin.contains(r#""/x/wb""#) && plugin.contains("turn-end"));
        assert_eq!(
            std::fs::read_to_string(cfg.join("opencode.json")).unwrap(),
            r#"{"permission":"allow"}"#
        );
        // A second project without auto-approve must not delete the first project's file.
        let other = launch_spec(
            &p("opencode"),
            Path::new("/x/wb"),
            false,
            dir.path(),
            "beta",
            fresh,
        );
        let beta = dir.path().join("beta").join("opencode");
        assert_eq!(other.env[0].1, beta.to_string_lossy().into_owned());
        assert!(cfg.join("opencode.json").exists());
        assert!(!beta.join("opencode.json").exists());
        launch_spec(
            &p("opencode"),
            Path::new("/x/wb"),
            false,
            dir.path(),
            "alpha",
            fresh,
        );
        assert!(!cfg.join("opencode.json").exists());
        assert!(!dir.path().join("opencode").join("opencode.json").exists());
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
            "p",
            &AgentSession::Resume("x".into()),
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
        let s = launch_spec(
            &provs[0],
            Path::new("/x"),
            false,
            dir.path(),
            "p",
            &AgentSession::Fresh,
        );
        assert_eq!(&s.argv[..3], ["npx", "claude", "--settings"]);
    }
}
