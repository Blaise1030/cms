# Terminal Agents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hand-rolled ACP client and chat UI with the real coding CLIs running in a daemon-owned PTY, shown in an xterm.js panel, with checkpoints driven by each CLI's turn-end hook.

**Architecture:** `portable-pty` spawns `$SHELL -lc 'exec <cli>'` per project inside `TerminalManager`; bytes flow over `GET /api/terminal/{project}/ws` (binary frames + JSON control frames, tabsh framing) to xterm in `modules/terminal`. Each provider's launch argv carries a hook that runs `<exe> hook turn-end`, which POSTs to `/api/terminal/turn-end`; the daemon validates a per-session token, runs `require_change`, and calls the existing `git::checkpoint`.

**Tech Stack:** Rust (axum 0.8 `ws`, tokio, `portable-pty` 0.9, libc), Vue 3 + Pinia, `@xterm/xterm` 5.5 + `@xterm/addon-fit` 0.10, vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-10-10-terminal-agents-design.md`
**Prior art:** `~/Developer/ide` (tabsh). Port from it where cited; read the cited file before writing the equivalent.

## Global Constraints

- At most one live terminal per project.
- PTY initial size 80×24; reader chunk 8 KB; scrollback ring 512 KB; broadcast capacity 1024.
- Child env: `TERM=xterm-256color`, `WORKBENCH_URL=http://127.0.0.1:{config.port}`, `WORKBENCH_PROJECT`, `WORKBENCH_HOOK_TOKEN` (fresh random per session, 32 hex chars), `WORKBENCH_HOOK_EXE` (absolute path of the hook binary).
- Kill = `SIGTERM` to the child's process group, `SIGKILL` after 2 s, then reap.
- Never modify user-global CLI config (`~/.claude`, `~/.codex`, `~/.config/opencode`).
- Hook subcommand always exits 0; it must never break the CLI's turn.
- Checkpoint message: one line, ≤ 72 chars (char-boundary safe), else `Update <n> files: <paths>`.
- WS framing — binary: PTY bytes both ways. Text client→server `{"cols":N,"rows":N}`. Text server→client `{"exit":true,"code":N|null}`. First server frame = replay.
- Commit style `feat(terminal): …` / `chore: …`, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Verification: `just test` green at the end of every task.

## Review Focus

1. **Multibyte UTF-8 split across 8 KB reads** — emoji/CJK in CLI output render intact. Bytes are forwarded raw and written to xterm as `Uint8Array`, never decoded to a JS string per frame. → Task 5 test `writes binary frames as Uint8Array`.
2. **Two browser tabs on one project** — both see live output and both can type; the latest resize wins. → Task 4 test `two_clients_both_receive_output`.
3. **Turn ends with no file changes** (Claude `Stop` after answering a question) — no commit, no `checkpoint.created`, response 200. → Task 4 test `turn_end_without_changes_makes_no_commit`.
4. **Hostile summaries** — newlines, quotes, 500-char prompts, non-ASCII at the 72-char boundary produce a valid one-line message without panicking. → Task 4 unit test `commit_message_is_one_line_and_char_safe`.
5. **Paths and argv needing shell quoting** — a project dir with spaces, an override argv element containing `'` and spaces, reach the CLI verbatim. → Task 3 test `argv_with_spaces_and_quotes_reaches_child`.

---

### Task 1: Provider registry with hook injection

**Files:**
- Create: `src/providers.rs` (inline `#[cfg(test)] mod tests`)
- Modify: `src/lib.rs` (add `pub mod providers;` — keep `agents_config` until Task 7)

