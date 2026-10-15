# Tauri Desktop App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Workbench to non-technical users as a signed, auto-updating macOS desktop app wrapping the existing Rust daemon and Vue UI.

**Architecture:** Tauri 2 thin shell. The existing daemon binary ships as a Tauri sidecar; the shell spawns it on an ephemeral port, health-checks `GET /api/health`, and points the webview at the daemon's local HTTP URL. All HTTP/WebSocket/preview traffic keeps the current transport — no Tauri IPC rewrite.

**Tech Stack:** Tauri 2 (Rust shell, `@tauri-apps/cli`), existing Rust daemon (axum/tokio), Vue 3 + Vite, GitHub Actions, GitHub Releases.

**Spec:** `docs/superpowers/specs/2026-10-15-tauri-desktop-design.md`

## Global Constraints

- Phase 1 platforms: macOS only, targets `aarch64-apple-darwin` and `x86_64-apple-darwin`.
- The daemon, hosted `/app`, PWA, and Homebrew paths must keep working unchanged; `just test` and the Playwright e2e suite stay green throughout.
- No Tauri IPC commands for daemon APIs; webview ↔ daemon stays HTTP/WebSocket on `127.0.0.1`.
- Agent CLIs (`claude`/`codex`/`cursor-agent`), Node, and git are never bundled in the installer.
- Unsigned builds are never shipped to users; signing + notarization are release blockers, not follow-ups.
- Every task ends committed separately, in order.

## Review Focus

- **Daemon killed or crashed mid agent-turn** — shell must restart it and the UI must reconnect without user action. Test in Task 4.
- **Quit while an agent turn or dev server is running** — no orphaned daemon, PTY, or dev-server processes afterwards. Test in Task 4.
- **Second app launch while one instance is running** — must focus the existing window, never spawn a second daemon. Test in Task 3.
- **Update offered while a terminal session is busy** — must be deferred until idle; applying mid-turn is a release-blocking bug. Test in Task 6.
- **First run on a machine with no `git`, no `gh`, no agent CLI** — onboarding must name each missing item in plain language with a next step, never a raw error. Test in Task 5.

---

### Task 1: Prove the daemon's desktop contract (no code changes expected)

**Files:**
- Test: `tests/` existing suite only.
- Verify: `src/lib.rs:85-105` (`serve()` binds `127.0.0.1:<port>`, prints `workbench ready: <url>`), `src/http/health.rs`, `src/lib.rs:38-72` (`serve_on()` graceful shutdown already kills terminals/devservers/watchers on signal).

**Interfaces:**
- Produces (confirmed contract for Task 3): sidecar started as `workbench serve --port 0 [--token <hex>]`; readiness = parse the `workbench ready: http://127.0.0.1:<port>/...` stdout line, then poll `GET /api/health` until `{"status":"ok"}`; shutdown = SIGTERM.

- [ ] **Step 1:** Run `cargo run -- serve --port 0` and confirm the printed ready URL uses a non-7800 ephemeral port and `GET /api/health` on it returns `{"status":"ok"}`.
- [ ] **Step 2:** Send SIGTERM during an active terminal session; confirm the process exits and no PTY child survives (`pgrep -P`).
- [ ] **Step 3:** If either check fails, stop and fix in `src/lib.rs` with a regression test in `tests/` before continuing; if both pass, record the confirmed contract in a short comment at the top of `src-tauri/src/main.rs` when Task 2 creates it.

### Task 2: Scaffold the Tauri shell crate

**Files:**
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/build.rs`, `src-tauri/icons/` (generated from the web app icon via `pnpm tauri icon`).
- Note: `src-tauri/Cargo.toml` carries its own empty `[workspace]` table so the shell stays outside the daemon package; `just test`/`cargo test --locked` at the root are unaffected (the shell is built by the Tauri CLI, not the daemon build).

**Interfaces:**
- Consumes: nothing yet; window opens `about:blank` in this task.
- Produces: `pnpm --prefix src-tauri tauri build` (or `cargo tauri build` from `src-tauri/`) produces `Workbench.app`; bundle identifier `com.workbench.app`; product name `Workbench`.

- [ ] **Step 1:** Scaffold with the Tauri 2 CLI (`pnpm dlx @tauri-apps/cli@2 init` equivalents done by hand so the existing repo files are not overwritten); pin `tauri` and `@tauri-apps/cli` to the current 2.x releases and commit the lockfiles.
- [ ] **Step 2:** Configure `tauri.conf.json`: `frontendDist` unused (webview loads the daemon URL, set in Task 3), `bundle.externalBin: ["binaries/workbench"]`, macOS targets `["app", "dmg"]`.
- [ ] **Step 3:** Add a build helper `src-tauri/scripts/stage-sidecar.sh` that runs `cargo build --release --target <triple>`, copies the daemon binary to `src-tauri/binaries/workbench-<triple>`, and is invoked before `tauri build` in dev docs and CI (Task 6).
- [ ] **Step 4:** Verify: staged sidecar present, `tauri build` produces an app bundle that opens a blank window; `just test` still passes at the repo root.
- [ ] **Step 5: Commit** — `feat(desktop): scaffold Tauri shell with daemon sidecar staging`

### Task 3: Daemon supervision — spawn, health, single-instance, shutdown

**Files:**
- Create: `src-tauri/src/supervisor.rs`, `src-tauri/src/lib.rs` (builder wiring) if the Tauri template splits them; otherwise keep modules under `src-tauri/src/`.
- Modify: `src-tauri/src/main.rs`, `src-tauri/tauri.conf.json` (sidecar permissions in `src-tauri/capabilities/default.json`: `shell:allow-execute` scoped to the `workbench` sidecar only).
- Test: `src-tauri/src/supervisor.rs` unit tests; manual checks below.

**Interfaces:**
- Consumes: Task 1 contract (ready-line parse, `/api/health`, SIGTERM).
- Produces: `Supervisor::start(app) -> Result<DaemonHandle, SupervisorError>`; `DaemonHandle { base_url: String, token: String }`; `DaemonHandle::shutdown()` used by the quit handler and the updater (Task 6); an app event `daemon://failed` carrying a user-readable message after 3 failed restarts.

