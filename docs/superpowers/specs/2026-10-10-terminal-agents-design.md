# Terminal agents: run coding CLIs in a PTY instead of ACP (spec)

Status: **draft 2026-10-10**, awaiting review.
Tracker: GitHub Issues, `Blaise1030/cms`.
Prior art: `~/Developer/ide` (tabsh) — same stack (Rust, axum `ws`, `portable-pty` 0.9, xterm 5.5).
References below as `tabsh:<path>`.

## Intent

Stop maintaining our own agent-protocol layer for the time being. The workspace runs the real
coding CLI (`claude`, `codex`, `opencode`) in a PTY and streams it to an xterm.js panel; the user
drives the CLI directly and gets all of its native features.

Must keep working: git checkpoints on the active change branch (Phase 2 depends on them) and the
"start a change first" guard. Accepted trade-off: a raw TUI is a rougher UX for non-developer
writers than the chat UI.

### Success criteria

1. Opening a project with an active change shows a terminal running the chosen CLI in the
   project directory.
2. Reloading the page reattaches to the same CLI session with its screen intact.
3. When the CLI finishes a turn, a checkpoint commit lands on the change branch and
   `checkpoint.created` fires, with no settle heuristic.
4. The preview element picker inserts the element reference into the CLI's prompt without
   submitting it.
5. No ACP code, chat module or `/api/agent*` routes remain.

## Decisions

| # | Decision |
|---|----------|
| D1 | Embedded PTY terminal (not headless JSON mode). |
| D2 | Delete the ACP client and chat UI outright; git history keeps them. |
| D3 | Checkpoints are triggered by each CLI's own turn-end hook, injected per launch. User-global CLI config is never modified. |
| D4 | PTY lives in the daemon, bridged over a WebSocket (no tmux, no ttyd). |
| D5 | Sessions outlive the browser tab but not the daemon. No scrollback persistence across daemon restarts. |
| D6 | Starting a terminal is refused on `main` for projects with a GitHub remote (same rule and error as today's prompt route). |
| D7 | Permissions are the CLI's own in-terminal prompts. The project's `auto_approve` maps to each CLI's bypass flag. |

## Removed

- `src/acp.rs`, `src/agents_config.rs`, `src/http/agent.rs` and the `/api/agents`, `/api/agent/*` routes.
- `tests/acp.rs`, `examples/fake_acp_agent.rs`.
- `web/src/modules/chat/` (ChatPanel, ChatMessage, ComposerEditor, ToolCard, PermissionPrompt,
  store, types). `elementRef.ts` moves to the terminal module (see Element picker).
- `history.jsonl` writing and `GET /api/agent/history`. Each CLI keeps its own history.
- Chat event kinds `chat.chunk`, `chat.tool`, `chat.plan`, `chat.done`, `chat.error`,
  `permission.request` (from `web/src/core/types.ts` and the socket handling).

`--agents-config` / `~/.config/orso/agents.json` remain as the override for each provider's
base command (see Providers).

## Architecture

### `src/providers.rs` (replaces `agents_config.rs`)

Resolves a provider id to the argv to launch, including hook injection and the bypass flag.

| Provider | Base argv (overridable) | Hook injection | `auto_approve` flag |
|----------|-------------------------|----------------|---------------------|
| `claude` | `claude` | `--settings '<json>'` with a `Stop` hook running `<exe> hook turn-end` | `--dangerously-skip-permissions` |
| `codex` | `codex` | `-c 'notify=["<exe>","hook","turn-end"]'` | `--dangerously-bypass-approvals-and-sandbox` |
| `opencode` | `opencode` | daemon-owned config dir containing a plugin that runs `<exe> hook turn-end` on session idle (adapted from `tabsh:src/cli/setup.md`) | its config `permission` set to allow |

`<exe>` is `std::env::current_exe()`. The override file keeps its current shape
(`{ "<id>": ["argv", ...] }`) and replaces only the base argv; hook and bypass flags are always
appended by the daemon. Verified against claude 2.1.296, codex-cli 0.162.1, opencode v2.0.25.
To confirm during implementation and pin with a test: the OpenCode plugin event name and
config-dir env var, and whether codex's hook-trust gate (`--dangerously-bypass-hook-trust`
exists) applies to a `-c notify=` override.

`GET /api/terminal/providers` → `{ providers: [{ id, command }] }` (the base argv, for the picker).

### `src/terminal.rs` — `TerminalManager`

At most one live terminal per project. Owned by `AppState`, replacing `agents`.

- **spawn(project, provider)**: checks D6, then opens a PTY (80×24) via `portable-pty` running
  `$SHELL -lc 'exec <argv>'` (login shell so the user's PATH applies; `$SHELL` falls back to the
  passwd shell, then `/bin/sh`, as in `tabsh:src/sessions/pty.rs:279`). cwd is the project dir.
  Env: `TERM=xterm-256color`, `ORSO_URL`, `ORSO_PROJECT`, `ORSO_HOOK_TOKEN`
  (fresh random per session). Fails with `conflict` if a terminal is already live for the project.
- **I/O**: one writer thread fed by an mpsc channel; one reader thread reading 8 KB chunks,
  appending to a 512 KB scrollback ring and publishing on a broadcast channel (capacity 1024).
  The ring append and broadcast happen under one lock (`tabsh:src/sessions/ws.rs:54-80`).
- **Mode tracking**: port `tabsh:src/sessions/modes.rs` so terminal modes set by bytes trimmed
  off the ring's front (alt screen, bracketed paste, cursor/mouse modes) are prefixed to the
  replay. Handles escape sequences split across chunks.
