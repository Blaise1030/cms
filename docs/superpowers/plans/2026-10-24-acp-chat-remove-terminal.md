# ACP Chat + Remove Terminal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace CMS headless per-turn chat with long-lived ACP sessions (one process per chat id, all four providers) streaming tabsh-shaped `UiChunk`s, and delete the in-app agent Terminal (PTY + `/api/terminal/*`).

**Architecture:** Port tabsh’s `AcpClient` / normalize / transcript ideas into `src/chat/`. `ChatManager` keys live agents by `(project_id, chat_id)`. HTTP hard-cuts `/api/chat/*` to the new shape. Vue chat client is intentionally broken until a follow-up; this plan only keeps the web app compiling (API moves + Terminal references gone).

**Tech Stack:** Rust (axum, tokio process/stdio, serde_json), existing CMS `AppState` / auth, vitest for web compile fixes. Reference implementation: `/Users/blaise/Developer/tabsh/src/chat/`.

**Spec:** `docs/superpowers/specs/2026-10-24-acp-chat-remove-terminal-design.md`

## Global Constraints

- Browser never speaks ACP; only the daemon does.
- Wire format: tabsh-shaped `UiChunk` (`#[serde(tag = "type", rename_all = "snake_case")]`), plus CMS-only `checkpoint` (see Task 2).
- Hard cutover on `/api/chat/*`; no AI SDK SSE headers (`x-vercel-ai-ui-message-stream` removed).
- One long-lived ACP process per chat id; multiple chats in one project may run concurrently.
- Chat providers v1: `cursor`, `opencode`, `claude`, `codex` — each must have a non-empty ACP argv.
- Default ACP commands (verbatim): Cursor `cursor-agent acp` (fallback try `agent acp` only if documented at implement time as today in tabsh), OpenCode `opencode acp`, Claude `npx --yes @agentclientprotocol/claude-agent-acp`, Codex `npx --yes @agentclientprotocol/codex-acp`.
- Transcript path: `{state_dir}/{project}/chats/{chatId}.jsonl` — one `UiChunk` JSON per line (new format; do not read old AI SDK message lines).
- Chat id rule unchanged: `^[A-Za-z0-9_-]{1,64}$` else `400`.
- Auth: every chat route stays behind existing `require_auth` / `check_host`.
- No live cloud agent calls in CI — use tabsh-style fake ACP agent over stdio.
- Verification: `cargo test`, `pnpm -C web test`, `pnpm -C web build`.

## Review Focus

- Two chats in one project both `start` + `message` concurrently without sharing one process — Task 4.
- Path-traversal chat id (`../x`) never writes outside `chats/` — Task 3 + Task 5.
- Permission SSE chunk answered by `POST …/approve` unblocks the agent — Task 4 + Task 5.
- ACP binary missing surfaces `UiChunk::Error` (or HTTP error on start), not a hung stream — Task 4 + Task 5.
- After Terminal removal, no remaining `/api/terminal` route and web `api.ts` has no terminal client methods — Task 8.

## File map

| Path | Responsibility |
|---|---|
| `src/chat/acp.rs` | Port from tabsh: JSON-RPC ACP client + `fake_agent()` |
| `src/chat/normalize.rs` | ACP events → `UiChunk` (+ CMS `checkpoint`) |
| `src/chat/chunk.rs` | Replace AI SDK enum with tabsh-shaped `UiChunk` + `sse_frame` |
| `src/chat/transcript.rs` | Append/load `UiChunk` jsonl; seal open turns |
| `src/chat/models.rs` | ACP `configOptions` → model list/set (port tabsh) |
| `src/chat/manager.rs` | Per `(project, chat)` live ACP lifecycle (no PTY mutex) |
| `src/chat/translate/` | **Delete** |
| `src/http/chat.rs` | New route handlers matching spec table |
| `src/http/mod.rs` | Register chat routes; remove terminal routes |
| `src/http/terminal.rs` | **Delete** |
| `src/terminal/` | **Delete** (PTY manager) |
| `src/providers.rs` | Add `acp: Vec<String>` per provider; drop chat use of `headless_spec` |
| `src/state.rs` / `src/lib.rs` | Drop `terminals`; keep `chats: ChatManager` |
| `tests/terminal.rs` | **Delete** or replace with absence tests |
| `web/src/core/api.ts` | Providers/models/attachments under chat; remove terminal methods |
| `web/src/modules/chat/*` | Compile: stop calling terminal status/stop; chat transport may stub |

---

### Task 1: Port `AcpClient` + fake agent

