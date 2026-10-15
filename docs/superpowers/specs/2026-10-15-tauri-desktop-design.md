# Tauri desktop app for non-technical users — design

## Goal

Ship Workbench to non-technical users as a signed, auto-updating macOS desktop
app. Users download one installer, open the app, and reach a working CMS
without installing Rust/Node, running a daemon by hand, or using a terminal.
The existing Rust daemon and Vue UI keep their behaviour; the desktop app is
an additional shell, not a replacement for the browser/PWA or Homebrew paths.

## Decisions

| Topic | Decision |
|---|---|
| Shell | Tauri 2, thin shell over the existing daemon |
| Daemon integration | Daemon ships as a Tauri sidecar binary; webview loads the daemon's local HTTP URL. No migration to Tauri IPC commands |
| Platforms, phase 1 | macOS only (`aarch64-apple-darwin`, `x86_64-apple-darwin`, matching `release.yml`) |
| Platforms, later | Windows is a gated phase 2 after the macOS beta shows real support load. Linux out of scope (WebKitGTK reliability) |
| Rejected alternative | Electron: identical Chromium rendering, but forces a Node main process plus the Rust daemon as sidecar (two runtimes) or a daemon rewrite |
| Updates | Tauri updater plugin, signed artifacts on GitHub Releases, offered only when no agent turn is running, applied on quit |

## Architecture

```
Workbench.app (Tauri 2)
├── webview (WKWebView) ──HTTP/WS──▶ http://127.0.0.1:<ephemeral port>
├── supervisor: spawn, health-check (/api/health), restart, shutdown
└── sidecar: workbench serve --port 0  (existing daemon, embeds web UI)
        └── PTY children (claude / codex / cursor-agent), git, fs watch
```

HTTP/WebSocket/preview proxying stay exactly as today, so the Playwright e2e
suite and the hosted `/app` work are unaffected. The shell is disposable: if
it dies, no project data is at risk; if the daemon dies, the supervisor
restarts it and the webview reconnects via the existing socket backoff.

## Components

### Daemon (Rust, minimal changes)

- `src/main.rs` / `src/config.rs`: `--port 0` binds an ephemeral port and the
  bound address is discoverable by the supervisor (stdout ready line, as the
  existing ready link already prints it).
- Graceful shutdown: on SIGTERM the daemon terminates PTY children and dev
  servers it spawned before exiting.
- `/api/health` (existing, unauthenticated) is the readiness probe.

### Tauri shell (new `src-tauri/` crate)

- Spawns the sidecar, polls `/api/health`, shows a splash window until
  healthy, then opens the main window at the daemon URL.
- Single-instance plugin: a second launch focuses the running window.
- Watchdog: restarts a crashed sidecar without closing the window; after
  repeated failures shows an error screen with a "collect logs" button that
  bundles daemon and shell logs for support.
- On quit: stops the sidecar and verifies no orphaned daemon, PTY, or dev
  server processes remain.

### First-run onboarding (web UI)

Non-technical users cannot be assumed to have `git`, `gh`, or an agent CLI.

- A setup screen checks each prerequisite and shows plain-language status.
- GitHub sign-in reuses the existing device flow.
- Agent CLIs (`claude` / `codex` / `cursor-agent`) cannot be silently bundled
  (licensing, per-user auth), so each gets a guided install/sign-in step.
- Projects folder picker with a default under the user's home directory.

### Packaging and updates (CI)

- Extend `.github/workflows/release.yml`: build `.app`/`.dmg` per macOS
  target alongside the existing tarballs.
- Developer ID signing and notarization via CI secrets; unsigned builds are
  not shipped (Gatekeeper warnings are a support failure for this audience).
- Updater artifacts published to the same GitHub Release; the app checks on
  launch and after idle, never mid agent-turn.

## Reliability requirements

- Port already in use, daemon crash mid-session, sleep/wake, and
  quit-during-turn must all recover or shut down cleanly.
- Manual WKWebView test pass on both macOS architectures covering the
  terminal (xterm), editor (CodeMirror), rich text (TipTap), and the site
  preview iframe — the areas where an OS webview can diverge from Chromium.

## Non-goals

- No Tauri IPC rewrite of daemon APIs.
- No Windows or Linux packaging in phase 1.
- No bundling of agent CLIs, Node, or git inside the installer.
- No changes to the hosted `/app`, PWA, or Homebrew distribution paths.

## Validation

- `just test` and the existing Playwright e2e pass unchanged.
- Launch the built app on a clean macOS account: window reaches a healthy UI
  with no manual daemon start.
- Kill the sidecar mid-session: watchdog recovers it, no orphaned processes.
- Publish build N+1 to a test release: build N updates itself cleanly.
- First-run walkthrough with 3–5 non-technical users; top onboarding
  blockers fixed before public release, then the Windows go/no-go decision.
