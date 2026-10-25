# ACP chat + remove Terminal

Status: approved design, 2026-10-24.

## Intent

**Who and when:** Orso users talking to coding agents from the project Agent panel, and maintainers who currently juggle headless chat translators plus an in-app agent Terminal (PTY).

**Success looks like:**
- The daemon drives agents over the Agent Client Protocol (ACP): one long-lived process **per chat id**, so several agents can run in the same project at once.
- `/api/chat/*` hard-cuts over to **tabsh-shaped** `UiChunk` SSE (snake_case, `type`-tagged), porting the client/normalize ideas from tabsh’s 2026-10-22 ACP UI sessions work.
- The in-app **Terminal** (agent PTY/xterm and `/api/terminal/*`) is **removed** in the same effort.
- Billing/auth stay with each agent’s own subscription or CLI login.

**Out of scope (this effort):**
- Vue chat client rewrite for the new chunk shape (chat UI will break until a follow-up PR)
- Multiplayer / Zed-style collaboration
- Full permissions/policy editor (only in-thread approve/deny)
- Mid-session provider switch without an explicit restart
- Live cloud agent calls in CI

**Constraints:**
- Browser never speaks ACP; only the daemon does.
- Same auth/guards as other API routes.
- Prefer porting tabsh’s `src/chat/` ACP stack over a greenfield JSON-RPC client.
- CMS-specific post-turn behavior (checkpoints, content refresh, preview reload) stays, rewired to ACP turn end.
- Replaces the headless per-turn CLI + `translate/*` path from `2026-10-17-headless-chat-panel-design.md` and removes the agent Terminal surface from `2026-10-10-terminal-agents-design.md`.

## Decisions

| Question | Choice |
|---|---|
| UI wire format | Tabsh-shaped `UiChunk` (not AI SDK `UIMessageChunk`) |
| Cutover | Hard replace `/api/chat/*` (no parallel old stream) |
| Providers in v1 | Cursor, OpenCode, Claude, Codex — all via ACP |
| Process model | One long-lived ACP process **per chat id** (multi-agent) |
| Terminal | Remove from the app (API + UI) in this effort |
| Approach | Port tabsh ACP client/normalize/manager ideas into CMS |
| Chat Vue client | Later PR |

## 1. Architecture

```text
Browser (Vue chat — later PR)
    ↕ HTTP + SSE on /api/chat/*  (tabsh-shaped UiChunk)
Daemon chat/ (AcpClient + per-chat-id ChatManager + transcript)
    ↕ stdio JSON-RPC (ACP)
Agent (cursor-agent acp | opencode acp | claude-acp | codex-acp)
```

## 2. HTTP routes & lifecycle

### Routes (project-scoped)

| Method | Path | Role |
|---|---|---|
| GET | `/api/chat/{project}` | List chats |
| POST | `/api/chat/{project}` | Create chat (provider/model optional) |
| GET/DELETE | `/api/chat/{project}/{chat}` | History / delete |
| POST | `/api/chat/{project}/{chat}/start` | Ensure ACP process live |
| GET | `/api/chat/{project}/{chat}/stream` | SSE `UiChunk`s (+ replay) |
| POST | `/api/chat/{project}/{chat}/message` | User prompt (may auto-start) |
| POST | `/api/chat/{project}/{chat}/stop` | Cancel/interrupt current turn |
| POST | `/api/chat/{project}/{chat}/approve` | Permission allow/deny |
| GET | `/api/chat/{project}/{chat}/models` | Models from ACP config |
| POST | `/api/chat/{project}/{chat}/model` | Set model via ACP |
| POST/GET | `/api/chat/{project}/attachments`… | Moved off `/api/terminal` |

Provider listing that today lives under `/api/terminal/providers` moves to chat (or a small `/api/agents/providers` if clearer at implement time). Exact path pick is an implementation detail; Terminal URLs must not remain as the source of truth.