- [ ] **Step 1: Write failing unit tests** for the pure parts: ready-line parsing (`workbench ready: http://127.0.0.1:51234/?t=abc…` → base URL + token) and restart backoff (delays 1s, 2s, 4s, give up after 3).
- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml` → FAIL (module missing).
- [ ] **Step 3: Implement `supervisor.rs`:** spawn sidecar `serve --port 0`, read stdout until the ready line, poll `/api/health` every 250 ms with a 15 s timeout, then navigate the main window to `<base_url>/?t=<token>` (same pairing the hosted app uses). Watch the child; on unexpected exit restart with backoff and re-navigate. On app quit, SIGTERM the child and wait up to 5 s before force-kill.
- [ ] **Step 4:** Register the single-instance plugin (must be the first plugin registered); a second launch focuses the existing window.
- [ ] **Step 5:** Run unit tests → PASS. Manual checks: launch app → UI loads with no daemon started by hand; launch again → same window focused; quit → `pgrep workbench` shows no leftover daemon.
- [ ] **Step 6: Commit** — `feat(desktop): supervise daemon sidecar with health check and single instance`

### Task 4: Failure UX — splash, error screen, watchdog recovery, log bundle

**Files:**
- Create: `src-tauri/ui/splash.html`, `src-tauri/src/logs.rs`.
- Modify: `src-tauri/tauri.conf.json` (point the initial window at the bundled `ui/splash.html`; Task 3's navigation replaces it with the daemon URL once healthy), `src-tauri/src/supervisor.rs` (emit `daemon://failed`), `src-tauri/src/main.rs` (register a `collect_logs` command — the only Tauri IPC command in the app, used solely by the error screen).
- Test: `src-tauri/src/logs.rs` unit tests.

**Interfaces:**
- Consumes: `daemon://failed` event + `DaemonHandle` from Task 3.
- Produces: error window state with **Retry** (restarts supervisor) and **Collect logs** (saves a zip of daemon stdout/stderr ring buffer + shell log to a user-chosen path via the dialog plugin).

- [ ] **Step 1: Write failing tests** for `logs.rs`: ring buffer keeps the last 500 lines; bundle writer produces a zip containing `daemon.log` and `shell.log`.
- [ ] **Step 2:** Run the shell tests → FAIL.
- [ ] **Step 3: Implement:** splash window at startup; on `daemon://failed`, swap the webview to the bundled error page with Retry / Collect logs; keep daemon stderr captured into the ring buffer from Task 3 onward.
- [ ] **Step 4:** Run tests → PASS. Manual checks: `kill` the sidecar mid-session → UI recovers without relaunch; kill it 4 times quickly → error screen appears and Collect logs saves a readable bundle; quit during an active terminal session → no orphaned processes.
- [ ] **Step 5: Commit** — `feat(desktop): splash, error screen, watchdog recovery and log bundle`

### Task 5: First-run onboarding for missing prerequisites

**Files:**
- Create: `src/http/setup.rs` (`GET /api/setup/status`), registered in `src/http/mod.rs`; `web/src/app/SetupCheck.vue`, `web/src/app/SetupCheck.test.ts`.
- Modify: `web/src/App.vue` (show `SetupCheck` before the shell when status is incomplete and the user has not dismissed it), `src/providers.rs` only if provider detection needs a reusable availability check.
- Test: `tests/setup_status.rs`, `web/src/app/SetupCheck.test.ts`.

