# Agent Skill Pack and Control CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every Workbench agent a shared Workbench-level skill layer (plain language, greenlight rules, verification habits) and a `workbench ctl` CLI for seeing app state it cannot see from files.

**Architecture:** Skill text lives in `src/providers.rs` as `WORKBENCH_GUIDE`, combined with the existing `GIT_GUIDE` via `agent_guide()` and injected through the existing per-provider channels (Claude system prompt, Codex developer instructions, Cursor rule file; interactive + headless). The CLI is a new `workbench ctl` subcommand in the same binary: it reads `WORKBENCH_URL` / `WORKBENCH_PROJECT` / `WORKBENCH_HOOK_TOKEN` (already set in agent terminals) and POSTs to new hook-token-authenticated `/api/ctl/*` daemon endpoints, printing JSON. No MCP server in this plan.

**Tech Stack:** Rust (clap, axum, reqwest, serde_json), existing `AppState`, `changes`, `git`, `devserver` modules.

**Spec:** `docs/superpowers/specs/2026-10-18-creator-publishes-owner-greenlight-design.md` (greenlight vocabulary and human-only publish actions), plus the 2026-10-18 conversation decision: CLI first, MCP later as a thin wrapper.

## Global Constraints

- Agent-facing words only: change, greenlight, ready to publish, publish, live. Never branch/commit/PR/merge in agent-to-user text.
- Publish, share, greenlight, and merge are human clicks in the app. No ctl command performs them; ctl is read-only except `restart-preview`.
- ctl authenticates exactly like `hook turn-end`: hook token in the JSON body, validated by `TerminalManager::token_matches`; `/api/ctl/` is exempt from daemon-token auth like `/api/terminal/turn-end`.
- No new dependencies. No force-push, rebase, or destructive git in any new code path.
- Screenshot and in-page error reporting are out of scope: the daemon cannot see inside the preview iframe; a headless-browser tool is a separate project.

## Review Focus

- Agent outside a Workbench terminal (env vars missing): ctl must fail with one plain sentence, exit non-zero, and make no request.
- Stale/forged hook token: daemon answers 403 with no project data leaked.
- Preview `crashed`: `preview-status` must say `crashed` (not hang or claim ready) so the agent can offer `restart-preview`.
- Change on `main` (no active change): `change-status` must report `active: null`, not error.
- Edits made after a greenlight: guide text must tell the agent the greenlight no longer covers them (spec step 9).

---

### Task 1: ctl daemon endpoints

**Files:**
- Create: `src/http/ctl.rs`
- Modify: `src/http/mod.rs` (register `pub mod ctl` + 3 routes)
- Modify: `src/auth.rs` (exempt `/api/ctl/` prefix alongside `TURN_END_PATH`)
- Test: `tests/ctl.rs`

**Interfaces:**
- Consumes: `AppState.projects.get`, `AppState.devservers.start`, `AppState.terminals.token_matches`, `changes::active`, `changes::list`, `git::changed_paths`, `ProjectInfo`.
- Produces: `POST /api/ctl/preview-status`, `POST /api/ctl/restart-preview`, `POST /api/ctl/change-status`, each `{project, token}` → JSON; `pub async fn preview_status_value(&AppState, &str) -> Result<Value, ApiError>` and `change_status_value` for direct testing.

- [ ] **Step 1: Write failing tests** in `tests/ctl.rs`: oneshot POST without a live terminal → `403`; `preview_status_value` on a scaffolded project → `{devserver: "starting", port, previewPath}` shape; `change_status_value` on a seeded repo on `main` → `active: null`, `dirty: false`; after creating a change and editing a file → active title + `dirty: true`.
- [ ] **Step 2: Run** `cargo test --test ctl` — expect compile failure (`http::ctl` missing).
- [ ] **Step 3: Implement** handlers: parse body (same error wording as turn-end), `token_matches` → `ApiError::unauth(...).with_status(403)`, then the `*_value` functions. `restart-preview` calls `state.devservers.start` then returns `preview_status_value`.
- [ ] **Step 4: Run** `cargo test --test ctl` — PASS. Add the auth exemption and a router-level test that `/api/ctl/preview-status` without daemon token reaches the handler (403 hook token, not 401).