**Files:**
- Create: `src/chat/acp.rs` (port from `tabsh/src/chat/acp.rs`, CMS module paths)
- Modify: `src/chat/mod.rs` — `pub mod acp;`
- Test: unit tests inside `acp.rs` (bring over tabsh fake-agent tests that don’t need the board)

**Interfaces:**
- Produces:
  - `pub struct AcpClient` with `spawn_with_env`, `initialize`, `session_new`, `prompt`, `cancel`, `answer_permission`, `event_stream`, `shutdown`, `set_config_option`, `mark_cancelled` / `clear_cancelled`
  - `pub enum AcpEvent` — same variants as tabsh
  - `pub fn fake_agent() -> (String, Vec<String>)` and `fake_agent_hold(gate: &Path) -> (String, Vec<String>)`

- [x] **Step 1: Write the failing test** `initialize_and_session_new_against_fake_agent` — spawn `fake_agent()`, `initialize` ok, `session_new` returns non-empty session id.

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -q initialize_and_session_new_against_fake_agent`
Expected: FAIL (module missing)

- [x] **Step 3: Port `acp.rs` from tabsh** (adapt `Result`/`BoxError` only as needed for CMS; do not pull board/PTY code)

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test -q acp::`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/chat/acp.rs src/chat/mod.rs
git commit -m "$(cat <<'EOF'
feat(chat): port ACP client and fake agent

EOF
)"
```

---

### Task 2: Tabsh-shaped `UiChunk` + normalize (+ CMS checkpoint)

**Files:**
- Replace: `src/chat/chunk.rs` — new `UiChunk` enum
- Create: `src/chat/normalize.rs` — `to_chunks(ev: &AcpEvent) -> Vec<UiChunk>`
- Modify: `src/chat/mod.rs`
- Delete later (Task 7): callers of old AI SDK variants

**Interfaces:**
- Produces `UiChunk` variants matching tabsh: `text_delta`, `thought_delta`, `user_text`, `tool_call`, `tool_call_update`, `plan`, `permission`, `turn_end`, `unknown`, `error` — plus CMS:
  - `checkpoint { sha?, summary?, files: string[], blocked?: bool }` (serde snake_case; fields mirror today’s `CheckpointData`)
- Produces: `pub fn sse_frame(chunk: &UiChunk) -> String` → `data: {json}\n\n`
- Produces: `pub fn to_chunks(ev: &AcpEvent) -> Vec<UiChunk>`

- [x] **Step 1: Write failing tests**
  - `agent_message_chunk_serializes_with_type_tag` — `TextDelta` → `{"type":"text_delta",...}`
  - `prompt_ended_becomes_turn_end`
  - `checkpoint_chunk_round_trips`

- [x] **Step 2: Run** `cargo test -q agent_message_chunk_serializes_with_type_tag prompt_ended_becomes_turn_end checkpoint_chunk_round_trips` — Expected: FAIL

- [x] **Step 3: Implement `chunk.rs` + `normalize.rs`** (port tabsh normalize; add `Checkpoint`)

- [x] **Step 4: Run** same tests — Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/chat/chunk.rs src/chat/normalize.rs src/chat/mod.rs
git commit -m "$(cat <<'EOF'
feat(chat): tabsh-shaped UiChunk and ACP normalize

EOF
)"
```

---

### Task 3: Transcript as `UiChunk` jsonl

**Files:**
- Rewrite: `src/chat/transcript.rs`
- Test: tests in `transcript.rs`

**Interfaces:**
- Produces:
  - `pub fn path(state_dir: &Path, project_id: &str, chat_id: &str) -> Option<PathBuf>` — `None` if chat id invalid
  - `pub fn append(..., chunk: &UiChunk) -> io::Result<()>`
  - `pub fn load(...) -> Vec<UiChunk>` — skips invalid lines; empty if missing/invalid id
  - Seal open turn with synthetic `turn_end` / `error` on load if last turn incomplete (match tabsh `seal_open_turn` behavior)

- [x] **Step 1: Write failing tests** `append_and_load_round_trip`, `reject_traversal_chat_id`, `seal_open_turn_on_load`

- [x] **Step 2: Run** `cargo test -q transcript::` — Expected: FAIL

- [x] **Step 3: Implement jsonl transcript**

- [x] **Step 4: Run** `cargo test -q transcript::` — Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/chat/transcript.rs
git commit -m "$(cat <<'EOF'
feat(chat): store ACP UiChunk transcripts as jsonl