- **resize(cols, rows)**, **write(bytes)**.
- **kill(project)**: `SIGTERM` to the child's process group, `SIGKILL` after 2 s, then reap.
  Also run for every terminal on daemon shutdown and when a project is deleted.
- On child exit: broadcast an exit message, drop the session, emit `terminal.exited { projectId, code }`.

### HTTP / WebSocket (`src/http/terminal.rs`)

- `POST /api/terminal` `{ project, provider }` → `{ running: true }`. Errors: D6 guard (existing
  `require_change` error), unknown provider (`badreq`), already running (`conflict`).
- `DELETE /api/terminal/{project}` → kills it.
- `GET /api/terminal/{project}` → `{ running, provider? }`.
- `GET /api/terminal/{project}/ws` — framing copied from tabsh:
  - binary frames: PTY bytes, both directions;
  - text frames, client → server: `{"cols":N,"rows":N}`;
  - text frames, server → client: `{"exit":true,"code":N}`;
  - the first server frame is always the replay (mode prefix + ring) taken atomically with the
    subscription, so no bytes are lost or doubled;
  - a client that lags past the broadcast capacity is closed and is expected to reconnect.
- `POST /api/terminal/turn-end` `{ project, token, summary? }` — called by the hook subcommand.
  See Checkpoints.

### Hook subcommand (`<exe> hook turn-end`)

A hidden clap subcommand, modelled on `tabsh:src/cli/status.rs`. Reads the hook payload from
stdin, or from a single trailing argument (Codex `notify` passes JSON as argv). Extracts a summary:

- Codex: last entry of `input-messages` (the user's prompt).
- Claude: `last_assistant_message` if present, otherwise none.
- OpenCode: whatever the plugin passes.

POSTs `{ project: $ORSO_PROJECT, token: $ORSO_HOOK_TOKEN, summary }` to
`$ORSO_URL/api/terminal/turn-end`. Missing env (CLI run outside the orso) → exit 0
silently. Network errors → print to stderr, exit 0 (never break the CLI's turn).

### Checkpoints

On `turn-end`:

1. Token must match the live session for that project → otherwise `403`.
2. `require_change` (existing, `src/http/checkpoints.rs`) → on failure emit
   `checkpoint.blocked { projectId, reason }`, return `200` without committing.
3. `git::checkpoint(dir, message)` where message is the summary truncated to one line (≤72
   chars), or `Update <n> files: <first few paths>` when there is no summary or it's empty.
   No changes → no commit, no event.
4. Emit `checkpoint.created` as today.

Turn-end calls for one project are serialized (a per-project mutex) so overlapping hooks can't
race on git.

## Web

### `web/src/modules/terminal/`

- `TerminalPanel.vue`: provider picker + Start when nothing is running; otherwise an xterm
  (`@xterm/xterm` 5.5, `@xterm/addon-fit` 0.10, from npm since assets are embedded).
  - A ResizeObserver calls `fit()` then sends `{cols, rows}`.
  - The first (replay) frame is written immediately with `onData` muted while it's parsed
    (`tabsh:web/src/app/sessions/terminal.ts:216`), so xterm doesn't answer stale terminal
    queries back into the CLI. Live output is batched per animation frame.
  - Fonts are awaited before the terminal is opened (`tabsh:settings.ts:42`).
  - On `{exit}`: overlay "Session ended (code N)" with Restart. Exit code 127 → "`<cmd>` not
    found on PATH".
  - Socket closed without exit → reconnect with backoff; the replay restores the screen.
- `store.ts` (Pinia): running state, provider, connection state.
- `elementRef.ts` (moved from chat): the element picker builds the same reference text as today and
  sends it as a single bracketed-paste write (`ESC[200~ … ESC[201~`), inserting without submitting.

`ProjectPage.vue` puts `TerminalPanel` where `ChatPanel` was. `HomePage.vue` loses its chat
reference (provider choice moves to the panel).

## Error handling summary

| Situation | Behaviour |
|-----------|-----------|
| Start on `main` with a remote | `POST /api/terminal` returns the existing "start a change first" error; the panel shows it. |
| CLI not installed | Shell exits 127 → panel message naming the command. |
| CLI exits / crashes | `{exit, code}` frame + `terminal.exited`; Restart available. |
| Browser closes | PTY keeps running; next open replays. |
| Daemon restarts | Terminals are gone; panel shows Start. |
| Bad hook token | `403`, no commit. |
| Hook while on `main` | `checkpoint.blocked`, no commit. |
| Slow client | Socket closed, client reconnects and gets a fresh replay. |

## Testing

- **Rust integration** (`tests/terminal.rs`) with a fake CLI script (`tests/fixtures/fake_cli.sh`)
  that prints a banner, echoes stdin, and on a trigger line runs `$EXE hook turn-end` with a
  payload. Cover: spawn + output, WS first-frame replay after output, resize reaches the PTY
  (`stty size`), turn-end → checkpoint commit + `checkpoint.created`, bad token → 403,
  turn-end on `main` with remote → `checkpoint.blocked`, kill reaps a grandchild process
  (process-group kill), D6 refusal, double start → conflict.
- **Unit**: ported `modes.rs` tests; `providers.rs` argv assembly for each provider with and
  without `auto_approve` and with an override file; hook payload parsing for each CLI shape.
- **Web (vitest)**: replay frame mutes `onData`; resize message on fit; exit overlay; element
  ref is wrapped in bracketed paste.
- **e2e**: open a project with a change, start the fake provider, see its banner.

## Out of scope

- Resuming CLI sessions after a daemon restart (CLIs have `--resume`; revisit later).
- Multiple terminals per project, plain shell tabs.
- Rendering CLI output as chat, or any parsing of CLI output.