### Task 2: `workbench ctl` client

**Files:**
- Create: `src/ctl.rs`
- Modify: `src/lib.rs` (`pub mod ctl`), `src/main.rs` (`Ctl` subcommand)
- Test: `src/ctl.rs` unit tests (stub TCP server, pattern from `src/hook.rs` tests)

**Interfaces:**
- Consumes: Task 1 endpoints; env `WORKBENCH_URL`, `WORKBENCH_PROJECT`, `WORKBENCH_HOOK_TOKEN`.
- Produces: `workbench ctl preview-status|restart-preview|change-status`; `pub fn request_for(cmd, base, project, token) -> (String, Value)` (pure); `pub async fn run(cmd: CtlCommand) -> ExitCode` printing pretty JSON to stdout, errors to stderr.

- [ ] **Step 1: Write failing tests**: `request_for` builds `{base}/api/ctl/preview-status` + `{project, token}` body for each variant; `run` against a stub server returns `SUCCESS` on 200 JSON and `FAILURE` on 403; missing env returns `FAILURE` without any request reaching the stub.
- [ ] **Step 2: Run** `cargo test --lib ctl` — expect compile failure.
- [ ] **Step 3: Implement** with reqwest (10 s timeout, same as `hook.rs`); missing env → `workbench ctl: run this inside a Workbench agent session` on stderr.
- [ ] **Step 4: Run** `cargo test --lib ctl` — PASS; `cargo run -- ctl --help` lists the three commands.

### Task 3: Workbench skill pack

**Files:**
- Modify: `src/providers.rs` (`WORKBENCH_GUIDE`, `agent_guide()`, all injection sites, `write_cursor_rule`)
- Test: `src/providers.rs` tests

**Interfaces:**
- Consumes: Task 2 command names (guide names them verbatim).
- Produces: `pub const WORKBENCH_GUIDE: &str`, `pub fn agent_guide() -> String` = Workbench guide + git guide.

- [ ] **Step 1: Write failing tests**: `agent_guide()` contains `workbench ctl preview-status`, `change-status`, `restart-preview`; contains the human-only rule (never publish/share/greenlight for the user) and plain-words rule; Claude/Codex launch + headless specs and the Cursor rule carry `agent_guide()`, not bare `GIT_GUIDE` (update the three existing assertions that pin `GIT_GUIDE`).
- [ ] **Step 2: Run** `cargo test --lib providers` — expect failures.
- [ ] **Step 3: Implement**: guide covers (a) who the user is and the five allowed words, (b) human-only actions, (c) verify after editing via ctl (`preview-status`; if `crashed`, `restart-preview`; re-check `change-status` before saying done), (d) post-greenlight edits void the greenlight, (e) ask in plain words when intent is unclear. Swap every `GIT_GUIDE` injection to `agent_guide()`; keep `GIT_GUIDE` public (tests, docs reference it).
- [ ] **Step 4: Run** `cargo test --lib providers` — PASS.

### Task 4: Template pointer + full verification

**Files:**
- Modify: `templates/astro-marketing/AGENTS.md` (one short "Checking your work" section naming the ctl commands)

- [ ] **Step 1:** Add the section: after editing, run `workbench ctl preview-status`; edits hot-reload, do not build; if `crashed`, `workbench ctl restart-preview` once, then report in plain words.
- [ ] **Step 2:** Run `just test` (web build, cargo tests, web unit tests, template build) — all PASS.
- [ ] **Step 3:** Manual smoke: `cargo run -- serve --no-auth` + a terminal agent session, run each ctl command by hand, confirm JSON matches the preview the UI shows.
