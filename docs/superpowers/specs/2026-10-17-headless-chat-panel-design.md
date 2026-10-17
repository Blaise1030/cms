# Headless chat panel: drive the coding CLIs in JSON mode behind a chat UI (spec)

Status: **draft 2026-10-17**, awaiting review.
Tracker: GitHub Issues, `Blaise1030/cms`.
Revisits: `2026-10-10-terminal-agents-design.md` (its "rendering CLI output as chat" was out of scope).

## Intent

The terminal panel gives power users every CLI feature, but a raw TUI is hard for non-developer
writers: no discoverability, cryptic permission prompts, noisy output, odd copy/scroll. Add a chat
panel as the default way to talk to the agent, while still running the user's own installed CLI
(`cursor-agent`, `claude`, `codex`) so it uses their login, subscription, settings, skills and MCP
servers. The terminal stays as an "Advanced" tab on the same conversation.

### Success criteria

1. With a project open, the agent panel opens on **Chat**. Sending a message runs the chosen CLI
   headless in the project directory and streams its reply, reasoning and tool steps into the chat.
2. Each turn that changed files ends with a checkpoint commit (as today) and a marker in the chat
   listing the changed files with a **Restore** button.
3. Reloading the page mid-turn rejoins the running turn without losing or doubling output.
4. Switching to **Terminal** resumes the same CLI conversation; switching back continues it.
   Only one of them runs at a time.
5. The element picker and design requests reach the chat when it is the active tab.
6. A missing CLI or an expired login shows a plain-language message, not raw JSON or a stack trace.

## Decisions