EOF
)"
```

---

### Task 4: `ChatManager` — one ACP process per chat id

**Files:**
- Rewrite: `src/chat/manager.rs`
- Create: `src/chat/models.rs` (port `parse_model_config` / `ModelConfig` from tabsh)
- Modify: `src/state.rs` — `ChatManager::new` signature as needed (events + projects state dir accessor)

**Interfaces:**
- Produces `ChatManager` with (names may be `pub(crate)`):
  - `start(project_id, chat_id, cwd, acp_argv, env) -> Result<()>`
  - `ensure_started(...) -> Result<()>`
  - `send_message(project_id, chat_id, text) -> Result<()>`
  - `cancel(project_id, chat_id) -> Result<()>`
  - `answer(project_id, chat_id, request_id, allow) -> Result<()>`
  - `models(project_id, chat_id) -> Option<ModelConfig>`
  - `set_model(project_id, chat_id, model_id) -> Result<()>`
  - `subscribe(project_id, chat_id) -> Option<Subscription>` — replay + live `UiChunk`
  - `history(project_id, chat_id) -> Vec<UiChunk>`
  - `is_live(project_id, chat_id) -> bool`
  - `forget` / `shutdown`
- **No** PTY claim APIs.
- Map key: `(project_id, chat_id)` — two keys in one project are two processes.
- On `AcpEvent::PromptEnded` / turn end: append `turn_end` chunk; invoke checkpoint hook in Task 6 (manager may expose `on_turn_end` callback or call into `finish_turn` once HTTP state is available — prefer injecting `AppState` handle like today’s manager).

- [x] **Step 1: Write failing tests** (tokio + fake agent):
  - `two_chats_same_project_are_both_live`
  - `send_message_streams_text_delta_then_turn_end`
  - `cancel_ends_turn`
  - `missing_binary_notes_error_chunk`

- [x] **Step 2: Run** `cargo test -q chat::manager` — Expected: FAIL

- [x] **Step 3: Implement manager + models** (port tabsh lifecycle **without** `Surfaces`/PTY; key by project+chat; cwd = project dir)

- [x] **Step 4: Run** same tests — Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/chat/manager.rs src/chat/models.rs src/state.rs
git commit -m "$(cat <<'EOF'
feat(chat): per-chat-id ACP session manager

EOF
)"
```

---

### Task 5: Hard-cut `/api/chat/*` routes

**Files:**
- Rewrite: `src/http/chat.rs`
- Modify: `src/http/mod.rs` — route table per spec

**Interfaces:**
- Routes (all auth-guarded):

| Method | Path |
|---|---|
| GET/POST | `/api/chat/{project}` |
| GET/DELETE | `/api/chat/{project}/{chat}` |
| POST | `/api/chat/{project}/{chat}/start` |
| GET | `/api/chat/{project}/{chat}/stream` |
| POST | `/api/chat/{project}/{chat}/message` body `{ "text": string }` |
| POST | `/api/chat/{project}/{chat}/stop` |
| POST | `/api/chat/{project}/{chat}/approve` body `{ "requestId": string, "allow": bool }` (camelCase JSON ok if CMS convention; be consistent with existing API style) |
| GET | `/api/chat/{project}/{chat}/models` |
| POST | `/api/chat/{project}/{chat}/model` body `{ "modelId": string }` |

- Remove: `DELETE /api/chat/{project}/turn` (replaced by per-chat `stop`).
- `POST /api/chat/{project}` creates/lists as today but must persist provider + optional model for ACP argv lookup.
- Stream: SSE of `sse_frame(UiChunk)`; keep-alive ok; **no** `[DONE]` AI SDK trailer unless you also document it — prefer tabsh behavior (stream stays open for the chat’s life / ends on disconnect).
- List/history JSON: return `{ chunks: UiChunk[] }` or equivalent — pick one shape and use it in tests.

- [x] **Step 1: Write failing HTTP tests** (follow existing `tests/` or `http/chat` test style):
  - `chat_message_against_fake_acp_appends_transcript`
  - `chat_stream_replays_then_live`
  - `chat_stop_cancels_turn`
  - `chat_approve_permission`
  - `chat_id_traversal_rejected`

- [x] **Step 2: Run** those tests — Expected: FAIL

- [x] **Step 3: Implement handlers wiring `ChatManager` + provider ACP argv**

- [x] **Step 4: Run** — Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/http/chat.rs src/http/mod.rs
git commit -m "$(cat <<'EOF'
feat(chat): ACP HTTP routes and SSE stream

