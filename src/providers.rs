//! Provider registry: which coding CLIs the daemon can launch in a PTY, and how to
//! inject each one's turn-end hook.
//!
//! Read from `~/.config/orso/agents.json` (or `--agents-config`):
//!
//! ```json
//! { "claude": ["npx", "claude"], "opencode": ["opencode"] }
//! ```
//!
//! Each key is a provider id, each value the base argv. A missing file means the built-in
//! defaults (Cursor CLI via `cursor-agent`, Claude, Codex, and OpenCode); a present-but-invalid file
//! is an error. Overriding the base argv keeps the
//! hook injection (keyed on the id). Unknown ids get no injection; they call
//! `$ORSO_HOOK_EXE hook turn-end` themselves.
//!
//! # Hook mechanics (verified 2026-10-10 against claude 2.1.296, codex-cli 0.162.1,
//! opencode v2.0.25)
//!
//! - Cursor CLI: `--force` and `--trust` when auto-approve is on. No turn-end hook. No
//!   system-prompt flag: the agent guide goes in a git-excluded `.cursor/rules/*.mdc` rule
//!   (rule loading not exercised at runtime here).
//! - Claude: `--settings <json>` adds a `Stop` command hook; `--dangerously-skip-permissions`
//!   auto-approves.
//! - Codex: `-c notify=[exe,"hook","turn-end"]` fires with NO `--dangerously-bypass-hook-trust`.
//!   Verified by one `codex exec -c 'notify=["n.sh","hook","turn-end"]' "say ok"`: the script ran
//!   and received `hook turn-end <json payload>` (codex appends the payload as the last arg).
//!   Hook trust applies to hooks.json hooks, not `notify`. Open question: hooks.json trust
//!   was not exercised, so the bypass flag is not appended. The agent guide goes in
//!   `-c developer_instructions=...` (verified 2026-10-13: `codex exec` obeyed it).
//! - OpenCode: the installed binary reads `OPENCODE_CONFIG_DIR` (replaces the global config
//!   dir; this daemon never reads or writes `~/.config/opencode`). Each project gets its own
//!   directory `{state_dir}/{project_id}/opencode`, so one project's `auto_approve` cannot
//!   rewrite another's. `plugins/*.ts` there load per tabsh's setup.md, 2.x plugin shape
//!   `{ id, setup(ctx) }`. Idle event: `session.execution.succeeded|failed|interrupted`
//!   (`ctx.event.subscribe`). All-permissions: `{"permission":"allow"}` in `opencode.json` in
//!   that dir (not exercised at runtime; the CLI also has an `--auto` flag). Concern: if the CLI
//!   attaches to an already-running background service, that server does not see this env var.
//!
//! # Headless mechanics ([`headless_spec`], checked 2026-10-15 in a scratch git repo with a
//! "write `{"ok":true}` to `$ORSO_RESOLVE_RESULT`" prompt)
//!
//! - Cursor CLI 2026.10.14-0f9edde: `cursor-agent -p --force --output-format stream-json
//!   <prompt>` exited 0 and wrote the file (no `--trust` needed).
//! - Codex codex-cli 0.162.1: `codex exec --json --dangerously-bypass-approvals-and-sandbox
//!   <prompt>` exited 0 and wrote the file. The env var reached its shell tool.
//! - Claude 2.1.296: `claude -p <prompt> --output-format stream-json --verbose
//!   --dangerously-skip-permissions` not verified: OAuth session expired (exit 1, flags parsed).
//! - OpenCode 1.18.31 (checked 2026-10-18 with `-m opencode/big-pickle`): `opencode run
//!   --format json --thinking <prompt>` with `OPENCODE_CONFIG_DIR` exited 0 and edited the
//!   file; `-s <session>` resumed the conversation, and an `instructions` file listed in that
//!   dir's `opencode.json` was obeyed.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;

use crate::error::ApiError;

/// Temporary ACP argv by provider id. Task 6 moves this onto [`Provider`].
static ACP_ARGV_OVERRIDE: Mutex<Option<HashMap<String, Vec<String>>>> = Mutex::new(None);