| # | Decision |
|---|----------|
| D1 | One headless CLI process per turn, resumed by the CLI's own session id. No long-lived process, no ACP. |
| D2 | Chat turns always auto-approve (each CLI's bypass flag). Safety is the per-turn checkpoint + Restore, not approval prompts. |
| D3 | The daemon translates each CLI's JSON lines into the AI SDK UI message stream (SSE). The browser uses `@ai-sdk/vue` `Chat` + `DefaultChatTransport`. |
| D4 | Chat and terminal share one conversation (one `sessions.json` record, one agent session id). At most one of them is running per project. |
| D5 | Turns typed in the terminal are not copied into the chat transcript; the chat shows a "Continued in terminal" marker instead. |
| D6 | UI uses the current shadcn-vue components (Trace, Code Block, Message, Bubble, Marker, Message Scroller, Input Group). The unused AI Elements Vue folder `web/src/components/ai-elements/` is deleted. |
| D7 | Each turn shows: assistant text, a collapsed "Thinking" row, one-line tool steps that expand to input/output, and a changed-files marker. |

## Findings this spec relies on (checked 2026-10-17)

Scratch git repos, prompt "append a line to about.md, run `ls`, reply in one sentence", then a
resumed turn asking what was added.

| CLI | Version | Turn 1 | Resume | Billing signal |
|-----|---------|--------|--------|----------------|
| Cursor | 2026.10.16-e3cb207 | `cursor-agent -p --force --output-format stream-json <p>`: edit made, exit 0 | `--resume <session_id>`: remembered | `system/init` has `apiKeySource: "login"` |
| Codex | codex-cli 0.162.1 | `codex exec --json --dangerously-bypass-approvals-and-sandbox <p>`: edit made, exit 0 | `codex exec --json … resume <thread_id> <p>`: remembered | ChatGPT login (no `OPENAI_API_KEY` set) |
| Claude | 2.1.301 | `claude -p <p> --output-format stream-json --verbose --dangerously-skip-permissions`: **not verified**, OAuth session expired | not verified | `system/init` has `apiKeySource` |

Event shapes seen:

- **Cursor**: `system/init` (session_id, model, apiKeySource) → `user` → `thinking/delta`… `thinking/completed`
  → `tool_call/started|completed` with one key per tool kind (`readToolCall`, `editToolCall` with
  `result.success.diffString`, `shellToolCall`) and `call_id` → `assistant` (text) → `result`
  (`subtype`, `is_error`, `result`, `usage`).
- **Codex**: `thread.started` (thread_id) → `turn.started` → `item.started|completed` with
  `item.type` ∈ `agent_message`, `command_execution` (command, aggregated_output, exit_code),
  file changes, MCP tool calls → `turn.completed` (usage). `turn.failed` / `error` on failure.
- **Claude**: many `system/hook_started|hook_response` lines (the user's own hooks) → `system/init`
  → `assistant` / `user` (tool_use / tool_result content blocks) → `result`. **On the expired-login
  failure the final `result` still had `subtype: "success"`**; only `terminal_reason: "api_error"`
  and the synthetic assistant text "Failed to authenticate: …" showed the failure.

Before implementation: log in to Claude, record its turn + resume transcript, and confirm Codex
accepts `-c developer_instructions=…` before `resume`.

## Architecture

### Providers: `headless_spec` takes a session

`providers::headless_spec(provider, prompt, state_dir, project_id)` gains `session: &AgentSession`:

| Provider | Fresh | Assign(id) | Resume(id) |
|----------|-------|------------|------------|
| `cursor` | as today | `--resume <id>` (chat made by `create-chat`) | `--resume <id>` |
| `claude` | as today | `--session-id <id>` | `--resume <id>` |
| `codex` | as today | n/a (treated as Fresh) | `exec --json … -c developer_instructions=… resume <id> <prompt>` |
| others | as today | ignored | ignored |

Auto-resolve keeps calling it with `AgentSession::Fresh`, so its behaviour does not change.

### `src/chat/` — `ChatManager`

Owned by `AppState` next to `terminals`.

- **One running turn per project.** `start` fails with `conflict` if a turn is running or
  `state.terminals.status(project)` is `Some`. `TerminalManager::start` likewise refuses while a
  chat turn is running (same `conflict` error, message "The agent is busy in Chat").
- **Turn lifecycle**:
  1. Resolve the session (below), apply the same guard as the terminal: `write_cursor_rule` for
     Cursor; `require_change` only for providers without the git guide.
  2. Spawn via `login_command(argv, dir)` (login shell PATH, as the terminal), stdout piped,
     stderr to `{state_dir}/{project}/chat-logs/{chatId}.log`, own process group, `kill_on_drop`.
  3. Emit `terminal.busy { projectId, busy: true }` (the event the preview store's `agentBusy`
     already follows), and `busy: false` after turn-end. The event name stays as is.
  4. Read stdout line by line → translator → `UiChunk`s. Each chunk is appended to the turn's
     buffer and broadcast (capacity 1024), under one lock, like the terminal ring.
  5. On exit: run the shared turn-end path (below), emit the final chunks, save the transcript,
     emit `terminal.busy { busy: false }` and `chat.turn { projectId, chatId }`.
- **Stop**: `killpg` SIGTERM, SIGKILL after 2 s (same helper as the terminal). The stream ends
  with an `abort` chunk; the turn still goes through turn-end so partial edits are checkpointed.
- **Daemon shutdown / project delete**: kill running turns, as for terminals.

### Session resolution

The chat id is a `sessions.json` record id (`terminal::history`).

- **New chat**: create a record as `POST /api/terminal` does (`new_agent_session`: Claude gets a
  UUID, Cursor gets `create-chat`, Codex starts Fresh).
- **Later turns**: `Resume(record.agent_session_id)`.
- **Codex Fresh**: the translator reports `thread.started.thread_id`; the manager stores it as
  `agent_session_id`. Cursor/Claude also report `system/init.session_id`; if it differs from the
  record, the record is updated (the CLI's id wins).
- The terminal's `POST /api/terminal { resume: chatId }` already resumes from the same record,
  which is how D4 works with no new terminal code.

### Translators: `src/chat/translate/{cursor,claude,codex}.rs`

```rust
pub trait Translator {
    /// One stdout line in, zero or more chunks out. Non-JSON lines are ignored (logged).
    fn feed(&mut self, line: &str) -> Vec<UiChunk>;
    /// Process exited: close open parts, produce the error chunk if the turn failed.
    fn finish(&mut self, exit: Option<i32>) -> Vec<UiChunk>;
    fn session_id(&self) -> Option<&str>;
    /// Final assistant text, used as the checkpoint commit summary.
    fn summary(&self) -> Option<&str>;
}
```

`UiChunk` is a serde enum matching the AI SDK 7 `UIMessageChunk` subset we emit: `start`,
`start-step`, `finish-step`, `text-start|delta|end`, `reasoning-start|delta|end`,
`tool-input-available`, `tool-output-available`, `tool-output-error`, `data-checkpoint`,
`message-metadata`, `error`, `abort`, `finish`.

Mapping rules:

- Assistant text → text part. Cursor/Claude deliver whole messages; emit start/delta/end in one go.
  Codex `agent_message` items likewise.
- Thinking/reasoning → reasoning part (Cursor `thinking/delta`, Claude `thinking` blocks, Codex
  reasoning items if present).
- Tool calls → `dynamic-tool` parts (`tool-input-available` on start, `tool-output-available` /
  `tool-output-error` on completion), keyed by the CLI's call id. `toolName` is normalised to
  `read | edit | shell | search | web | mcp | other`; `input` keeps a small `{ path? | command? |
  query? }` plus the raw args; outputs are truncated to 16 KB.
- Usage → `message-metadata { provider, model, usage }` before `finish`.
- Claude `system/hook_*` lines are ignored.
- **Failure detection** (`finish`): exit code 127 → `error` "`<cmd>` is not installed or not on
  PATH"; Claude `terminal_reason` other than a normal end, `is_error: true`, or Cursor
  `result.is_error` → `error` with the CLI's text; any of "authenticate", "OAuth", "log in" in
  that text → error code `login`, message "Your <Provider> login has expired. Open the Terminal tab
  and run `/login`." Non-zero exit with no result → `error` with the last 20 stderr lines.

### Shared turn-end

Extract the body of `http::terminal::turn_end` after the token check into
`checkpoints::finish_turn(state, project, summary) -> Result<Option<Checkpoint>, ApiError>`
(record turn, `require_change`/`checkpoint.blocked`, `git::checkpoint`, `checkpoint.created`,
`finish_pending_update`). The terminal hook and the chat manager both call it. Chat then emits
`data-checkpoint { sha, summary, files }` (files from `git::changed_paths` before the commit).
On `checkpoint.blocked` it emits `data-checkpoint { blocked: reason }` so the chat can explain.

### Transcript

`{state_dir}/{project}/chats/{chatId}.jsonl`, one AI SDK `UIMessage` per line (user message, then
the assembled assistant message), appended at turn end. The record's `title` is set from the
first user message (first line, 60 chars) when empty. A terminal turn on the same record appends
a `{ role: "assistant", parts: [{ type: "data-terminal" }] }` marker (D5).

### HTTP (`src/http/chat.rs`)

- `POST /api/chat/{project}` — body is what `DefaultChatTransport` sends:
  `{ id, messages, trigger, provider }` (`provider` via the transport's `body`). Uses the last
  user message's text parts as the prompt (file parts are ignored for now). `id` unknown → new
  record. Response: `text/event-stream` with header `x-vercel-ai-ui-message-stream: v1`, one
  `data: <chunk json>` per chunk, then `data: [DONE]`. Errors before spawning are normal JSON
  `ApiError`s (guard, unknown provider, conflict).
- `GET /api/chat/{project}/{chatId}/stream` — resume: if a turn is running for that chat, send
  the buffered chunks then live ones (taken atomically with the subscription); otherwise `204`.
- `GET /api/chat/{project}/{chatId}` → `{ messages: UIMessage[], provider, running }`.
- `GET /api/chat/{project}` → `{ chats: [{ id, provider, title, updatedAt }], running? }` (from
  `sessions.json`).
- `DELETE /api/chat/{project}/turn` → stop.

Auth is the daemon's existing token middleware.

## Web

### `web/src/modules/chat/`

- `ChatPanel.vue`
  - `new Chat({ id: chatId, messages, transport: new DefaultChatTransport({ api:
    `/api/chat/${project}`, body: () => ({ provider }), prepareReconnectToStreamRequest }) })`,
    `resume: true` on mount so a reload rejoins a running turn. Initial messages come from
    `GET /api/chat/{project}/{chatId}`. The daemon token goes in the transport headers, as `core/api.ts` does.
  - Conversation: `MessageScroller` + `Message`/`Bubble` for user turns.
  - Composer: `InputGroup` textarea, provider select (Cursor CLI / Claude Code / Codex from
    `GET /api/terminal/providers`, locked once the chat has turns), Send / Stop (`chat.stop()`
    plus `DELETE …/turn`). Empty chat shows 3 starter suggestions.
  - "New chat" button; a small menu lists past chats (`GET /api/chat/{project}`).
- `ChatMessage.vue` renders an assistant message's parts:
  - `text` → markdown via `vue-stream-markdown` (already a dependency; today only the deleted
    `MessageResponse.vue` uses it).
  - `reasoning` → collapsed "Thinking" row (Trace).
  - `dynamic-tool` → one Trace line: "Read about.md", "Edited about.md", "Ran `ls`", "Searched
    …"; expands to Code Block input/output; errors in red.
  - `data-checkpoint` → Marker: "Saved N files" + file list + **Restore**, which opens the same
    confirm dialog as `CheckpointsPanel` and calls `api.restoreCheckpoint`. Blocked → Marker
    "Not saved: <reason>".
  - `data-terminal` → Marker "Continued in Terminal".
  - Chat-level `error` → inline Alert; code `login` / not-installed add an **Open Terminal** button.
- `store.ts` (Pinia): active chat id per project, provider, running flag (also driven by
  `chat.turn` events so other tabs stay in sync).

### Agent panel tabs

`ProjectPage.vue` wraps the agent area in `Tabs`: **Chat** (default) and **Terminal** (the
existing `TerminalPanel`, unchanged except it starts with `resume: <active chatId>` when one
exists). The tab that is not running shows a tooltip and disabled Start/Send while the other is
busy ("The agent is busy in Terminal").

### Outbox

`useAgentOutboxStore` drains into the active tab. In Chat: `submit: true` text is sent as a
message; element references are inserted into the composer without sending (same text as
`elementPrompt`). `askAgentToApplyDesign` and conflict helpers need no change.

### Components and dependencies

- `pnpm dlx shadcn-vue@latest add trace code-block`.
- Delete `web/src/components/ai-elements/` (nothing imports it).
- Add `@ai-sdk/vue@4.0.143`; bump `ai` to `7.0.143` (the version it pins).

## Error handling summary

| Situation | Behaviour |
|-----------|-----------|
| Send while terminal running / turn running | `409 conflict`, composer shows "busy" message. |
| Provider without git guide on `main` with a remote | Existing "start a change first" error, inline. |
| CLI not installed (exit 127) | Inline error naming the command + Open Terminal. |
| Login expired (any CLI) | Inline error code `login`: "run `/login` in the Terminal tab". |
| CLI crashes / non-zero exit | Inline error with last stderr lines; partial edits still checkpointed. |
| User presses Stop | Process group killed, `abort` chunk, partial edits checkpointed. |
| Browser reload mid-turn | Resume stream replays buffer then live output. |
| Browser closed mid-turn | Turn keeps running; transcript saved at the end. |
| Daemon restart mid-turn | Turn killed; transcript has only the user message; next send resumes the CLI session. |
| Turn ends on `main` with edits | `checkpoint.blocked` + "Not saved" marker. |

## Testing

- **Fixtures**: `tests/fixtures/chat/{cursor,codex,claude}/*.jsonl` — recorded transcripts
  (turn, resume, tool error, login failure), with home paths and skill contents scrubbed.
- **Rust unit**: each translator against its fixtures → expected `UiChunk` sequence (snapshot);
  Claude "success but api_error" → `login` error; `headless_spec` argv for Fresh/Assign/Resume per
  provider; `finish_turn` extracted with existing terminal tests still passing.
- **Rust integration** (`tests/chat.rs`): a fake headless CLI (`tests/fixtures/fake_headless.sh`)
  that replays a fixture and optionally edits a file. Cover: SSE body + header, transcript saved,
  checkpoint commit + `data-checkpoint`, resume stream after a mid-turn reconnect (no duplicates),
  stop kills the process group, chat/terminal mutual exclusion both ways, guard on `main`,
  exit 127 → not-installed error.
- **Web (vitest)**: `ChatMessage` renders each part type; Restore requires confirm; error
  variants show Open Terminal; outbox routes to the active tab; provider select locks after the
  first turn.
- **e2e**: project with a change, fake provider, send a message → reply text, a tool line and a
  "Saved 1 file" marker appear; switch to Terminal shows the same session resumed.

## Out of scope

- Approve/deny prompts inside the chat (D2).
- Attachments and images in the composer (file parts are ignored).
- Copying terminal turns into the chat transcript (D5).
- OpenCode and custom providers in the chat (they keep the terminal); can be added later with a
  translator each.
- Hosted / Flue-based agents.
- Restoring a checkpoint does not rewind the CLI's conversation; the agent is told nothing.
