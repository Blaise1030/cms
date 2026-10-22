# orso

Local site builder: a Rust daemon, a Vue SPA, and (on macOS) a Tauri desktop shell.
Edit content in the UI, chat with a coding agent about a project, preview locally, and
publish to GitHub Pages. Sync with a live site can hand conflict resolution to the agent.

Repo path is still `Blaise1030/cms`; the product name is **orso**.

## Layout

| Path | Role |
| --- | --- |
| `src/` | Daemon (`orso` binary): API, preview, terminals, publish, sync |
| `web/` | Vue SPA (embedded in release builds; Vite in dev) |
| `src-tauri/` | macOS desktop app: overlay title bar, supervises the daemon, in-app updater |
| `e2e/` | Playwright smoke tests against a release-built daemon |
| `templates/` | Starter site templates |

## Development

### Prerequisites

- [`just`](https://github.com/casey/just)
- Rust toolchain (`cargo`)
- Node.js with `npm` and `pnpm`
- GitHub CLI (`gh`) for GitHub sign-in; see [docs/github-setup.md](docs/github-setup.md)
- **Desktop only (macOS):** Xcode CLT / Tauri prerequisites; see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

Before sharing previews, protect `main` so merges need review; see
[docs/branch-protection.md](docs/branch-protection.md).

### Browser (daemon + Vite)

```sh
just setup   # first time: install web, orso-astro package and template dependencies
just dev     # daemon :7800, Vite :5173
```

Open **http://localhost:5173**. Vite proxies `/api`, `/ws`, and `/preview` to the daemon.
Port `:7800` serves the embedded `web/dist` build (no HMR). Ctrl-C stops both.

Without `just`:

```sh
cargo run -- serve --ui-url http://localhost:5173
pnpm -C web dev
```

### Desktop (macOS)

Title-bar-less shell with web HMR. Defaults stay clear of `just dev` so both can run:

```sh
just desktop-dev          # Vite :5180, daemon :7810
# or: just desktop-dev 5173 7800
```

Runs `src-tauri/scripts/desktop-dev.sh`: debug daemon, Vite, then `tauri dev` with
identifier `com.orso.desktop.dev` (leaves an installed orso.app alone).
Shell (`src-tauri/`) edits rebuild and relaunch; daemon (`src/`) edits need a restart.
Ctrl-C stops everything. Release builds ignore `ORSO_DEV_*`.

Signing, notarization, updater keys, and cutting a release:
[docs/desktop-release.md](docs/desktop-release.md).

### Agent providers

The agent panel's chat runs a coding CLI through the daemon. Built-in providers: Cursor CLI
(`cursor-agent`), Claude (`claude`), and Codex (`codex`). Override in
`~/.config/orso/agents.json`, or pass `--agents-config`.

### Build and test

```sh
just build   # SPA (+ PWA check), then release binary that embeds web/dist
just test    # web build, Rust tests, web unit tests, orso-astro package and template tests
just e2e     # Playwright smoke (rebuilds release daemon with embedded UI)
```

## Releases

GitHub Releases ship daemon tarballs and the signed macOS app (DMG + updater artifacts).
Details and required secrets: [docs/desktop-release.md](docs/desktop-release.md).