**Interfaces:**
- Produces:
  - `pub struct Provider { pub id: String, pub command: Vec<String> }` (Serialize; the base argv)
  - `pub fn load(path: Option<&Path>) -> Result<Vec<Provider>, ApiError>` — same file format, missing-file and invalid-file rules as `agents_config::AgentsConfig::load` (move its parse logic and tests here). Defaults: `claude` → `["claude"]`, `codex` → `["codex"]`, `opencode` → `["opencode"]`.
  - `pub struct LaunchSpec { pub argv: Vec<String>, pub env: Vec<(String, String)> }`
  - `pub fn launch_spec(provider: &Provider, hook_exe: &Path, auto_approve: bool, state_dir: &Path) -> LaunchSpec`

Injection by `provider.id` (appended after the base argv; unknown ids get nothing appended — they call `$WORKBENCH_HOOK_EXE` themselves):

| id | appended argv | env | auto_approve |
|----|---------------|-----|--------------|
| `claude` | `--settings`, `{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"<sh-quoted exe> hook turn-end"}]}]}}` | — | `--dangerously-skip-permissions` |
| `codex` | `-c`, `notify=["<exe>","hook","turn-end"]` (exe as a JSON string literal) | — | `--dangerously-bypass-approvals-and-sandbox` |
| `opencode` | — | config-dir env var → `{state_dir}/opencode` (Step 1) | permission allow via that config |

- [ ] **Step 1: Verify OpenCode and Codex hook mechanics against installed versions** (opencode v2.0.25, codex-cli 0.162.1)

Read `~/Developer/ide/src/cli/setup.md` (OpenCode 2.x plugin). Confirm: (a) the env var that points OpenCode at an extra config dir (expected `OPENCODE_CONFIG_DIR`) and that `plugins/*.ts` in it load; (b) the idle event name (tabsh uses `session.execution.*`); (c) the config key allowing all permissions; (d) that `codex -c 'notify=[...]'` fires without `--dangerously-bypass-hook-trust` (run `codex exec` with a notify pointing at a script that touches a file). Record the answers as a doc comment at the top of `providers.rs`. If (a) or (b) fails, `opencode` launches with no hook and `launch_spec` logs `workbench: opencode turn-end hook unavailable; checkpoints disabled`. If (d) fails, also append `--dangerously-bypass-hook-trust` for codex.

- [ ] **Step 2: Write the failing tests**

```rust
#[test] fn claude_gets_stop_hook_settings() // argv == ["claude","--settings",<json>]; parse json; hooks.Stop[0].hooks[0].command ends with " hook turn-end" and starts with the quoted exe
#[test] fn claude_auto_approve_appends_skip_permissions()
#[test] fn codex_gets_notify_override() // argv[1]=="-c", argv[2]==r#"notify=["/x/work bench","hook","turn-end"]"# for exe "/x/work bench"
#[test] fn codex_auto_approve_appends_bypass()
#[test] fn opencode_writes_plugin_and_sets_config_env() // plugin file exists under state_dir/opencode/plugins, env contains the Step 1 var
#[test] fn unknown_provider_gets_base_argv_only()
#[test] fn override_replaces_base_argv_but_keeps_hook() // file {"claude":["npx","claude"]} → argv starts ["npx","claude","--settings",..]
// + the parse tests moved from agents_config.rs
```

- [ ] **Step 3: Run** `cargo test --lib providers` — Expected: FAIL (module missing).
- [ ] **Step 4: Implement** `src/providers.rs`. `opencode` writes the plugin file (contents from Step 1, calling `$WORKBENCH_HOOK_EXE hook turn-end`) on every `launch_spec` call, creating dirs.
- [ ] **Step 5: Run** `cargo test --lib providers` — Expected: PASS.
- [ ] **Step 6: Commit** `feat(terminal): provider registry with per-launch turn-end hooks`

---

### Task 2: `workbench hook turn-end` subcommand

**Files:**
- Create: `src/hook.rs` (inline tests)
- Modify: `src/main.rs` (add hidden `Hook { #[command(subcommand)] event: HookEvent }` with `TurnEnd { payload: Option<String> }`; `main` matches `Command`), `src/lib.rs` (`pub mod hook;`)