**Remove:** all `/api/terminal/*` and Terminal UI entry points.

### Lifecycle (per `chatId`)

1. Create chat → persist record; process optional until first message/`start`.
2. First need → spawn provider ACP command, `initialize` + `session/new` (or `loadSession` when a stored session id exists).
3. Prompt → normalize ACP events to `UiChunk`, broadcast on SSE, append transcript.
4. Turn end → CMS checkpoint / content / preview hooks when applicable.
5. Stop → ACP cancel; process may stay up for the next message.
6. Delete chat / project teardown → shut down **that** chat’s process only.
7. Daemon restart → resume via `loadSession` when possible; else fresh ACP session with prior transcript as history above it.

## 3. Daemon modules & removals

### Add / reshape (`src/chat/`)

- `acp.rs` — spawn, JSON-RPC, initialize / session / prompt / cancel / permissions (port from tabsh).
- `normalize.rs` — ACP events → tabsh-shaped `UiChunk`.
- `manager.rs` — `chatId` → live session; concurrency; start/stop/message.
- `models.rs` — ACP `configOptions` → list/set model.
- `transcript.rs` — history + resume; keep useful CMS storage patterns.
- HTTP handlers in `src/http/chat.rs` and/or `chat/routes.rs` — one clear owner.

### Default ACP commands

| Provider | Default command |
|---|---|
| Cursor | `cursor-agent acp` (or `agent acp`) |
| OpenCode | `opencode acp` |
| Claude | `npx @agentclientprotocol/claude-agent-acp` |
| Codex | `npx @agentclientprotocol/codex-acp` |

Overrides may live with existing provider settings when implemented.

### Delete / stop using

- `src/chat/translate/*` and per-turn `headless_spec` chat spawning
- AI SDK wire `UiChunk` in `chunk.rs` (replace with tabsh shape)
- `src/http/terminal.rs`, PTY/WS plumbing, Terminal web UI
- Chat↔terminal mutual exclusion (obsolete)

### Keep (rewired)

- Checkpoints / turn-end finish hooks
- Content + preview refresh on turn end
- Auth guards on every chat route

## 4. Errors

| Case | Behavior |
|---|---|
| ACP binary missing / spawn fail | Failed start; name provider + install hint |
| Auth / not logged in | Inline error; login via that agent’s CLI |
| Protocol / version reject | Incompatible-agent error; keep transcript |
| Process death mid-turn | Failed turn; retry/resume if `loadSession` allows |
| Permission request | SSE permission chunk; `approve` allow/deny |
| User stop | Cancel turn; composer/process ready for next prompt |
| Unknown chat / project | 404 |

## 5. Testing

- **Rust:** fake ACP agent over stdio — handshake, multi-chat concurrency in one project, prompt/stream/cancel, approve, auth-fail, process death
- **Normalize:** fixture ACP events → `UiChunk`
- **HTTP:** chat routes guarded; `/api/terminal/*` absent (404)
- **Web:** Terminal UI removed (compile + targeted tests); Agent/Chat panel may be stubbed or left knowingly broken until the client PR
- **Not in CI:** live calls to Cursor/Claude/Codex/OpenCode clouds

## 6. Rollout

1. Port ACP client + fake-agent tests into CMS
2. Per-`chatId` manager + transcript + new `/api/chat/*` semantics
3. Wire all four provider ACP commands
4. Delete Terminal (API + UI) and headless translator chat path
5. Follow-up PR: Vue client for tabsh-shaped chunks (+ restore Agent panel UX)

## 7. Open points for the plan (not blockers)

- SSE reconnect/replay buffer details (follow tabsh where practical)
- Transcript on-disk format migration from AI SDK messages vs fresh format
- Whether provider list is `/api/chat/.../providers` or `/api/agents/providers`
- Exact ACP crate vs hand-rolled client (match tabsh / current ACP at implement time)
- How aggressively to stub vs leave broken Vue chat during the backend PR series
