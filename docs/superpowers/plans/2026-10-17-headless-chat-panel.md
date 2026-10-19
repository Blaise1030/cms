# Headless Chat Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Chat tab (default) beside the existing Terminal tab that runs the user's own `cursor-agent` / `claude` / `codex` headless per turn and renders their JSON output as a chat with tool steps and checkpoint markers.

**Architecture:** The Rust daemon spawns one headless CLI process per turn (resumed by the CLI's session id), translates each CLI's JSON lines into AI SDK 7 `UIMessageChunk`s, streams them as SSE and saves a per-chat transcript. The Vue app uses `@ai-sdk/vue` `Chat` + `DefaultChatTransport` and renders parts with shadcn-vue Trace / Code Block / Message / Marker components.

**Tech Stack:** Rust (axum 0.8, tokio, serde), Vue 3 + Pinia, `ai@7.0.143`, `@ai-sdk/vue@4.0.143`, shadcn-vue, vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-10-17-headless-chat-panel-design.md`

## Global Constraints

- Chat providers: exactly `cursor`, `claude`, `codex`. Others are terminal-only (`400` "Chat supports Cursor CLI, Claude Code and Codex").
- Chat turns always auto-approve; no approval UI.
- SSE response headers: `content-type: text/event-stream`, `cache-control: no-cache`, `x-vercel-ai-ui-message-stream: v1`, `x-accel-buffering: no`. Frames `data: <json>\n\n`, ending `data: [DONE]\n\n`.
- Chat id = `sessions.json` record id, client-generated, must match `^[A-Za-z0-9_-]{1,64}$` (else `400`).
- Transcript path: `{state_dir}/{project}/chats/{chatId}.jsonl`, one `UIMessage` JSON per line. Stderr log: `{state_dir}/{project}/chat-logs/{chatId}.log`.
- Tool output stored/streamed truncated to 16 KB. Stderr tail kept: last 20 lines.
- Busy signalling reuses `terminal.busy { projectId, busy }`; turn end also emits `chat.turn { projectId, chatId }`.
- Chat and terminal are mutually exclusive per project: `409 conflict` "The agent is busy in Terminal" / "The agent is busy in Chat".
- Error copy (`data-agent-error`): `login` → "Your {Display} login has expired. Open the Terminal tab and run /login."; `not_installed` → "{Display} is not installed or not on your PATH."; `failed` → "{Display} stopped with an error: {text}". `{Display}` = `providers::display_name`.
- Web deps: `ai@7.0.143`, `@ai-sdk/vue@4.0.143`; shadcn-vue `trace` and `code-block`; delete `web/src/components/ai-elements/`.
- Verification commands: `cargo test`, `pnpm -C web test`, `pnpm -C web build` (runs `vue-tsc`), `just e2e`.

## Review Focus

- **Chat id from the client used in a file path** (`../x`, empty, 200 chars) → `400`, no file outside `chats/`. Test in Task 9.
- **Reload mid-turn** → resume replays buffered chunks then live ones, exactly one `start` chunk, no duplicated text. Test in Task 9.
- **Stop while the CLI has spawned children** (a shell command running) → whole process group gone, partial edits still checkpointed, stream ends with `abort`. Test in Task 8.
- **CLI prints non-JSON lines or a partial line** (warnings, banners, Claude hook noise) → ignored, turn continues. Test in Tasks 3–5.
- **Expired login reported as "success"** (Claude `subtype: "success"` + `terminal_reason: "api_error"`) → `login` agent error, not an empty reply. Test in Task 5.

---

### Task 1: Record and scrub CLI fixtures

**Prerequisite (human):** run `claude`, then `/login`. Stop and ask if `claude -p "say ok"` still fails.

**Files:**
- Create: `tests/fixtures/chat/{cursor,codex,claude}/turn.jsonl`, `resume.jsonl`, `tool-error.jsonl`
- Create: `tests/fixtures/chat/claude/login-expired.jsonl`
- Create: `tests/fixtures/chat/README.md` (how they were recorded: CLI versions, prompts, scrub rules)

- [ ] **Step 1:** In fresh scratch git repos (`/tmp/wb-fixtures/{cli}` with `about.md` = `# About\n`, one commit), record per CLI with the exact argv from spec "Findings": turn prompt `Append the line "Hello from the agent." to about.md, then run \`ls\` and reply with one short sentence.`; resume prompt `What line did you add in the previous turn? Answer in one sentence without using tools.`; tool-error prompt `Run \`cat missing.txt\` and tell me what happened.`
- [ ] **Step 2:** For Codex, record the resume with `-c developer_instructions="x"` placed before `resume`. If it fails, record the working order in the README; Task 2 uses that order.
- [ ] **Step 3:** Copy `/tmp/wb-spike/out/claude1.jsonl` to `claude/login-expired.jsonl` if it exists; otherwise hand-write it from the event shape in the spec (hook lines, `system/init`, synthetic assistant text "Failed to authenticate: OAuth session expired and could not be refreshed", `result` with `subtype: "success"`, `terminal_reason: "api_error"`).
- [ ] **Step 4:** Scrub: replace `$HOME` with `/home/user` and scratch paths with `/work/project`; truncate any string field over 2 KB to its first 200 chars + `…`; drop Claude `system/init` `slash_commands`, `agents`, `tools`, `mcp_servers` arrays to `[]`. Record in the README the `terminal_reason` value of a successful Claude turn.
- [ ] **Step 5:** Verify every line parses: `for f in tests/fixtures/chat/*/*.jsonl; do jq -c . "$f" >/dev/null || echo BAD $f; done` prints nothing.
- [ ] **Step 6:** Commit `test: recorded headless CLI transcripts for chat translators`.

### Task 2: `headless_spec` takes an `AgentSession`

**Files:**
- Modify: `src/providers.rs` (`headless_spec`, tests module)
- Modify: `src/changes/auto_resolve.rs:607` (pass `&AgentSession::Fresh`)

**Interfaces:**
- Produces: `pub fn headless_spec(provider: &Provider, prompt: &str, state_dir: &Path, project_id: &str, session: &AgentSession) -> LaunchSpec`

- [ ] **Step 1: Write failing tests** in the `providers.rs` tests module (`PROMPT`, `p(id)` helpers already exist):
  - `headless_cursor_resume`: argv == `["cursor-agent","--resume","c1","-p","--force","--output-format","stream-json",PROMPT]` for `Resume("c1")`; same for `Assign("c1")`.
  - `headless_claude_sessions`: `Assign("u1")` argv ends with `["--session-id","u1"]`; `Resume("u1")` ends with `["--resume","u1"]`; `Fresh` unchanged from today.
  - `headless_codex_resume`: `Resume("t1")` argv == `["codex","exec","--json","--dangerously-bypass-approvals-and-sandbox","-c",<developer_instructions=…>,"resume","t1",PROMPT]` (or the order Task 1 recorded); `Assign` == `Fresh`.
- [ ] **Step 2:** `cargo test --lib providers::` → FAIL (wrong arity).
- [ ] **Step 3:** Add the parameter; update the existing headless tests and `auto_resolve.rs` to pass `&AgentSession::Fresh`.
- [ ] **Step 4:** `cargo test --lib providers:: && cargo test --test auto_resolve` → PASS.
- [ ] **Step 5:** Commit `feat(providers): resume sessions in headless mode`.

### Task 3: `UiChunk`, SSE framing and the Cursor translator

**Files:**
- Create: `src/chat/mod.rs` (`pub mod chunk; pub mod translate; pub mod transcript; pub mod manager;` — add modules as their tasks land), `src/chat/chunk.rs`, `src/chat/translate/mod.rs`, `src/chat/translate/cursor.rs`
- Modify: `src/lib.rs` (`pub mod chat;`)

**Interfaces:**
- Produces (`chunk.rs`):
  ```rust
  #[derive(Debug, Clone, PartialEq, Serialize)]
  #[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
  pub enum UiChunk {
      Start { message_id: String }, StartStep, FinishStep,
      TextStart { id: String }, TextDelta { id: String, delta: String }, TextEnd { id: String },
      ReasoningStart { id: String }, ReasoningDelta { id: String, delta: String }, ReasoningEnd { id: String },
      ToolInputAvailable { tool_call_id: String, tool_name: String, input: Value, dynamic: bool },
      ToolOutputAvailable { tool_call_id: String, output: Value, dynamic: bool },
      ToolOutputError { tool_call_id: String, error_text: String, dynamic: bool },
      #[serde(rename = "data-checkpoint")] DataCheckpoint { data: CheckpointData },
      #[serde(rename = "data-agent-error")] DataAgentError { data: AgentError },
      MessageMetadata { message_metadata: Value },
      Error { error_text: String }, Abort, Finish,
  }
  pub struct CheckpointData { sha: Option<String>, summary: Option<String>, files: Vec<String>, blocked: Option<String> } // camelCase, skip None
  pub struct AgentError { code: AgentErrorCode, message: String }
  pub enum AgentErrorCode { Login, NotInstalled, Failed } // snake_case
  pub fn sse_frame(chunk: &UiChunk) -> String;   // "data: {json}\n\n"
  pub const SSE_DONE: &str = "data: [DONE]\n\n";
  ```
  `dynamic` is always `true`.
- Produces (`translate/mod.rs`):
  ```rust
  pub trait Translator: Send {
      fn feed(&mut self, line: &str) -> Vec<UiChunk>;
      fn finish(&mut self, exit: Option<i32>, stderr_tail: &str) -> Vec<UiChunk>;
      fn session_id(&self) -> Option<&str>;
      fn summary(&self) -> Option<&str>;
  }
  pub fn for_provider(provider_id: &str) -> Option<Box<dyn Translator>>; // cursor|claude|codex
  pub fn is_chat_provider(provider_id: &str) -> bool;
  pub fn agent_error(provider_id: &str, exit: Option<i32>, text: &str) -> AgentError;
  pub fn truncate_output(s: &str) -> String; // 16 KB, char boundary, appends "…"
  ```
  Translators never emit `Start`/`StartStep`/`FinishStep`/`Finish` (the manager does). Text part ids `t{n}`, reasoning `r{n}`, counting from 0 per turn. Whole messages are emitted as start + one delta + end.
  `agent_error`: exit `Some(127)` → `NotInstalled`; text containing (case-insensitive) `authenticate`, `oauth`, `log in` or `login` → `Login`; else `Failed`. Messages per Global Constraints.

- [ ] **Step 1: Failing tests** in `chunk.rs`: exact JSON for `ToolInputAvailable` → `{"type":"tool-input-available","toolCallId":"c1","toolName":"edit","input":{"path":"a.md"},"dynamic":true}`; `DataCheckpoint` with only `files` → `{"type":"data-checkpoint","data":{"files":["a.md"]}}`; `DataAgentError` → `{"type":"data-agent-error","data":{"code":"not_installed","message":"…"}}`; `Start` → `{"type":"start","messageId":"m1"}`; `sse_frame` ends with `"\n\n"`. In `translate/mod.rs`: `agent_error("claude", Some(1), "Failed to authenticate: OAuth …").code == Login`; `agent_error("codex", Some(127), "") == NotInstalled` with message "Codex is not installed or not on your PATH."; `truncate_output` of 20 000 `é` is ≤ 16 KB + `…` and valid UTF-8.
- [ ] **Step 2: Failing Cursor tests** in `cursor.rs` reading `tests/fixtures/chat/cursor/*.jsonl` (`include_str!`):
  - `turn`: chunks contain a `ToolInputAvailable { tool_name: "edit", input.path ends with "about.md" }` followed later by its `ToolOutputAvailable` with the same id; a `tool_name: "shell"` with `input.command == "ls"`; `ReasoningStart/Delta/End` (deltas concatenated into one `ReasoningDelta` is fine); a final `TextDelta` equal to the fixture's assistant text; `session_id()` == fixture `system/init.session_id`; `summary()` == that text; then `finish(Some(0), "")` is empty.
  - `tool-error`: the shell call ends in `ToolOutputError` or `ToolOutputAvailable` with non-zero exit visible in output (whichever the fixture shows; assert the one it shows).
  - `noise`: `feed("not json")` and `feed("")` return `[]`.
  - `failure`: a `result` line with `"is_error":true,"result":"boom"` then `finish(Some(1), "")` → one `DataAgentError` with code `Failed`, message contains "boom".
- [ ] **Step 3:** `cargo test --lib chat::` → FAIL.
- [ ] **Step 4: Implement.** Cursor mapping: the single key under `tool_call` decides the tool — `readToolCall`→`read`; `editToolCall`/`writeToolCall`/`deleteToolCall`→`edit`; `shellToolCall`→`shell`; `grepToolCall`/`globToolCall`/`lsToolCall`→`search`; `webSearchToolCall`/`webFetchToolCall`→`web`; `mcpToolCall`→`mcp`; otherwise `other`. `input` = `{ path | command | query, raw: args }`. Output: edit → `result.success.diffString`; shell → stdout/stderr/exitCode from the result; read → `{ "lines": totalLines }` (never file content); `result.error`/`failure` → `ToolOutputError`. `thinking/delta` → reasoning (close on `thinking/completed`). `assistant` → text. `result` → `MessageMetadata { provider:"cursor", model, usage }`; remember `is_error`/`result` for `finish`.
- [ ] **Step 5:** `cargo test --lib chat::` → PASS.
- [ ] **Step 6:** Commit `feat(chat): UI message chunks and Cursor translator`.

### Task 4: Codex translator

**Files:** Create `src/chat/translate/codex.rs`; register in `for_provider`.

**Interfaces:** Consumes `Translator`, `UiChunk`, `agent_error`, `truncate_output` (Task 3).

- [ ] **Step 1: Failing tests** from `tests/fixtures/chat/codex/*.jsonl`:
  - `turn`: `session_id()` == `thread.started.thread_id`; each `agent_message` item → one text part (3 text parts for the spike-shaped fixture, or as many as the fixture has); `command_execution` → `ToolInputAvailable { tool_name:"shell", input.command }` on `item.started` and `ToolOutputAvailable` with `aggregated_output` on `item.completed`; `turn.completed` → `MessageMetadata` with `usage.output_tokens`; `summary()` == last `agent_message` text.
  - `tool-error`: a `command_execution` with `exit_code != 0` → `ToolOutputError` whose `error_text` contains the output.
  - `failure`: lines `{"type":"turn.failed","error":{"message":"You've hit your usage limit"}}` then `finish(Some(1), "")` → `DataAgentError` code `Failed`; with message `"Not logged in. Please run codex login"` → `Login`.
  - `noise`: non-JSON ignored.
- [ ] **Step 2:** `cargo test --lib chat::translate::codex` → FAIL.
- [ ] **Step 3: Implement.** Item types: `agent_message`→text; `reasoning`→reasoning; `command_execution`→`shell`; `file_change`→`edit` (`input.path` = first change path, `raw` = all changes); `mcp_tool_call`→`mcp`; `web_search`→`web`; `todo_list` ignored; `error` item / `turn.failed` / top-level `error` → remembered failure text.
- [ ] **Step 4:** `cargo test --lib chat::` → PASS.
- [ ] **Step 5:** Commit `feat(chat): Codex translator`.

### Task 5: Claude translator

**Files:** Create `src/chat/translate/claude.rs`; register in `for_provider`.

- [ ] **Step 1: Failing tests** from `tests/fixtures/chat/claude/*.jsonl`:
  - `turn`: `system/hook_*` lines produce nothing; `session_id()` from `system/init`; `tool_use` blocks map `Edit`→`edit`, `Bash`→`shell` (input.command `ls`); each `tool_result` (matched by `tool_use_id`) → output, or `ToolOutputError` when `is_error`; final text; `result` → `MessageMetadata { provider:"claude", model, usage, costUsd }`.
  - `login_expired` (`login-expired.jsonl`, exit 1): `finish(Some(1), "")` → exactly one `DataAgentError` with code `Login` and message "Your Claude Code login has expired. Open the Terminal tab and run /login."; the synthetic assistant text is **not** emitted as a text part.
  - `noise`: non-JSON ignored.
- [ ] **Step 2:** `cargo test --lib chat::translate::claude` → FAIL.
- [ ] **Step 3: Implement.** Tool names: `Read`→`read`; `Edit`/`Write`/`MultiEdit`/`NotebookEdit`→`edit`; `Bash`→`shell`; `Grep`/`Glob`/`LS`→`search`; `WebFetch`/`WebSearch`→`web`; `mcp__*`→`mcp`; else `other`. `thinking` blocks → reasoning. Failure when `result.is_error == true` or `result.terminal_reason` is present and differs from the success value recorded in Task 1 (a `const`). Hold assistant text whose `message.model == "<synthetic>"` as failure text instead of emitting it.
- [ ] **Step 4:** `cargo test --lib chat::` → PASS.
- [ ] **Step 5:** Commit `feat(chat): Claude translator`.

### Task 6: Shared turn-end (`finish_turn`)

**Files:**
- Modify: `src/http/checkpoints.rs` (add `finish_turn`, `TurnOutcome`; move `turn_end_lock` here)
- Modify: `src/http/terminal.rs:441-497` (`turn_end` calls `finish_turn`)
- Test: `tests/checkpoints.rs`

**Interfaces:**
- Produces:
  ```rust
  pub enum TurnOutcome { Committed { checkpoint: git::Checkpoint, files: Vec<String> }, Blocked(String), Unchanged }
  pub async fn finish_turn(state: &AppState, project_id: &str, summary: Option<&str>) -> Result<TurnOutcome, ApiError>;
  ```
  Takes the per-project lock, does `require_change` (→ `checkpoint.blocked` + `Blocked` only if dirty, else `Unchanged`), `git::changed_paths` (= `files`), `terminal::commit_message`, `git::checkpoint`, `checkpoint.created`, spawns `changes::finish_pending_update`. Terminal-only steps (`record_turn`, `set_idle`, `terminal.turn`) stay in `turn_end`.

- [ ] **Step 1: Failing test** `finish_turn_commits_and_lists_files` in `tests/checkpoints.rs`: on a change branch, write `a.md` and `b.md`, call `finish_turn(&state, id, Some("Add posts"))` → `Committed { files == ["a.md","b.md"] (sorted), checkpoint.summary == "Add posts" }`; second call → `Unchanged`. And `finish_turn_blocked_on_main_with_remote` → `Blocked(_)` with a dirty tree.
- [ ] **Step 2:** `cargo test --test checkpoints finish_turn` → FAIL.
- [ ] **Step 3:** Implement by extracting from `turn_end`.
- [ ] **Step 4:** `cargo test --test checkpoints --test terminal` → PASS (existing turn-end tests unchanged).
- [ ] **Step 5:** Commit `refactor(checkpoints): share turn-end commit between terminal and chat`.

### Task 7: Transcript store and message assembly

**Files:** Create `src/chat/transcript.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub fn valid_chat_id(id: &str) -> bool;                       // ^[A-Za-z0-9_-]{1,64}$
  pub fn path(state_dir: &Path, project_id: &str, chat_id: &str) -> PathBuf;
  pub fn read(state_dir: &Path, project_id: &str, chat_id: &str) -> Vec<Value>;   // missing → []
  pub fn append(state_dir: &Path, project_id: &str, chat_id: &str, messages: &[Value]) -> std::io::Result<()>;
  pub fn user_message(id: &str, text: &str) -> Value;           // {id, role:"user", parts:[{type:"text",text}]}
  pub fn assemble(message_id: &str, chunks: &[UiChunk]) -> Value;
  pub fn mark_terminal(state_dir: &Path, project_id: &str, chat_id: &str) -> std::io::Result<()>;
  ```
  `assemble` builds `{id, role:"assistant", parts, metadata?}`: text start/delta/end → `{type:"text",text}`; reasoning → `{type:"reasoning",text}`; `StartStep` → `{type:"step-start"}`; tool input → `{type:"dynamic-tool",toolName,toolCallId,state:"input-available",input}`, updated in place by output (`state:"output-available",output`) or error (`state:"output-error",errorText`); a tool still `input-available` at the end → `output-error` with `errorText:"Interrupted"`; `data-*` → `{type:"data-…",data}`; `MessageMetadata` → `metadata`. `mark_terminal` appends `{id:"term-<ms>", role:"assistant", parts:[{type:"data-terminal",data:{}}]}` only if the file exists and its last message is not already that marker.

- [ ] **Step 1: Failing tests:** `valid_chat_id` rejects `""`, `"../x"`, `"a/b"`, 65 chars, accepts a UUID; `assemble` on `[StartStep, TextStart t0, TextDelta "Hi", TextEnd, ToolInputAvailable c1 edit, ToolOutputAvailable c1, ToolInputAvailable c2 shell]` → exact parts with c2 `output-error`/`"Interrupted"`; `append` twice + `read` round-trips 2 messages; `mark_terminal` twice adds one marker; on a missing file adds none.
- [ ] **Step 2:** `cargo test --lib chat::transcript` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test --lib chat::` → PASS.
- [ ] **Step 5:** Commit `feat(chat): transcript store`.

### Task 8: `ChatManager` (turn lifecycle)

**Files:**
- Create: `src/chat/manager.rs`, `tests/fixtures/fake_headless.sh`, `tests/chat.rs`
- Modify: `src/state.rs` (`pub chats: Arc<ChatManager>`), `src/terminal/mod.rs` (make the process-group signal helper `pub(crate)`), `src/http/terminal.rs` (`new_agent_session` → `pub(crate)`)

**Interfaces:**
- Consumes: `headless_spec` (Task 2), `for_provider`/`agent_error` (Task 3), `finish_turn` (Task 6), `transcript::*` (Task 7), `terminal::login_command`, `history::{get, insert, update}`.
- Produces:
  ```rust
  pub struct TurnRequest { pub project_id: String, pub chat_id: String, pub provider: Provider, pub dir: PathBuf, pub prompt: String, pub session: AgentSession }
  pub struct Subscription { pub replay: Vec<UiChunk>, pub rx: broadcast::Receiver<Option<UiChunk>> } // None = turn over
  impl ChatManager {
      pub fn new(events: EventBus) -> Self;
      pub fn running(&self, project_id: &str) -> Option<String>;           // running chat id
      pub fn start(&self, state: AppState, req: TurnRequest) -> Result<Subscription, ApiError>;
      pub fn subscribe(&self, project_id: &str, chat_id: &str) -> Option<Subscription>;
      pub async fn stop(&self, project_id: &str);
  }
  ```
  `start`: conflict if `state.terminals.status(project).is_some()` ("The agent is busy in Terminal") or a turn is running ("The agent is busy in Chat"). Spawns `login_command(spec.argv)` + `spec.env`, cwd = dir, stdin null, stdout piped, stderr piped (copied to the chat log, last 20 lines kept), `process_group(0)`, `kill_on_drop(true)`. Emits `Start { message_id: "msg-<random>" }`, `StartStep`, translator chunks; on exit: translator `finish`, `finish_turn(summary)` → `DataCheckpoint` (`Committed` → sha/summary/files; `Blocked` → `blocked`; `Unchanged` → nothing), `FinishStep`, then `Finish` (or `Abort` if stopped), then `None`. Replay buffer append + broadcast (capacity 1024) under one lock. The `user_message` is appended to the transcript when the turn starts (so a daemon restart keeps it). Afterwards: append `assemble(...)` to the transcript; update the history record (`agent_session_id` from `translator.session_id()` when different; `title` from the first prompt line, 60 chars, if empty; `updated_at`); emit `terminal.busy false` and `chat.turn`. `terminal.busy true` at start. `stop`: SIGTERM the group, SIGKILL after 2 s.

`fake_headless.sh <fixture> [mode] …ignored flags`: if any arg is `create-chat`, print `chat-fake-1` and exit 0. Modes: `edit` → `printf 'hello\n' >> notes.md` then `cat` fixture; `slow` → print the fixture's first line, `sleep 1`, the rest, then `sleep 300` with a `(trap '' HUP TERM; exec sleep 300) &` child whose pid goes to `$FAKE_PIDFILE`; default → `cat` fixture. Exit `${FAKE_EXIT:-0}`.

- [ ] **Step 1: Failing tests** in `tests/chat.rs` (manager-level, providers overridden via `agents_config` so `cursor` = `["sh", fake_headless, cursor/turn.jsonl, mode]`; each test first inserts a `sessions.json` record for the chat id with `agent_session_id: "chat-fake-1"`):
  - `turn_streams_and_checkpoints`: mode `edit` on a change branch → collected chunks start with `Start`, `StartStep`; contain the fixture text; end `DataCheckpoint{files:["notes.md"]}`, `FinishStep`, `Finish`; transcript has 2 messages; history record has `agent_session_id` == fixture session id; events saw `terminal.busy` true then false and `chat.turn`.
  - `second_start_conflicts`: mode `slow`, second `start` → `409`, message "The agent is busy in Chat".
  - `stop_kills_group_and_checkpoints`: mode `slow` + edit → `stop` → child pid from `$FAKE_PIDFILE` gone within 3 s (`kill -0` fails); chunks end `DataCheckpoint`, `Abort`.
  - `missing_cli_is_not_installed`: provider argv `["/nonexistent/cursor-agent"]` → `DataAgentError{code:not_installed}`.
- [ ] **Step 2:** `cargo test --test chat` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test --test chat` → PASS.
- [ ] **Step 5:** Commit `feat(chat): ChatManager runs headless turns`.

### Task 9: Chat HTTP routes

**Files:**
- Create: `src/http/chat.rs`
- Modify: `src/http/mod.rs` (routes), `src/http/terminal.rs` (`start` → 409 "The agent is busy in Chat" when `state.chats.running`; `record_turn` → `transcript::mark_terminal`), `src/http/projects.rs:167` (stop chat on project delete)
- Test: `tests/chat.rs` (HTTP section, `oneshot` helpers as in `tests/checkpoints.rs`)

**Interfaces:**
- Produces routes:
  - `POST /api/chat/{project}` body `{ id, messages, provider? }` → SSE. Unknown `id` → new record (needs chat `provider`, `new_agent_session`); known → `Resume(agent_session_id)` (or `Fresh` if none yet). Prompt = text parts of the last `role:"user"` message joined; empty → `400`. Cursor → `write_cursor_rule` first (as `terminal::start`).
  - `GET /api/chat/{project}` → `{ chats: [{id, provider, title?, updatedAt}], running: chatId|null }`
  - `GET /api/chat/{project}/{chat}` → `{ messages, provider, running: bool }`
  - `GET /api/chat/{project}/{chat}/stream` → SSE (replay + live) or `204`
  - `DELETE /api/chat/{project}/turn` → `{ stopped: bool }`
  A lagged broadcast receiver ends the SSE body (client reconnects).

- [ ] **Step 1: Failing tests:**
  - `post_returns_ai_sdk_sse`: headers per Global Constraints; every frame starts `data: `; last frame `data: [DONE]`; then `GET …/{chat}` → roles `["user","assistant"]`; `GET /api/chat/{project}` lists it with a title.
  - `resume_replays_without_duplicates`: mode `slow`; POST (don't read body), then `GET …/stream` → body has exactly one `"type":"start"` and the fixture text once; `GET …/stream` when idle → `204`.
  - `rejects_bad_input`: id `../x` → `400`; provider `opencode` → `400` "Chat supports Cursor CLI, Claude Code and Codex"; no user text → `400`.
  - `chat_and_terminal_exclusive`: terminal running (fake_cli) → chat POST `409` "The agent is busy in Terminal"; chat running (slow) → `POST /api/terminal` `409` "The agent is busy in Chat".
  - `blocked_on_main`: edit mode on `main` with a remote → stream has `data-checkpoint` with `blocked`.
  - `terminal_turn_marks_transcript`: existing chat record, terminal resumed on it, turn-end hook → transcript's last message has a `data-terminal` part.
- [ ] **Step 2:** `cargo test --test chat` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test` → PASS (whole suite).
- [ ] **Step 5:** Commit `feat(chat): HTTP routes for the chat panel`.

### Task 10: Web dependencies and component swap

**Files:** `web/package.json`, `web/pnpm-lock.yaml`, new `web/src/components/ui/trace/*`, `web/src/components/ui/code-block/*`; delete `web/src/components/ai-elements/`.

- [ ] **Step 1:** `pnpm -C web add ai@7.0.143 @ai-sdk/vue@4.0.143`
- [ ] **Step 2:** `cd web && pnpm dlx shadcn-vue@latest add trace code-block` (accept overwrites only for files it creates).
- [ ] **Step 3:** `rm -rf web/src/components/ai-elements` and confirm `rg -n "ai-elements" web/src` prints nothing.
- [ ] **Step 4:** `pnpm -C web build && pnpm -C web test` → PASS.
- [ ] **Step 5:** Commit `chore(web): AI SDK Vue, shadcn-vue trace and code-block; drop AI Elements Vue`.

### Task 11: Web API, events and chat store

**Files:**
- Modify: `web/src/core/types.ts` (types + `chat.turn` event), `web/src/core/socket.ts` (`"chat.turn": { projectId: "string", chatId: "string" }`), `web/src/core/api.ts`
- Create: `web/src/modules/chat/store.ts`, `web/src/modules/chat/store.test.ts`

**Interfaces:**
- Produces (`api.ts`): `listChats(project): Promise<{ chats: ChatSummary[]; running: string | null }>`, `chatHistory(project, chatId): Promise<{ messages: UIMessage[]; provider: string; running: boolean }>`, `stopChat(project)`, `chatUrl(project): string` (`${origin}/api/chat/${enc(project)}`), `authHeaders(): Record<string,string>`.
- Produces (`store.ts`) `useChatStore`: `activeChatId(projectId): string | null`, `setActive(projectId, chatId)` (persisted under `orso.chat.active.<projectId>`), `newChat(projectId): string` (`crypto.randomUUID()`), `chats: Record<projectId, ChatSummary[]>`, `running: Record<projectId, string | null>`, `load(projectId)`. Subscribes to `chat.turn` (reload list) and `terminal.session` (set the active chat to the terminal's `sessionId`, so both tabs share the conversation).

- [ ] **Step 1: Failing tests:** `newChat` sets and persists the active id; `chat.turn` for the project calls `listChats` again; `terminal.session` sets `activeChatId` to its `sessionId`; socket decodes `chat.turn` and rejects one without `chatId`.
- [ ] **Step 2:** `pnpm -C web test src/modules/chat src/core` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command → PASS.
- [ ] **Step 5:** Commit `feat(web): chat API client and store`.

### Task 12: `ChatMessage.vue` (part rendering)

**Files:** Create `web/src/modules/chat/parts.ts`, `parts.test.ts`, `ChatMessage.vue`, `ChatMessage.test.ts`.

**Interfaces:**
- Produces: `toolLabel(part: DynamicToolUIPart): string` — `read` "Read {basename}", `edit` "Edited {basename}", `shell` "Ran {command}" (first line, 60 chars), `search` "Searched {query}", `web` "Searched the web", `mcp`/`other` "Used {raw tool name or 'a tool'}". `ChatMessage` props `{ projectId: string; message: UIMessage }`, emits `open-terminal`.

- [ ] **Step 1: Failing tests:** `toolLabel` for each kind (e.g. `/work/project/src/about.md` edit → "Edited about.md"). Component: text renders as markdown (`**b**` → `<strong>`); reasoning shows a collapsed "Thinking" trigger; a tool part shows its label and, when expanded, input/output in a code block; `output-error` shows `errorText` with `data-testid="tool-error"`; `data-checkpoint` with 2 files shows "Saved 2 files" (1 → "Saved 1 file") and a Restore button; clicking Restore opens the confirm dialog and `api.restoreCheckpoint(projectId, sha)` is called only after confirming; `blocked` shows "Not saved: {reason}"; `data-terminal` shows "Continued in Terminal"; `data-agent-error` code `login` shows its message and an "Open Terminal" button that emits `open-terminal`; code `failed` has no button.
- [ ] **Step 2:** `pnpm -C web test src/modules/chat` → FAIL.
- [ ] **Step 3:** Implement with `Message`/`Bubble` (user), `Trace*` (reasoning, tools), `CodeBlock`, `Marker` (checkpoint/terminal), `Alert` (agent error), `vue-stream-markdown` (text), `AlertDialog` copied from `CheckpointsPanel.vue`'s restore confirm.
- [ ] **Step 4:** Same command → PASS.
- [ ] **Step 5:** Commit `feat(web): render chat message parts`.

### Task 13: `ChatPanel.vue`

**Files:** Create `web/src/modules/chat/ChatPanel.vue`, `ChatPanel.test.ts`.

**Interfaces:**
- Consumes: `useChatStore`, api helpers (Task 11), `ChatMessage` (Task 12), `useTerminalStore().provider/providers/loadProviders`.
- Produces: props `{ projectId: string; active: boolean }`, emits `open-terminal`; exposes `insert(text: string)` and `send(text: string)` for the outbox (Task 14).
  `new Chat({ id, messages, transport: new DefaultChatTransport({ api: chatUrl(projectId), headers: authHeaders, body: () => ({ provider }) }) })`, rebuilt when the active chat id changes; `chat.resumeStream()` on mount when history `running`. Test ids: `chat-input`, `chat-send`, `chat-stop`, `chat-provider`, `chat-new`, `chat-history`, `chat-suggestion`. Empty-state suggestions (exact): "Write a new blog post about our latest update", "Make the homepage headline punchier", "Fix typos on the About page" — clicking one fills the input. Provider select lists only `cursor`/`claude`/`codex` from the providers list and is disabled once the chat has messages. Stop = `chat.stop()` + `api.stopChat`. While the terminal is running, Send is disabled with tooltip "The agent is busy in Terminal". `chat.error` → Alert above the composer.

- [ ] **Step 1: Failing tests** (mock `fetch` returning an SSE `ReadableStream` built from a fixed chunk list): sending "hi" POSTs to `…/api/chat/p1` with `provider:"cursor"` and an `Authorization` header, then renders the streamed text; provider select disabled after the first message; Stop calls `DELETE …/api/chat/p1/turn`; empty chat shows 3 suggestions; terminal running disables Send; a `409` response shows its message in the Alert; `chat-new` starts an empty chat with a new active id; picking a chat in `chat-history` loads its messages via `chatHistory`.
- [ ] **Step 2:** `pnpm -C web test src/modules/chat` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command → PASS.
- [ ] **Step 5:** Commit `feat(web): chat panel`.

### Task 14: Agent panel tabs, outbox routing, shared session

**Files:**
- Create: `web/src/modules/chat/AgentPanel.vue`, `AgentPanel.test.ts`
- Modify: `web/src/app/ProjectPage.vue` (`#chat` slot renders `AgentPanel`), `web/src/modules/terminal/TerminalPanel.vue` (prop `active: boolean`; drain the outbox only when active; Start with no explicit session resumes the chat store's active chat when `loadSessions` lists it), `web/src/modules/terminal/TerminalPanel.test.ts`

**Interfaces:**
- Consumes: `ChatPanel` (`insert`, `send`, `open-terminal`), `TerminalPanel`, `useAgentOutboxStore`, `useChatStore`.
- Produces: `AgentPanel` props `{ projectId: string }`. Tabs `agent-tab-chat` (default) / `agent-tab-terminal`, both panels kept mounted (`force-mount` + `v-show`). In Chat, outbox `isRequest` messages → `send(agentPrompt(m))`, element messages → `insert(agentPrompt(m))`.

- [ ] **Step 1: Failing tests:** Chat is the default tab; `open-terminal` from ChatPanel switches to Terminal; an outbox element message lands in the chat input (not sent) when Chat is active, and is left for the terminal when Terminal is active; a `submit: true` message is sent in Chat; TerminalPanel with `active=false` does not drain; TerminalPanel Start resumes the session whose id equals the active chat id.
- [ ] **Step 2:** `pnpm -C web test src/modules/chat src/modules/terminal src/app` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `pnpm -C web test && pnpm -C web build` → PASS.
- [ ] **Step 5:** Commit `feat(web): Chat and Terminal tabs share the agent conversation`.

### Task 15: End-to-end

**Files:** Modify `e2e/fixtures/daemon.ts` (agents.json: `cursor: ["sh", fake_headless, "<repo>/tests/fixtures/chat/cursor/turn.jsonl", "edit"]`); create `e2e/specs/chat.spec.ts`.

- [ ] **Step 1: Write the spec** `chat sends a message, shows the reply and a checkpoint`: start a change, open the project, Chat tab is selected, choose Cursor CLI, type "hello", Send → the fixture's assistant text is visible, a tool line starting "Edited" is visible, a marker "Saved 1 file" is visible; then the History page lists a checkpoint whose summary is the fixture's assistant text (≤72 chars).
- [ ] **Step 2:** `just e2e` → the new spec PASSes and existing specs still pass.
- [ ] **Step 3:** Commit `test(e2e): chat panel turn`.