**Interfaces:**
- Produces:
  - `pub fn summary_from_payload(payload: &str) -> Option<String>` — JSON object; first non-empty of: last string in `input-messages` (Codex), `summary` (OpenCode plugin), `last_assistant_message` (Claude). Non-JSON / empty → `None`.
  - `pub async fn turn_end(payload_arg: Option<String>) -> ExitCode` — payload = `payload_arg` if given, else stdin (read to end, skip if stdin is a TTY). Reads `WORKBENCH_URL`, `WORKBENCH_PROJECT`, `WORKBENCH_HOOK_TOKEN`; any missing → return `ExitCode::SUCCESS` silently. POSTs `{project, token, summary}` to `{url}/api/terminal/turn-end` with `reqwest`, 10 s timeout. Errors/non-2xx → one `workbench hook: …` line on stderr. Always `ExitCode::SUCCESS`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn codex_payload_uses_last_input_message() // {"type":"agent-turn-complete","input-messages":["a","b"]} → Some("b")
#[test] fn claude_payload_uses_last_assistant_message()
#[test] fn opencode_payload_uses_summary()
#[test] fn garbage_payload_is_none() // "not json", "", "[]"
#[tokio::test] async fn missing_env_exits_zero_without_request() // env cleared; returns SUCCESS
#[tokio::test] async fn posts_to_daemon() // bind a tiny axum server on 127.0.0.1:0 recording the JSON body; set env; assert body == {project, token, summary:"b"}
```

Env-mutating tests run serially (a `static ENV_LOCK: Mutex<()>`).

- [ ] **Step 2: Run** `cargo test --lib hook` — Expected: FAIL.
- [ ] **Step 3: Implement** `src/hook.rs` and the `main.rs` subcommand (`#[command(hide = true)]`).
- [ ] **Step 4: Run** `cargo test --lib hook && cargo run -- hook turn-end '{}'` — Expected: PASS; the run exits 0 silently.
- [ ] **Step 5: Commit** `feat(terminal): hook turn-end subcommand`

---

### Task 3: `TerminalManager` (PTY lifecycle, replay, kill)

**Files:**
- Modify: `Cargo.toml` (`portable-pty = "0.9.0"`)
- Create: `src/terminal/mod.rs`, `src/terminal/modes.rs` (port `~/Developer/ide/src/sessions/modes.rs` and its tests)
- Modify: `src/config.rs` (add `pub hook_exe: Option<PathBuf>`; `None` → `std::env::current_exe()`), `src/lib.rs` (`pub mod terminal;`)
- Test: `tests/terminal.rs`, `tests/fixtures/fake_cli.sh`

**Interfaces:**
- Consumes: `providers::LaunchSpec` (Task 1).
- Produces:
  - `pub enum Output { Data(Arc<[u8]>), Exit(Option<i32>) }` (Clone)
  - `pub struct Attach { pub replay: Vec<u8>, pub rx: broadcast::Receiver<Output> }`
  - `impl TerminalManager`:
    - `pub fn new(config: Arc<Config>, events: EventBus) -> Self`
    - `pub async fn start(&self, project_id: &str, provider: &str, cwd: &Path, launch: LaunchSpec) -> Result<(), ApiError>` — `conflict` if live
    - `pub fn status(&self, project_id: &str) -> Option<String>` (provider id)
    - `pub fn attach(&self, project_id: &str) -> Result<Attach, ApiError>` — `enoent` if none; replay = modes prefix + ring, taken under the same lock as subscription
    - `pub fn write(&self, project_id: &str, bytes: Vec<u8>) -> Result<(), ApiError>`
    - `pub fn resize(&self, project_id: &str, cols: u16, rows: u16) -> Result<(), ApiError>`
    - `pub fn token_matches(&self, project_id: &str, token: &str) -> bool` (constant-time compare; reuse the helper in `src/auth.rs`, making it `pub(crate)`)
    - `pub async fn kill(&self, project_id: &str)` (no-op if none)
    - `pub async fn shutdown(&self)` (kill all)