EOF
)"
```

---

### Task 6: Provider ACP argv + checkpoints on turn end

**Files:**
- Modify: `src/providers.rs` — `Provider { id, command, acp: Vec<String> }` (or `acp: Option<Vec<String>>` with defaults filled); `default_providers()` sets ACP defaults from Global Constraints; agents.json may override `acp` when present
- Modify: `src/chat/manager.rs` / `src/http/chat.rs` — on `turn_end` with `stop_reason == "end_turn"`, call existing `checkpoints::finish_turn` (and the same content/preview refresh side effects today’s headless manager used), append `UiChunk::Checkpoint { ... }`
- Remove chat call sites of `headless_spec` (delete headless helpers when unused after Task 7)

- [x] **Step 1: Write failing tests**
  - `default_providers_include_acp_argv_for_four_ids`
  - `turn_end_appends_checkpoint_chunk` (temp project dir + fake finish or stub)

- [x] **Step 2: Run** — Expected: FAIL

- [x] **Step 3: Implement ACP defaults + checkpoint emission**

- [x] **Step 4: Run** — Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/providers.rs src/chat/manager.rs src/http/chat.rs
git commit -m "$(cat <<'EOF'
feat(chat): ACP provider commands and checkpoint chunks

EOF
)"
```

---

### Task 7: Delete Terminal (daemon) + translators

**Files:**
- Delete: `src/http/terminal.rs`, `src/terminal/**`, `src/chat/translate/**`, `tests/terminal.rs` (if solely PTY)
- Modify: `src/http/mod.rs`, `src/lib.rs`, `src/state.rs`, any `terminals.` call sites (changes/get-latest busy checks, etc.) — remove or simplify to chat-only busy if still needed
- Modify: events that referenced `terminal.busy` / `terminal.turn` — either delete emitters or rename to `chat.*` consistently; update web listeners in Task 8
- Move attachment handlers from terminal into `src/http/chat.rs` (paths under `/api/chat/{project}/attachments`)

- [x] **Step 1: Write failing test** `terminal_routes_are_gone` — `GET /api/terminal/providers` → 404 (or whatever CMS uses for unknown routes)

- [x] **Step 2: Run** — Expected: FAIL (still 200)

- [x] **Step 3: Delete PTY stack + translators; fix compile; register attachment routes on chat**

- [x] **Step 4: Run** `cargo test -q` — Expected: PASS (fix or delete obsolete terminal tests)

- [x] **Step 5: Commit**

```bash
git add -A src/http src/terminal src/chat src/lib.rs src/state.rs tests
git commit -m "$(cat <<'EOF'
refactor: remove agent Terminal and headless translators

EOF
)"
```

---

### Task 8: Web compile — API move + disable broken chat transport

**Files:**
- Modify: `web/src/core/api.ts` — providers/models/attachments → chat paths; delete terminal start/status/stop/ws helpers
- Modify: `web/src/modules/chat/providers.ts` (+ tests)
- Modify: `web/src/modules/chat/ChatPanel.vue` / tests — remove terminal busy/stop coupling; stub or no-op send/stream so the panel mounts without AI SDK posting to old body shape (empty state + “Chat reconnecting in a follow-up” is fine)
- Modify: `web/src/core/types.ts`, `socket.ts`, `fakeDaemon.ts`, `ChatMessage` attachment URLs
- Modify: any `terminal.turn` / `terminal.busy` listeners (e.g. `ProjectPage.vue`) → `chat.turn` if still emitted

- [x] **Step 1: Write failing tests** updating `providers.test.ts` / `api` expectations for new paths; ChatPanel test that it no longer calls `/api/terminal/*`

- [x] **Step 2: Run** `pnpm -C web test` — Expected: FAIL

- [x] **Step 3: Implement API moves + ChatPanel stub**

- [x] **Step 4: Run** `pnpm -C web test && pnpm -C web build` — Expected: PASS

- [x] **Step 5: Commit**

```bash
git add web/src
git commit -m "$(cat <<'EOF'
fix(web): drop Terminal client; stub chat until ACP UI

EOF
)"
```

---

### Task 9: Full verification

- [x] **Step 1: Run** `cargo test`

- [x] **Step 2: Run** `pnpm -C web test && pnpm -C web build`

- [x] **Step 3: Manual smoke (optional, not CI):** one real provider ACP if available on the machine

- [x] **Step 4: Commit any fixes**

```bash
git commit -m "$(cat <<'EOF'
test: green suite after ACP chat cutover

EOF
)"
```

---

## Self-review notes (author)

| Spec requirement | Task |
|---|---|
| Tabsh-shaped UiChunk hard cutover | 2, 5 |
| Per chat id / multi-agent | 4 |
| All four providers ACP | 6 |
| Remove Terminal same effort | 7, 8 |
| Checkpoints retained | 2, 6 |
| Approve/stop/models | 4, 5 |
| Vue client later | 8 stub only |
| Fake agent CI | 1, 4, 5 |