/// Install the argv `POST /api/chat/…/start` and `/message` pass to
/// [`crate::chat::manager::ChatManager::start`]. Tests point `fake` at
/// [`crate::chat::acp::fake_agent`]. A real provider has no argv until Task 6.
pub fn set_test_acp_argv(provider_id: impl Into<String>, argv: Vec<String>) {
    let mut map = ACP_ARGV_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner());
    map.get_or_insert_with(HashMap::new)
        .insert(provider_id.into(), argv);
}

/// ACP command for `provider_id`: the test override, else `ORSO_ACP_ARGV`
/// (a JSON string array, any provider). `None` when neither is set.
pub fn acp_argv(provider_id: &str) -> Option<Vec<String>> {
    if let Some(argv) = ACP_ARGV_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|map| map.get(provider_id).cloned())
    {
        return Some(argv);
    }
    std::env::var("ORSO_ACP_ARGV")
        .ok()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .filter(|argv| !argv.is_empty())
}

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
        ("opencode", "opencode"),
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
        r#"// Written by orso on every launch. Reports turn end to the daemon.
import {{ execFile }} from "node:child_process"

const exe = {exe_js}

export default {{
  id: "orso.turn-end",
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

The person you are helping is usually not technical. Do the git work for them and describe it in plain words ("I brought in the latest version of the site") — never ask them to run git.

A "change" (a draft) is a git branch named `content/<kebab-case-slug>`; its human title is stored with `git config branch.<branch>.description "<Title>"`. The app lists these as the user's changes.

- Only the person switches changes, in the app's change picker. Never switch to a different change yourself — do not run `git checkout <branch>` or `git switch` to move to another change (the app blocks this). Stay on the current change.
- If the work needs a new change, do not create or switch to one yourself. Propose it in plain words ("I'd start a new draft called …") and ask the person to create it in the app, then continue.
- Do a new, unrelated request in a new change; keep follow-ups on the current one.
- Never edit or commit on `main`. If the current change is the live site (`main`), ask the person to create a change first.
- Save your work as you go: `git add -A && git commit -m "<summary>"` before anything risky, so nothing is lost.
- Get the latest / sync with main: commit pending work, then `git fetch origin main && git merge origin/main` (just `git merge main` without a remote). If it conflicts, resolve it keeping the intent of both sides, run nothing destructive, and ask the user in plain words when the right result is unclear.
- Never rebase, force-push, `reset --hard`, delete branches, or push to `main`. Do not push at all unless asked — the app shares changes for review.
- The app also commits your work automatically at the end of each turn; that is expected."#;

/// The orso layer of [`agent_guide`]: who the user is, the words to use with them,
/// what only they may do, and how to check work with `orso ctl`. Agent terminals have
/// the binary at `$ORSO_HOOK_EXE`, not on `PATH`.
pub const ORSO_GUIDE: &str = r#"# Working in orso

The person you are helping runs a website through orso, a no-code app. They are usually not technical.

## Words to use
For how their site gets updated, use these words: **change** (a set of edits made together), **greenlight** (a site owner's OK on a change), **ready to publish**, **publish**, and **live** (what visitors see). Never say branch, commit, pull request, or merge to them; describe what happened in plain words.

## What only they do
Publishing, sharing a change, sending it for a greenlight, giving a greenlight, and merging are clicks the person makes in the app. Never publish, share, or greenlight for them, and never do the git equivalent (opening, approving, or merging a pull request, or pushing to main). When a change is done, tell them it is ready to look at in the preview.

## Check your work
`orso ctl` shows app state you cannot see from files, and refreshes the app UI. Run it as `"$ORSO_HOOK_EXE" ctl <command>`; it prints JSON.
- `"$ORSO_HOOK_EXE" ctl preview-status` after editing: `devserver` is `starting`, `ready`, or `crashed`; when `crashed`, `message` holds the end of the dev server's output. Ordinary file edits often hot-reload; do not run a build.
- `"$ORSO_HOOK_EXE" ctl restart-preview` when the preview `crashed`: fix the cause shown in `message`, restart once, and check again. If it still crashes, tell the person in plain words what is wrong.
- `"$ORSO_HOOK_EXE" ctl refresh-content` after adding or renaming content (or when the Content sidebar looks stale): reloads **collections** and entry lists in the app.
- `"$ORSO_HOOK_EXE" ctl refresh-preview` after that (or when the preview looks stale): remounts the preview page so the person sees the newest result.
- `"$ORSO_HOOK_EXE" ctl change-status` before saying you are done: confirm which change you are on (`active`). `changedPaths` lists files edited since the last save; the app saves them when your turn ends, so there is nothing to do unless a path is unexpected.
When a Chat or Terminal turn ends, the app also refreshes Content and the preview. Cursor has no turn-end hook — still call `refresh-content` then `refresh-preview` before saying you are done so the person sees the result right away. If `$ORSO_HOOK_EXE` is not set, or the command says to run it inside a orso agent session, skip these checks.

## Edits after a greenlight
If you edit a change after it got a greenlight, the greenlight no longer covers it. Tell the person it needs a new greenlight before it can be published.

## When unsure
If what they want is unclear, ask one short question in plain words before editing."#;

/// Everything a guided agent is told: [`ORSO_GUIDE`] then [`GIT_GUIDE`].
pub fn agent_guide() -> String {
    format!("{ORSO_GUIDE}\n\n{GIT_GUIDE}")
}

/// Agents that receive [`agent_guide`] and so may start on `main`: they branch before editing.
/// Claude via `--append-system-prompt`, Codex via `developer_instructions`, Cursor via the
/// project rule [`write_cursor_rule`] writes, OpenCode via `instructions` in its config dir.
pub fn gets_git_guide(provider_id: &str) -> bool {
    matches!(provider_id, "claude" | "codex" | "cursor" | "opencode")
}

/// Where [`write_cursor_rule`] puts [`agent_guide`], relative to the project.
pub const CURSOR_RULE_PATH: &str = ".cursor/rules/orso-changes.mdc";

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
            "---\ndescription: How to work in this app: changes, checks, and syncing\n\
             alwaysApply: true\n---\n{}\n",
            agent_guide()
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
            argv.push(agent_guide());
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
            let guide = serde_json::to_string(&agent_guide()).unwrap_or_default();
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
                    "orso: opencode turn-end hook unavailable; checkpoints disabled ({e})"
                ),
            }
        }
        _ => {}
    }
    LaunchSpec { argv, env }
}