Spawn: `$SHELL` (fallback: `getpwuid` shell, then `/bin/sh`) with args `-lc`, `exec <each argv element sh-single-quoted, space-joined>`. Env = Global Constraints vars + `launch.env`. Threads follow `~/Developer/ide/src/sessions/pty.rs:308-413`: writer thread on an mpsc channel, reader thread (8 KB) appending to the ring + modes tracker and broadcasting under one lock, waiter thread on `child.wait()` that broadcasts `Output::Exit(code)`, removes the session, and emits `terminal.exited {projectId, code}`. `portable-pty` makes the child a session leader, so `libc::killpg(pid, SIGTERM)`, wait up to 2 s, then `killpg(pid, SIGKILL)`.

`tests/fixtures/fake_cli.sh` (executable): prints `FAKE-CLI READY`, then loops reading lines from stdin: `size` → prints `stty size`; `argv` → prints each `$@` on its own line; `spawn-child` → starts `sleep 300 &` and prints `CHILD <pid>`; `edit <text>` → appends `<text>` to `notes.md` then runs `"$WORKBENCH_HOOK_EXE" hook turn-end "{\"summary\":\"<text>\"}"` and prints `TURN-END`; anything else → echoes `ECHO <line>`.

- [ ] **Step 1: Write the failing tests** (`tests/terminal.rs`, manager-level; helper `read_until(rx, needle, timeout 10s) -> String`)