**Interfaces:**
- Produces (daemon): `GET /api/setup/status` → `{ "git": bool, "gh": bool, "githubSignedIn": bool, "providers": [{ "id": string, "name": string, "available": bool }], "projectsDir": string }`. Provider availability reuses the registry behind the existing `GET /api/terminal/providers` (`src/http/terminal.rs:80`); GitHub state reuses the existing `/api/github/*` status.
- Produces (web): `SetupCheck.vue` renders one plain-language row per item (Ready / Action needed) and reuses `web/src/modules/github/DeviceCodeDialog.vue` for sign-in.

- [ ] **Step 1: Write failing tests.** Rust: with a PATH containing no agent CLIs, `/api/setup/status` reports every provider `available: false` and `git`/`gh` truthfully. Web: given an all-missing status fixture, `SetupCheck` shows one action row each for git, GitHub CLI, and an agent, and no raw error text.
- [ ] **Step 2:** Run `cargo test --test setup_status` and `pnpm -C web test SetupCheck` → FAIL.
- [ ] **Step 3: Implement** the endpoint (detection via `which`-style PATH lookup; no shelling out to the CLIs themselves) and the component with guided next steps per missing item (vendor install page links opened externally; **Recheck** button re-fetches status). Browser/PWA users see the same screen only when something is missing, so existing flows are untouched when the machine is fully set up.
- [ ] **Step 4:** Run both test suites → PASS; `just test` green.
- [ ] **Step 5: Commit** — `feat: first-run setup check for git, gh and agent CLIs`

### Task 6: Signed macOS release + auto-update

**Files:**
- Modify: `.github/workflows/release.yml` (new `desktop` job), `src-tauri/tauri.conf.json` (`plugins.updater` endpoints + pubkey), `src-tauri/src/main.rs` (updater check wiring), `web/src/app/` (small "update ready — restart to apply" notice fed by a daemon-visible flag or Tauri event; no mid-turn prompts).
- Create: `docs/desktop-release.md` (required secrets, certificate renewal, how to cut a desktop release).

**Interfaces:**
- Consumes: `DaemonHandle::shutdown()` (Task 3) before applying an update; terminal busy state from `GET /api/terminal/{project}` (`busy`, `src/http/terminal.rs:217`) — the updater defers while any known session is busy and while a turn is active.
- Produces: signed + notarized `Workbench-<version>-<arch>.dmg` and updater artifacts attached to the existing GitHub Release; existing tarball/Homebrew jobs unchanged.

- [ ] **Step 1:** Generate the updater signing keypair with the Tauri CLI signer; store the private key and password as CI secrets, commit only the public key in `tauri.conf.json`. Document every required secret (updater key, Apple Developer ID certificate, notarization credentials per Tauri's macOS signing guide) in `docs/desktop-release.md`.
- [ ] **Step 2:** Add the `desktop` release job: for each macOS target, build the web UI, stage the sidecar (Task 2 script), `tauri build`, sign, notarize, and upload `.dmg` + updater artifacts next to the existing tarballs; gate `publish` on it like the existing `build`/`e2e` jobs.
- [ ] **Step 3:** Wire the updater: check on launch and every 6 h; download in background; if any terminal session is busy, defer the prompt; apply only via the user clicking **Restart to update**, which shuts the daemon down gracefully first.
- [ ] **Step 4:** Verify end-to-end on a test release: install build N from the `.dmg` on a clean macOS user account, publish N+1, confirm the in-app update applies and projects/terminals still work; confirm `just test` and e2e still pass.
- [ ] **Step 5: Commit** — `feat(desktop): signed macOS release with deferred auto-update`

### Task 7: Non-technical beta and Windows go/no-go

**Files:**
- Create: `docs/superpowers/specs/` follow-up note recording beta findings (dated, e.g. `2026-10-15-tauri-desktop-beta-findings.md` — actual date at execution time).
- Modify: files implicated by beta blockers (expected: `web/src/app/SetupCheck.vue`, `src-tauri/ui/splash.html`).

**Interfaces:** none; this task is validation and triage, not new surface.

- [ ] **Step 1:** Manual WKWebView pass on Apple Silicon and Intel: terminal streaming (xterm), code editing (CodeMirror), rich text (TipTap), site preview iframe, file upload/download. File issues for every divergence from Chromium behaviour.
- [ ] **Step 2:** Clean-machine walkthrough with 3–5 non-technical users: download → install → first run → create a project → run an agent turn. Record where each user stalls; fix the top blockers (expected in onboarding copy, Task 5, and splash/error copy, Task 4).
- [ ] **Step 3:** Soak check: sleep/wake with a project open, daemon crash recovery (Task 4 checks repeated after wake), update from the previous beta build.
- [ ] **Step 4:** Write the findings note with a Windows go/no-go recommendation based on observed support load.
- [ ] **Step 5: Commit** — `docs: desktop beta findings and Windows decision`

## Sources

- Tauri sidecar binaries: https://v2.tauri.app/develop/sidecar/
- Tauri updater plugin: https://v2.tauri.app/plugin/updater/
- Tauri single-instance plugin: https://v2.tauri.app/plugin/single-instance/
- Tauri macOS code signing: https://v2.tauri.app/distribute/sign/macos/