/// Resolve the argv/env for running `provider` once, non-interactively, on `prompt`.
/// Always auto-approves; no turn-end hook. `session` is honoured by Cursor, Claude, Codex and
/// OpenCode (Codex and OpenCode only resume; `Assign` is treated as `Fresh`). OpenCode
/// gets its own `{state_dir}/{project_id}/opencode-headless` config dir (permissions and guide,
/// no plugin), so it never rewrites the interactive terminal's config. `model` is the CLI's
/// own model id. Unknown providers ignore `model` and get the prompt appended last.
pub fn headless_spec(
    provider: &Provider,
    prompt: &str,
    state_dir: &Path,
    project_id: &str,
    session: &AgentSession,
    model: Option<&str>,
) -> LaunchSpec {
    // The CLIs would parse a leading `-` as an option (`--` isn't verified for all of them).
    let padded;
    let prompt = if prompt.starts_with('-') {
        padded = format!(" {prompt}");
        padded.as_str()
    } else {
        prompt
    };
    let mut argv = provider.command.clone();
    let mut env = Vec::new();
    match provider.id.as_str() {
        "cursor" => {
            if let AgentSession::Assign(id) | AgentSession::Resume(id) = session {
                argv.extend(["--resume".into(), id.clone()]);
            }
            argv.extend(["-p", "--force", "--output-format", "stream-json"].map(String::from));
            if let Some(model) = model {
                argv.extend(["--model".into(), model.into()]);
            }
            argv.push(prompt.into());
        }
        "claude" => {
            argv.extend(
                [
                    "-p",
                    prompt,
                    "--output-format",
                    "stream-json",
                    "--verbose",
                    "--dangerously-skip-permissions",
                    "--append-system-prompt",
                ]
                .map(String::from),
            );
            argv.push(agent_guide());
            match session {
                AgentSession::Fresh => {}
                AgentSession::Assign(id) => argv.extend(["--session-id".into(), id.clone()]),
                AgentSession::Resume(id) => argv.extend(["--resume".into(), id.clone()]),
            }
            if let Some(model) = model {
                argv.extend(["--model".into(), model.into()]);
            }
        }
        "codex" => {
            let guide = serde_json::to_string(&agent_guide()).unwrap_or_default();
            argv.extend(
                [
                    "exec",
                    "--json",
                    "--dangerously-bypass-approvals-and-sandbox",
                    "-c",
                ]
                .map(String::from),
            );
            argv.push(format!("developer_instructions={guide}"));
            if let Some(model) = model {
                argv.extend(["-m".into(), model.into()]);
            }
            // `codex exec [OPTIONS] resume <id> <prompt>`: options must precede `resume`.
            if let AgentSession::Resume(id) = session {
                argv.extend(["resume".into(), id.clone()]);
            }
            argv.push(prompt.into());
        }
        "opencode" => {
            argv.extend(["run", "--format", "json", "--thinking"].map(String::from));
            if let Some(model) = model {
                argv.extend(["-m".into(), model.into()]);
            }
            if let AgentSession::Resume(id) = session {
                argv.extend(["-s".into(), id.clone()]);
            }
            argv.push(prompt.into());
            let dir = state_dir.join(project_id).join("opencode-headless");
            match std::fs::create_dir_all(&dir).and_then(|()| write_opencode_json(&dir, true)) {
                Ok(()) => env.push((
                    "OPENCODE_CONFIG_DIR".to_string(),
                    dir.to_string_lossy().into_owned(),
                )),
                Err(e) => eprintln!("orso: opencode headless config unavailable ({e})"),
            }
        }
        _ => argv.push(prompt.into()),
    }
    LaunchSpec { argv, env }
}