```rust
#[tokio::test] async fn spawns_in_cwd_and_streams_output()        // attach rx sees "FAKE-CLI READY"
#[tokio::test] async fn replay_contains_earlier_output()          // after READY, a fresh attach().replay contains "FAKE-CLI READY"
#[tokio::test] async fn write_reaches_stdin()                     // write "hello\r" → "ECHO hello"
#[tokio::test] async fn resize_reaches_pty()                      // resize(120,40); write "size\r" → "40 120"
#[tokio::test] async fn second_start_conflicts()
#[tokio::test] async fn exit_is_broadcast_and_session_removed()   // argv ["sh","-c","exit 3"]: rx gets Exit(Some(3)); status()==None; event terminal.exited code 3
#[tokio::test] async fn kill_reaps_process_group()                // "spawn-child" → pid; kill(); within 3s `kill(pid,0)` fails with ESRCH
#[tokio::test] async fn argv_with_spaces_and_quotes_reaches_child() // cwd "/tmp/x/with space", argv [fake, "it's a test"]; "argv" prints `it's a test`
#[tokio::test] async fn missing_command_exits_127()               // argv ["definitely-not-a-cli"] → Exit(Some(127))
```

- [ ] **Step 2: Run** `cargo test --test terminal` — Expected: FAIL (module missing).
- [ ] **Step 3: Port `modes.rs`** with its tests; run `cargo test --lib terminal::modes` — Expected: PASS.
- [ ] **Step 4: Implement** `src/terminal/mod.rs` and the `Config::hook_exe` field (update `Default` and test configs).
- [ ] **Step 5: Run** `cargo test --test terminal` — Expected: PASS.
- [ ] **Step 6: Commit** `feat(terminal): PTY manager with replay and process-group kill`

---

### Task 4: HTTP/WS routes and turn-end checkpoints

**Files:**
- Create: `src/http/terminal.rs`
- Modify: `src/http/mod.rs` (routes), `src/state.rs` (`pub terminals: Arc<TerminalManager>`; keep `agents` until Task 7), `src/lib.rs` (`serve_on` shuts down terminals), `src/http/projects.rs:156` (kill terminal on project delete), `src/auth.rs` (`require_auth` lets `POST /api/terminal/turn-end` through — it authenticates by hook token), `src/main.rs` (no change beyond Task 2)
- Test: `tests/terminal.rs` (HTTP section)

**Interfaces:**
- Consumes: Tasks 1–3; `checkpoints::require_change(&AppState, &str)`; `git::checkpoint(&Path, &str) -> Result<Option<Checkpoint>, ApiError>`; `EventBus::emit(kind, fields)`.
- Produces routes:
  - `GET /api/terminal/providers` → `{ providers: [{id, command}] }`
  - `POST /api/terminal` `{project, provider}` → `{running: true}`. Order: project exists (`enoent`) → `require_change` → provider known (`badreq`) → `launch_spec(.., project.auto_approve, {projects_dir}/.workbench)` → `terminals.start`.
  - `GET /api/terminal/{project}` → `{running, provider}`
  - `DELETE /api/terminal/{project}` → `{stopped: true}`
  - `GET /api/terminal/{project}/ws` — upgrade; send `Attach.replay` as one binary frame, then forward `Output::Data` as binary and `Output::Exit(c)` as `{"exit":true,"code":c}` then close. Incoming binary → `write`; text `{cols,rows}` → `resize`. `RecvError::Lagged` → close the socket. Never hold a manager lock while sending.
  - `POST /api/terminal/turn-end` `{project, token, summary?}`: bad token → `403` (`ApiError::unauth(..).with_status(403)`); `require_change` fails → emit `checkpoint.blocked {projectId, reason}`, `200 {committed:false}`; else `git::checkpoint(dir, &commit_message(summary, &changed))` → on `Some(cp)` emit `checkpoint.created` with the same fields the ACP path emits (`src/acp.rs:1019-1030`), `200 {committed:true}`; `None` → `200 {committed:false}`. Serialized per project with a `Mutex` map in the handler module.
  - `pub(crate) fn commit_message(summary: Option<&str>, changed: &[String]) -> String` — first line of trimmed summary, truncated to 72 chars on a char boundary (append `…` when truncated, within the 72); empty/None → `Update {n} files: {first 3 paths joined ", "}`. `changed` comes from `git status --porcelain` (add `git::changed_paths(dir) -> Result<Vec<String>, ApiError>` if `git.rs` has nothing equivalent).

- [ ] **Step 1: Write the failing tests** (real listener: bind `127.0.0.1:0`, build `Config { port, hook_exe: Some(env!("CARGO_BIN_EXE_workbench").into()), token: Some(..), agents_config: Some(<file with {"fake":[<fake_cli.sh>]}>) }`, `serve_on` in a task; WS client = `tokio-tungstenite`)

```rust
#[tokio::test] async fn start_refused_on_main_with_remote()      // same error code as tests/change_guard.rs expects for prompt
#[tokio::test] async fn ws_first_frame_is_replay()
#[tokio::test] async fn ws_resize_and_input()                    // send {"cols":100,"rows":30}, binary "size\r" → "30 100"
#[tokio::test] async fn two_clients_both_receive_output()
#[tokio::test] async fn edit_turn_creates_checkpoint()           // binary "edit hello\r" → event checkpoint.created; git log -1 subject == "hello"
#[tokio::test] async fn turn_end_without_changes_makes_no_commit()
#[tokio::test] async fn turn_end_bad_token_is_403()
#[tokio::test] async fn turn_end_blocked_on_main_emits_event()
#[tokio::test] async fn delete_project_kills_terminal()
#[test] fn commit_message_is_one_line_and_char_safe()           // "a\nb" → "a"; 500×"é" → ≤72 chars, valid; "" with ["x","y"] → "Update 2 files: x, y"
```

- [ ] **Step 2: Run** `cargo test --test terminal` — Expected: new tests FAIL.
- [ ] **Step 3: Implement** the handler module, wiring and auth exemption.
- [ ] **Step 4: Run** `cargo test` — Expected: PASS (ACP tests still pass; both systems coexist).
- [ ] **Step 5: Commit** `feat(terminal): terminal routes, websocket bridge and hook checkpoints`

---

### Task 5: Web terminal panel

**Files:**
- Modify: `web/package.json` (`@xterm/xterm@5.5.0`, `@xterm/addon-fit@0.10.0`), `web/src/core/api.ts` (terminal calls), `web/src/core/types.ts` (add `terminal.exited`, `checkpoint.blocked` events)
- Create: `web/src/modules/terminal/{socket.ts,paste.ts,store.ts,TerminalPanel.vue}` + `socket.test.ts`, `paste.test.ts`, `TerminalPanel.test.ts`
- Modify: `web/src/app/ProjectPage.vue` (replace `ChatPanel` in the `#chat` slot with `<TerminalPanel :key :project-id :auto-approve @toggle-auto-approve>`), `web/src/modules/preview/PreviewPanel.vue:309` (tooltip → `"Select an element to mention in the terminal"`)

