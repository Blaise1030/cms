# orso

Local site builder: a Rust CLI daemon and a Vue SPA (embedded in the binary).
Edit content in the UI, chat with a coding agent about a project, preview locally, and
publish to GitHub Pages. Sync with a live site can hand conflict resolution to the agent.

Repo path is still `Blaise1030/cms`; the product name is **orso**.

## Install

**macOS (recommended):** Homebrew

```sh
brew install blaise1030/tap/orso
orso serve --open
```

Opens `http://orso.localhost:<port>/?t=…` (still bound to `127.0.0.1`). Override with
`--hostname` if you want another `*.localhost` name.

**macOS or Linux:** curl installer (downloads the matching GitHub Release tarball, verifies sha256)

```sh
curl -fsSL https://raw.githubusercontent.com/Blaise1030/cms/main/scripts/install.sh | sh
orso serve --open
```

Pin a version with `ORSO_VERSION=0.1.2`, or change the install dir with `ORSO_INSTALL_DIR`.
On macOS, prefer Homebrew when possible: a curl-downloaded binary can still pick up a
Gatekeeper quarantine attribute (the installer clears it when it can).

Windows is not shipped in this pass; see [docs/launch/briefs/001-windows-build.md](docs/launch/briefs/001-windows-build.md).

## Layout

| Path | Role |
| --- | --- |
| `src/` | Daemon (`orso` binary): API, preview, agents, publish, sync |
| `web/` | Vue SPA (embedded in release builds; Vite in dev) |
| `e2e/` | Playwright smoke tests against a release-built daemon |
| `templates/` | Starter site templates |
| `scripts/install.sh` | Curl installer for GitHub Release tarballs |

Releases are CLI tarballs + Homebrew only (the former Tauri macOS app was removed).

## Development

### Prerequisites

- [`just`](https://github.com/casey/just)
- Rust toolchain (`cargo`)
- Node.js with `npm` and `pnpm`
- GitHub CLI (`gh`) for GitHub sign-in; see [docs/github-setup.md](docs/github-setup.md)

Before sharing previews, protect `main` so merges need review; see
[docs/branch-protection.md](docs/branch-protection.md).

### Browser (daemon + Vite)

```sh
just setup   # first time: install web, orso-astro package and template dependencies
just dev     # daemon :7800, Vite :5173
```

Open **http://localhost:5173**. Vite proxies `/api`, `/ws`, and `/preview` to the daemon.
Port `:7800` serves the embedded `web/dist` build (no HMR). Ctrl-C stops both.

In this dev UI the preview frame loads the project's dev server directly (`127.0.0.1:<port>`),
so Astro islands and other client modules hydrate; the daemon-served build frames the proxied
preview instead. The direct frame is cross-origin, so the element picker and appearance
controls go through a small `postMessage` bridge that `orso()` injects into dev pages. The
page-error overlay and address-bar history still need the proxied preview.

Without `just`:

```sh
cargo run -- serve --ui-url http://localhost:5173 --open
pnpm -C web dev
```

### Agent providers

The agent panel's chat runs a coding CLI through the daemon. Built-in providers: Cursor CLI
(`cursor-agent`), Claude (`claude`), Codex (`codex`), and OpenCode. OpenCode is the recommended
first assistant: it runs free models with no account and is downloaded from the setup screen.

orso installs and pins its own copies of Node, the provider CLIs, and the Claude/Codex ACP
adapters. Downloads are verified, staged, and kept under
`~/Library/Application Support/orso/runtimes` (or the XDG data dir on Linux); a CLI you
installed yourself is used as a fallback. See [docs/agent-runtimes.md](docs/agent-runtimes.md).
Override providers in `~/.config/orso/agents.json`, or pass `--agents-config`.

### Build and test

```sh
just build   # SPA (+ PWA check), then release binary that embeds web/dist
just test    # web build, Rust tests, web unit tests, orso-astro package and template tests
just e2e     # Playwright smoke (rebuilds release daemon with embedded UI)
```

## Releases

GitHub Releases ship daemon tarballs (`orso-v*-{target}.tar.gz` + sha256) for macOS and Linux.
Homebrew is updated from the same workflow. There is no notarized desktop app.