fn write_opencode_config(dir: &Path, exe: &str, auto_approve: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("plugins"))?;
    std::fs::write(dir.join("plugins/orso.ts"), opencode_plugin(exe))?;
    write_opencode_json(dir, auto_approve)
}

/// `opencode.json` in `dir` loading [`agent_guide`] (written next to it) as instructions,
/// allowing every permission when `allow_all`.
fn write_opencode_json(dir: &Path, allow_all: bool) -> std::io::Result<()> {
    let guide = dir.join("orso-guide.md");
    std::fs::write(&guide, agent_guide())?;
    let mut config = serde_json::json!({ "instructions": [guide.to_string_lossy()] });
    if allow_all {
        config["permission"] = "allow".into();
    }
    std::fs::write(dir.join("opencode.json"), config.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn agent_guide_adds_the_orso_layer_to_the_git_guide() {
        let g = agent_guide();
        assert!(g.starts_with(ORSO_GUIDE));
        assert!(g.ends_with(GIT_GUIDE));
        assert!(g.contains("orso ctl"));
        for cmd in [
            "preview-status",
            "change-status",
            "restart-preview",
            "refresh-preview",
            "refresh-content",
        ] {
            assert!(
                g.contains(&format!("\"$ORSO_HOOK_EXE\" ctl {cmd}")),
                "{cmd}"
            );
        }
        assert!(g.contains("collections"));
        for word in [
            "change",
            "greenlight",
            "ready to publish",
            "publish",
            "live",
        ] {
            assert!(g.contains(&format!("**{word}**")), "{word}");
        }
        assert!(g.contains("Never publish, share, or greenlight for them"));
        assert!(g.contains("Never say branch, commit, pull request, or merge"));
        assert!(g.contains("the greenlight no longer covers it"));
        assert!(g.contains("crashed"));
        assert!(g.contains("the app saves them when your turn ends"));
        assert!(!g.contains("not saved yet"));
    }

    #[test]
    fn git_guide_no_longer_starts_or_switches_changes() {
        assert!(!agent_guide().contains("git checkout -b"));
        assert!(!GIT_GUIDE.contains("git checkout -b"));
        assert!(GIT_GUIDE.contains("Only the person switches changes"));
        assert!(GIT_GUIDE.contains("Propose it in plain words"));
    }

    use super::*;

    #[test]
    fn defaults_cover_the_four_agents() {
        let c = default_providers();
        let ids: Vec<_> = c.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["cursor", "claude", "codex", "opencode"]);
        assert_eq!(c[0].command, ["cursor-agent"]);
        assert_eq!(c[1].command, ["claude"]);
        assert_eq!(c[2].command, ["codex"]);
        assert_eq!(c[3].command, ["opencode"]);
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
        assert_eq!(
            s.argv[3..],
            ["--append-system-prompt".to_string(), agent_guide()]
        );
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
        assert_eq!(
            serde_json::from_str::<String>(value).unwrap(),
            agent_guide()
        );
    }

    #[test]
    fn guided_agents_are_the_ones_given_the_guide() {
        for id in ["claude", "codex", "cursor", "opencode"] {
            assert!(gets_git_guide(id), "{id}");
        }
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
        assert!(rule.contains(&agent_guide()));
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
        let plugin = std::fs::read_to_string(cfg.join("plugins/orso.ts")).unwrap();
        assert!(plugin.contains(r#""/x/wb""#) && plugin.contains("turn-end"));
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(cfg.join("opencode.json")).unwrap())
                .unwrap();
        assert_eq!(config["permission"], "allow");
        assert_eq!(
            std::fs::read_to_string(config["instructions"][0].as_str().unwrap()).unwrap(),
            agent_guide()
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
        let permission = |d: &Path| {
            let text = std::fs::read_to_string(d.join("opencode.json")).unwrap();
            serde_json::from_str::<serde_json::Value>(&text).unwrap()["permission"].clone()
        };
        assert_eq!(permission(&cfg), "allow");
        assert!(permission(&beta).is_null());
        launch_spec(
            &p("opencode"),
            Path::new("/x/wb"),
            false,
            dir.path(),
            "alpha",
            fresh,
        );
        assert!(permission(&cfg).is_null());
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

    const PROMPT: &str = "resolve the conflicts";

    fn headless(id: &str, dir: &Path) -> LaunchSpec {
        headless_spec(&p(id), PROMPT, dir, "p", &AgentSession::Fresh, None)
    }

    fn headless_session(id: &str, session: AgentSession) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        headless_spec(&p(id), PROMPT, dir.path(), "p", &session, None).argv
    }

    fn headless_model(id: &str, session: AgentSession) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        headless_spec(&p(id), PROMPT, dir.path(), "p", &session, Some("m1")).argv
    }

    /// `argv` with the `flag value` pair removed, asserting it was there once.
    fn without_pair(argv: &[String], flag: &str, value: &str) -> Vec<String> {
        let at = argv
            .windows(2)
            .position(|w| w[0] == flag && w[1] == value)
            .unwrap_or_else(|| panic!("no {flag} {value} in {argv:?}"));
        [&argv[..at], &argv[at + 2..]].concat()
    }

    #[test]
    fn headless_model_is_one_flag_pair() {
        for (id, flag) in [
            ("cursor", "--model"),
            ("claude", "--model"),
            ("codex", "-m"),
            ("opencode", "-m"),
        ] {
            let argv = headless_model(id, AgentSession::Fresh);
            assert_eq!(
                without_pair(&argv, flag, "m1"),
                headless_session(id, AgentSession::Fresh),
                "{id}"
            );
        }
    }

    #[test]
    fn headless_codex_model_precedes_resume() {
        let argv = headless_model("codex", AgentSession::Resume("t1".into()));
        let model = argv.iter().position(|a| a == "-m").unwrap();
        let resume = argv.iter().position(|a| a == "resume").unwrap();
        assert!(model < resume, "{argv:?}");
    }

    #[test]
    fn headless_opencode_resumes_with_session_flag() {
        let argv = headless_session("opencode", AgentSession::Resume("o1".into()));
        assert_eq!(
            argv,
            [
                "opencode",
                "run",
                "--format",
                "json",
                "--thinking",
                "-s",
                "o1",
                PROMPT
            ]
        );
        assert_eq!(
            headless_session("opencode", AgentSession::Assign("o1".into())),
            headless_session("opencode", AgentSession::Fresh)
        );
    }

    #[test]
    fn headless_dash_prompt_is_not_a_flag() {
        for id in ["cursor", "claude", "codex", "opencode"] {
            let dir = tempfile::tempdir().unwrap();
            let argv = headless_spec(
                &p(id),
                "- fix the title",
                dir.path(),
                "p",
                &AgentSession::Fresh,
                None,
            )
            .argv;
            assert!(
                argv.iter().any(|a| a == " - fix the title"),
                "{id}: {argv:?}"
            );
            assert!(
                !argv.iter().any(|a| a == "- fix the title"),
                "{id}: {argv:?}"
            );
        }
        let argv = headless_spec(
            &p("cursor"),
            "fix - the title",
            Path::new("/nope"),
            "p",
            &AgentSession::Fresh,
            None,
        )
        .argv;
        assert_eq!(argv.last().map(String::as_str), Some("fix - the title"));
    }

    #[test]
    fn headless_cursor_resume() {
        for session in [
            AgentSession::Resume("c1".into()),
            AgentSession::Assign("c1".into()),
        ] {
            assert_eq!(
                headless_session("cursor", session),
                [
                    "cursor",
                    "--resume",
                    "c1",
                    "-p",
                    "--force",
                    "--output-format",
                    "stream-json",
                    PROMPT
                ]
            );
        }
    }

    #[test]
    fn headless_claude_sessions() {
        let assigned = headless_session("claude", AgentSession::Assign("u1".into()));
        assert_eq!(assigned[assigned.len() - 2..], ["--session-id", "u1"]);
        let resumed = headless_session("claude", AgentSession::Resume("u1".into()));
        assert_eq!(resumed[resumed.len() - 2..], ["--resume", "u1"]);
        assert_eq!(
            headless_session("claude", AgentSession::Fresh),
            headless("claude", Path::new("/nope")).argv
        );
    }

    #[test]
    fn headless_codex_resume() {
        let guide = serde_json::to_string(&agent_guide()).unwrap();
        assert_eq!(
            headless_session("codex", AgentSession::Resume("t1".into())),
            [
                "codex",
                "exec",
                "--json",
                "--dangerously-bypass-approvals-and-sandbox",
                "-c",
                &format!("developer_instructions={guide}"),
                "resume",
                "t1",
                PROMPT
            ]
        );
        assert_eq!(
            headless_session("codex", AgentSession::Assign("t1".into())),
            headless_session("codex", AgentSession::Fresh)
        );
    }

    #[test]
    fn headless_claude() {
        let spec = headless("claude", Path::new("/nope"));
        assert_eq!(
            spec.argv,
            [
                "claude",
                "-p",
                PROMPT,
                "--output-format",
                "stream-json",
                "--verbose",
                "--dangerously-skip-permissions",
                "--append-system-prompt",
                &agent_guide()
            ]
        );
        assert!(spec.env.is_empty());
    }

    #[test]
    fn headless_codex() {
        let spec = headless("codex", Path::new("/nope"));
        let guide = serde_json::to_string(&agent_guide()).unwrap();
        assert_eq!(
            spec.argv,
            [
                "codex",
                "exec",
                "--json",
                "--dangerously-bypass-approvals-and-sandbox",
                "-c",
                &format!("developer_instructions={guide}"),
                PROMPT
            ]
        );
        assert!(spec.env.is_empty());
    }

    #[test]
    fn headless_cursor() {
        let spec = headless("cursor", Path::new("/nope"));
        assert_eq!(
            spec.argv,
            [
                "cursor",
                "-p",
                "--force",
                "--output-format",
                "stream-json",
                PROMPT
            ]
        );
        assert!(spec.env.is_empty());
    }

    #[test]
    fn headless_opencode_sets_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        let spec = headless("opencode", dir.path());
        assert_eq!(
            spec.argv,
            ["opencode", "run", "--format", "json", "--thinking", PROMPT]
        );
        let cfg = dir.path().join("p").join("opencode-headless");
        assert_eq!(
            spec.env,
            [(
                "OPENCODE_CONFIG_DIR".to_string(),
                cfg.to_string_lossy().into_owned()
            )]
        );
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(cfg.join("opencode.json")).unwrap())
                .unwrap();
        assert_eq!(config["permission"], "allow");
        assert_eq!(
            std::fs::read_to_string(config["instructions"][0].as_str().unwrap()).unwrap(),
            agent_guide()
        );
        assert!(!cfg.join("plugins").exists());
    }

    #[test]
    fn headless_unknown_appends_prompt() {
        let spec = headless_spec(
            &Provider {
                id: "fake".into(),
                command: vec!["sh".into(), "fake.sh".into(), "resolved".into()],
            },
            PROMPT,
            Path::new("/nope"),
            "p",
            &AgentSession::Resume("x".into()),
            Some("m1"),
        );
        assert_eq!(spec.argv, ["sh", "fake.sh", "resolved", PROMPT]);
        assert!(spec.env.is_empty());
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