**Interfaces:**
- Consumes: Task 4 routes; `buildEventsUrl` pattern in `web/src/core/socket.ts:9` (same origin + `?t=` token handling); `elementPrompt(pick)` from `web/src/modules/preview/elementPicker.ts:92`; preview store `picked` + its take/consume function (`web/src/modules/preview/store.ts:46`).
- Produces:
  - api: `terminalProviders(): Promise<{id:string;command:string[]}[]>`, `startTerminal(project, provider)`, `stopTerminal(project)`, `terminalStatus(project): Promise<{running:boolean;provider?:string}>`
  - `buildTerminalUrl(config: DaemonConfig, projectId: string): string` (in `socket.ts`)
  - `bracketedPaste(text: string): string` → `"\x1b[200~" + text + "\x1b[201~"` (in `paste.ts`; strip any `\x1b[201~` inside `text` first)
  - `connectTerminal(opts: { url; onReplay(b: Uint8Array); onData(b: Uint8Array); onExit(code: number|null); onClose(); createSocket?: SocketFactory }): { send(b: Uint8Array|string): void; resize(cols, rows): void; close(): void }` — `binaryType = "arraybuffer"`; the first binary frame goes to `onReplay`, later ones to `onData`.

`TerminalPanel.vue` (`data-testid="terminal-panel"`): idle → provider select (from `terminalProviders`, remembered in `localStorage` key `workbench.terminal.provider`, wrapped in try/catch), auto-approve toggle (emits as ChatPanel did; applies on next start), Start button; start errors shown inline (the change-required message included). Running → xterm + fit addon; await `document.fonts.ready` before `open`; ResizeObserver → `fit()` then `resize`. `onReplay`: set `muted = true`, `term.write(bytes, () => muted = false)`; `term.onData` sends only when `!muted`. Live data batched with `requestAnimationFrame`. `onExit` → overlay `Session ended (code N)` (127 → `` `<command[0]>` not found on PATH ``) with Restart (= Start again). `onClose` without exit → reconnect after 500 ms, 1 s, 2 s, … capped at 10 s. A watch on preview `picked` takes the pick and sends `bracketedPaste(elementPrompt(pick))` then focuses the terminal.

- [ ] **Step 1: Write the failing tests**

```ts
// paste.test.ts
it("wraps text in bracketed paste markers")
it("strips an embedded end marker")
// socket.test.ts (fake SocketLike)
it("routes the first binary frame to onReplay and later ones to onData")
it("writes binary frames as Uint8Array")          // onData receives Uint8Array, bytes for "é" split across two frames arrive unmodified
it("sends resize as JSON text")                   // resize(100,30) → '{"cols":100,"rows":30}'
it("calls onExit for an exit control frame")
it("buildTerminalUrl uses ws scheme, path and token")
// TerminalPanel.test.ts (mock api + connectTerminal, stub xterm)
it("shows provider picker and starts the chosen provider")
it("shows the change-required error from start")
it("ignores terminal input while replay is being written")
it("pastes a picked element into the terminal")
it("shows not-found overlay on exit code 127")
```

