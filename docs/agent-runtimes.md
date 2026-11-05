# Agent runtimes

orso downloads and verifies the tools behind the assistant instead of asking
users to run installer scripts. The design follows OpenBot's managed-runtime
model: one shared store, pinned artifacts, source-published hashes, atomic
installs, and a managed copy preferred over one already on the machine.

## What gets installed

| Runtime | Source | Used for |
| --- | --- | --- |
| `node` | nodejs.org tarball + `SHASUMS256.txt` | `npm install`, Astro dev servers, Claude/Codex ACP adapters |
| `opencode` | `@opencode/cli-<target>` npm platform tarball | Chat over native ACP; free models work signed out |
| `claude` | `@anthropic-ai/claude-agent-sdk-<target>` | Claude chat, driven by the pinned ACP adapter |
| `codex` | OpenAI Codex GitHub release archive | Codex chat, driven by the pinned ACP adapter |
| `cursor` | Cursor versioned `agent-cli-package` archive | Cursor chat over native ACP |

Claude and Codex additionally get a pinned JavaScript ACP adapter, installed
with the managed `npm`:

- `@agentclientprotocol/claude-agent-acp`
- `@agentclientprotocol/codex-acp`

OpenCode and Cursor speak ACP themselves and need no adapter.

## Store

The default root is:

```text
~/Library/Application Support/orso/runtimes/
```

`ORSO_RUNTIME_DIR` or `--runtime-dir` overrides it (tests use this).

```text
runtimes/
  node/<version>/bin/node, bin/npm
  opencode/<version>/bin/opencode
  claude/<version>/bin/claude
  codex/<version>/bin/codex
  cursor/<version>/bin/cursor-agent
  acp/claude/…                  # pinned adapter + node_modules
  acp/codex/…
  .downloads/                   # resumable partial downloads
  .staging-… / .replaced-…      # transient, swept by age
```

Every completed install has `orso-install.json` beside its files, naming the
version, target, source URL and a SHA-256 for every installed file. The runtime
manager checks the manifest and executable on start and refuses a damaged or
replaced install.

The store is shared by every orso profile on the computer. Old versions are
retained for rollback and swept after a retention window; the current version
and one previous version are kept.

## Lock file

`agent-runtime.lock.json` at the repository root pins, for `darwin-arm64` and
`darwin-x64`:

- exact version and source URL/package;
- archive SHA-256 and byte size;
- installed size;
- executable path inside the install;
- per-file hashes where the archive contains more than one relevant file
  (Cursor);
- the Claude/Codex ACP adapter package and version.

The file is embedded in the daemon, so release builds install the same bytes
the repository reviewed. Regenerate it with:

```sh
node scripts/update-agent-runtimes.mjs
```

The script queries npm, GitHub Releases, nodejs.org and Cursor's distribution,
downloads the artifacts needed to compute hashes, and rewrites the lock. Review
the diff before committing. Tests never use the network.

### Update policy

v1 is pinned: a new provider version reaches users with an orso release. The
`updateAvailable` status and the row's **Update** button already use the same
download route, so a later upstream-release check can populate that state
without changing the UI. A failed update keeps the previous install.

## Download and install safety

- HTTPS only, with a source-published SHA-256 or npm integrity value.
- Download to `.downloads/<id>-<version>.partial`; resume with `Range` when the
  server supports it, otherwise restart. Stop at the pinned byte count.
- Verify the archive hash before extraction.
- Extract through the safe archive reader:
  - no absolute paths, `..`, or escapes from staging;
  - no symlinks, hardlinks, device/fifo/socket entries;
  - no duplicate paths;
  - size and entry-count limits;
  - only the documented executable gets `0o755`.
- Stage, verify, then rename atomically. A failed install removes only its
  staging/replaced directory; the previous working version stays.
- Require download size + installed size + headroom before starting and report
  a plain “not enough disk space” error.
- No `sudo`, no vendor installer scripts, no shell-profile edits.

## Resolution and PATH

For a built-in provider the order is:

1. `ORSO_ACP_ARGV` (tests and explicit overrides)
2. an explicit `acp` array in `~/.config/orso/agents.json`
3. the managed runtime + pinned adapter
4. a CLI already installed on the machine

Managed binaries are spawned by absolute path, never by PATH lookup. Child
processes get:

1. the git shim;
2. the managed Node `bin`;
3. managed provider bins;
4. the login-shell PATH;
5. the daemon PATH.

This is what makes a Finder-launched app see both the managed runtimes and a
Homebrew/`~/.local/bin` install. Claude/Codex adapters are pointed at the managed
provider binary with `CLAUDE_CODE_EXECUTABLE` / `CODEX_PATH`.

`agents.json` and `ORSO_ACP_ARGV` remain the escape hatch for custom agents.

## Sign-in

Sign-in reuses each CLI's own credentials; orso never reads or copies credential
files.

| Assistant | Command | Notes |
| --- | --- | --- |
| OpenCode | `opencode auth login opencode --method device` | Optional: free models work signed out |
| Claude | `claude auth login` | Browser URL; pasted code goes to stdin |
| Codex | `codex login` | `codex login --device-auth` fallback |
| Cursor | `cursor-agent login` | `cursor-agent status` confirms |

The daemon keeps one pending flow per provider, opens the printed URL, accepts a
pasted code when asked, and cancels by killing the process group. Flows time out
after ten minutes so a row cannot stay “connecting” forever.

## HTTP

All routes require the daemon token.

```text
GET  /api/setup/status
GET  /api/setup/runtimes
POST /api/setup/runtimes/{id}/download
POST /api/setup/runtimes/{id}/cancel
POST /api/setup/runtimes/{id}/retry
POST /api/setup/providers/{id}/verify
POST /api/setup/providers/{id}/sign-in
GET  /api/setup/providers/{id}/sign-in
POST /api/setup/providers/{id}/sign-in/code
POST /api/setup/providers/{id}/cancel
POST /api/setup/providers/{id}/sign-out
```

`GET /api/setup/status` and `/runtimes` use the same runtime shape:

```json
{
  "phase": "ready",
  "version": "2.2.5",
  "source": "managed",
  "progress": null,
  "message": null
}
```

`chatReady` means the resolved ACP command can start now. OpenCode is
`chatReady` while signed out because its free models need no account. Claude,
Codex and Cursor also need a sign-in.

`setup.runtime` events carry `{ runtime, phase, version, source, progress,
message }`; the assistant store merges them into the setup screen and chat.

## Troubleshooting

- **The row says “Download failed”** — press Retry. The previous install, if
  any, is still used. The row's details contain the daemon message.
- **Claude/Codex says “Getting chat ready…”** — the pinned adapter is being
  installed with the managed npm. It needs the site builder (Node).
- **A provider works in Terminal but not in orso** — check the row's source.
  Managed copies win over system copies; `agents.json` overrides both.
- **Collect logs from the desktop app** — the log bundle includes runtime
  status. On a browser build, `GET /api/setup/runtimes` shows the snapshot.
- **Starting over** — delete `~/Library/Application Support/orso/runtimes/` and
  download again. This also removes sign-in-independent manager state; CLI
  credentials live in each tool's own store and are untouched.

## Licence note

OpenBot is PolyForm Noncommercial 1.0.0. This implementation follows its design
patterns; no OpenBot source or lock file is copied.