- [ ] **Step 2: Run** `pnpm -C web test -- terminal` — Expected: FAIL.
- [ ] **Step 3: Implement** the module and the `ProjectPage` swap. Import `@xterm/xterm/css/xterm.css` in the panel; theme from the app's CSS tokens.
- [ ] **Step 4: Run** `pnpm -C web test && pnpm -C web build` — Expected: PASS. Then `just dev`, open a project with a change, start `claude`, type a prompt, see a checkpoint appear (manual check; note the result in the PR).
- [ ] **Step 5: Commit** `feat(terminal): xterm terminal panel replaces chat`

---

### Task 6: Move e2e onto the terminal

**Files:**
- Modify: `e2e/fixtures/daemon.ts` (agents.json → `{"fake": [<repo>/tests/fixtures/fake_cli.sh]}`; drop the `fake_acp_agent` lookup; replace `agentCheckpoint(env, text)` with `terminalCheckpoint(env, text)`), `e2e/specs/changes-start.spec.ts`, `changes-switch.spec.ts`, `share-preview.spec.ts` (use `terminalCheckpoint`), `changes-main-guard.spec.ts:28,85` (`chat-panel` → `terminal-panel`), `justfile` (stop building `fake_acp_agent` for e2e)
- Create: `e2e/specs/terminal.spec.ts`

**Interfaces:**
- Produces: `terminalCheckpoint(env, text): Promise<void>` — `POST /api/terminal {project, provider:"fake"}` (ignore `conflict`), open `buildTerminalUrl`-equivalent WS with Node's global `WebSocket`, wait for `FAKE-CLI READY` in the replay/data, send `edit <text>\r`, wait for a new commit via `GET /api/checkpoints` (same polling as today), close.

- [ ] **Step 1: Write** `terminal.spec.ts`: `test("terminal shows the CLI and checkpoints a turn")` — open the project with a change, pick `fake`, Start, expect `FAKE-CLI READY` in `.xterm-rows`, type `edit hello` + Enter, expect a checkpoint `hello` in the checkpoints UI.
- [ ] **Step 2: Run** `just e2e` — Expected: the new spec FAILS before the fixture changes, other specs still pass on the ACP fixture.
- [ ] **Step 3: Implement** the fixture swap and spec updates.
- [ ] **Step 4: Run** `just e2e` — Expected: all specs PASS.
- [ ] **Step 5: Commit** `test(e2e): drive checkpoints through the terminal`

---

### Task 7: Delete ACP and chat

**Files:**
- Delete: `src/acp.rs`, `src/agents_config.rs`, `src/http/agent.rs`, `tests/acp.rs`, `examples/fake_acp_agent.rs`, `web/src/modules/chat/` (whole dir, including `elementRef.ts` — a tiptap node only the composer used; the picker reuses `elementPrompt` instead, correcting the spec's "moves" note)
- Modify: `src/lib.rs`, `src/state.rs`, `src/http/mod.rs` (drop `/api/agents`, `/api/agent/*`), `src/http/projects.rs:157`, `src/http/checkpoints.rs` doc comment, `web/src/app/HomePage.vue` (drop `useChatStore` / `@deleted="chat.forget"` — instead the terminal store forgets the project), `web/src/core/types.ts` + `socket.ts` (remove `chat.*` and `permission.request` events), `web/src/modules/preview/elementPicker.ts` + tests (remove `parseElementRefs`/`MessagePart` if now unused), `README.md` (agents section → terminal), `Cargo.toml`/`web/package.json` (drop deps only chat/ACP used — check tiptap)

- [ ] **Step 1: Delete and fix compile errors**; `rg -n "acp|agents_config|AgentManager|modules/chat|chat\.(chunk|tool|plan|done|error)|permission\.request|history\.jsonl" src web/src e2e tests README.md` — Expected: no matches except the `--agents-config` flag/field and this plan/spec.
- [ ] **Step 2: Run** `just test && just e2e` — Expected: PASS.
- [ ] **Step 3: Commit** `chore: remove ACP client and chat UI`
