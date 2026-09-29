# workbench — Design Spec

Date: 2026-09-29
Status: Approved by user (brainstorming session)

## Summary

**workbench** is a minimal browser-based file editor with Git integration. A single Rust daemon binary (`workbench`) serves one or more workspace folders over an HTTP+WebSocket API and hosts the web UI. The frontend (Vue 3 + Vite) provides a file tree sidebar, a CodeMirror 6 editor, a diff view with per-hunk staging, and an xterm.js terminal.

Non-goals (v1): search, command palette, branch/push/pull UI, LSP, relay/E2E tunneling, settings UI, theming options. Git operations beyond stage/commit are done in the terminal.

## Goals

1. Small: single ~7 MB binary embedding the UI; frontend core bundle < 300 KB gzipped.
2. Fast: daemon cold start < 20 ms, idle RSS < 25 MB, save round-trip < 10 ms on localhost.
3. Minimal: no features beyond what's specified; YAGNI applies ruthlessly.
4. Modular: every daemon module and frontend module is independently replaceable.

## Architecture

```
Browser (Vue 3 + Vite; CodeMirror 6; xterm.js)
   │  served from http://127.0.0.1:7800 (embedded via rust-embed)
   │  or from a hosted origin (e.g. https://app.example.com) via --origin
   │
   │  REST  /api/*      — workspaces, tree, files, git (request/response)
   │  WS    /ws/events  — push: fs.change, git.status hints
   │  WS    /ws/pty     — binary PTY stream per terminal
   │
Rust daemon (axum + tokio, single crate)
   │  std::fs · git CLI (subprocess) · portable-pty · notify
```

Transport rationale: request/response ops go over REST (debuggable, idempotent); real-time freshness comes from the `/ws/events` push channel, which pushes file-watcher events so open editors track disk state continuously. This mirrors Paseo's split (REST/commands vs WS events).

## Repository layout

```
/                    Cargo workspace root (single crate `workbench`)
  src/               daemon modules (below)
  web/               Vue 3 + Vite frontend (pnpm)
  justfile           dev / build / test commands
  docs/              specs
```

## Daemon (Rust, single crate, clear modules)

| Module | Responsibility |
|---|---|
| `main.rs` | clap CLI: `workbench serve [PATHS]... [--port 7800] [--origin URL] [--no-auth] [--token T]`. No paths → cwd. Prints ready URL with token. |
| `state.rs` | AppState: workspace registry (id → canonical path), config, broadcaster. |
| `workspaces.rs` | Root registration, canonicalization, id generation (slug of dir name, deduped). |
| `fs.rs` | Tree listing (one level), read, write, create, rename, delete. Path sandboxing (below). Writes carry `baseMtimeMs` for optimistic locking. |
| `git.rs` | Typed wrappers over the `git` CLI via tokio::process: status, diff, stage (file or patch), unstage, commit. |
| `watch.rs` | `notify` recommended-watcher per root, ~100 ms debounce, broadcast `fs.change` events. Ignores `.git` internals. |
| `pty.rs` | One portable-pty session per `/ws/pty` connection; user's default shell; cwd = workspace root; binary WS frames. |
| `http/` | Router: thin handlers that parse input, call a module, return JSON. |
| `auth.rs` | Token middleware (REST: `Authorization: Bearer`), WS token check, Host-header allowlist, CORS for `--origin`. |
| `static.rs` | rust-embed of `web/dist`; serves the UI at `/` (dev: Vite proxy). |
| `error.rs` | Uniform error JSON `{ "error": { "code", "message" } }`. Codes: `SANDBOX`, `ENOENT`, `EISDIR`, `CONFLICT`, `UNAUTH`, `GIT_ERR`, `IO`, `BADREQ`. |

Modules do not import each other except through `AppState` and typed structs in a small shared `types` module. `fs.rs`, `git.rs`, `watch.rs`, `pty.rs` are HTTP-agnostic and unit-testable.

## REST API (all JSON; Bearer token required except /api/health)

- `GET /api/health` → `{ ok: true }` (no auth; for probes)
- `GET /api/workspaces` → `{ roots: [{ id, name, path }] }`
- `GET /api/tree?root=<id>&path=<rel>` → `{ entries: [{ name, kind: "file"|"dir", size }] }` — single level, dirs first; respects `.gitignore` (via git `status --ignored` or `ls-files` fast path — implementer's choice, must be fast).
- `GET /api/file?root=<id>&path=<rel>` → `{ content, binary: false, mtimeMs, size }` — UTF-8 text only; binary files → `{ binary: true }` and no content (UI shows a placeholder).
- `PUT /api/file` `{ root, path, content, baseMtimeMs? }` → `{ mtimeMs }`; if `baseMtimeMs` present and stale → `409 { error.code: "CONFLICT" }`. Omitting `baseMtimeMs` (create) is allowed.
- `POST /api/file` `{ root, path, kind: "file"|"dir" }` — create (fails if exists).
- `POST /api/rename` `{ root, from, to }` — move/rename.
- `DELETE /api/file?root=<id>&path=<rel>` — delete file or directory (recursive).
- `GET /api/git/status?root=<id>` → `{ branch, upstream, ahead, behind, files: [{ path, staged, unstaged, untracked, renamedFrom? }], added, removed }` (parsed from `git status --porcelain=v2 --branch -z`).
- `GET /api/git/diff?root=<id>&path=<rel>&staged=<bool>` → `{ diff }` (unified diff text). Untracked files: daemon runs `git add -N <path>` on first diff request so the diff renders; noted side effect on the index.
- `POST /api/git/stage` `{ root, paths: [rel...] }` — `git add`. OR `{ root, path, patch }` — apply a single-hunk patch via `git apply --cached -`.
- `POST /api/git/unstage` — same shapes; paths → `git restore --staged`; patch → `git apply --cached -R -`.
- `POST /api/git/commit` `{ root, message }` — commits the staged index (`git commit -m`). UI flow is stage → commit; no path-scoped commits in v1.

## WebSocket endpoints

- `GET /ws/events?token=T` — after token check, server pushes:
  - `{ "type": "fs.change", "root": id, "path": rel, "kind": "modify"|"create"|"delete"|"rename" }`
  - `{ "type": "git.status", "root": id }` — emitted after daemon-side git mutations so other tabs refresh.
- `GET /ws/pty?root=<id>&token=T` — binary frames. Client→server: raw bytes = stdin; JSON text frames `{ "type": "resize", "cols", "rows" }`. Server→client: raw bytes (PTY merges stdout/stderr). One shell process per connection; closed on disconnect.

Auth note: browsers cannot set headers on WS upgrade, so WS uses the query token. REST uses Bearer.

## Security model (Paseo-inspired)

1. **Bind**: `127.0.0.1` only, default port 7800.
2. **Token**: 32-byte random hex generated at startup (or `--token`). Printed as click-ready `http://127.0.0.1:7800/?t=<token>`; UI stores it in localStorage and sends Bearer/query. `--no-auth` disables (localhost-only use).
3. **Host-header allowlist**: allow `localhost`, `*.localhost`, `127.0.0.1`, `[::1]`, plus `--origin` host. Reject all others (DNS-rebinding protection).
4. **CORS**: only the `--origin` origin is allowed (with Bearer header). Embedded UI is same-origin, needs nothing.
5. **Path sandboxing**: every path from a client is joined to its root, `canonicalize()`d, and must remain under that root's canonical path. Symlink escapes are caught by canonicalization. Applies to REST and git paths.

## Frontend (Vue 3 + TypeScript + Vite, pnpm)

```
web/src/
  core/            The ONLY layer that knows about the daemon
    api.ts         typed REST client (fetch, token, error mapping)
    socket.ts      /ws/events client; PTY byte stream helper
    types.ts       TreeNode, FileContent, GitStatus, events...
  modules/
    workspaces/    pinia store: root registry, active root
    tree/          sidebar tree (lazy expand per level, dir-first; git status
                   badges colored via git store; minimal trees.software aesthetic)
    editor/        tab bar, CodeMirror 6 host, dirty tracking, cmd+S save,
                   409-conflict toast + reload prompt, lazy-loaded language
                   packs per file extension
    git/           status panel (file list, +N/-N stats), diff tab using
                   @codemirror/merge (unified + split toggle) with per-hunk
                   "stage hunk" buttons, commit box
    terminal/      xterm.js + fit addon, binary WS attach, resize
  app/             shell layout (sidebar | main | bottom panel), no router
  ui/               shared atoms (icons, buttons, toast)
```

Rules: a module may import only from `core/`, `ui/`, and its own files — never another module's internals. Each module owns its pinia store slice and components and can be deleted without touching others.

### Key flows

- Open file: tree click → `api.getFile` → editor tab; language pack dynamically imported by extension.
- Save: cmd+S → `PUT` with `baseMtimeMs` → 409 → toast "Changed on disk" → reload/discard choice.
- External change: `fs.change` event → if tab not dirty, reload silently; if dirty, mark tab as conflicted (badge) and prompt.
- Stage hunk: in diff tab, chunk gutter button → extract that hunk into a standalone patch (client-side) → `POST /api/git/stage { patch }` → refetch status.
- Commit: message box → `POST /api/git/commit` (index only).

### Styling

Hand-written CSS, dark theme by default, minimal palette (near-black canvas, one accent, monospace UI accents). No component library — keeps the bundle small.

## Error handling

- Daemon: every failure maps to the JSON error envelope; `SANDBOX` and `CONFLICT` are 4xx; git failures surface stderr excerpt in `message`.
- Frontend: toasts for transient errors; inline conflict UI in the editor; terminal shows exit messages in-band.

## Testing

- **Rust**: unit tests for path sandboxing, git porcelain parsing (fixture repos), optimistic-lock conflicts; integration test binding an ephemeral port, driving REST + WS with real HTTP/WS clients, including a scripted PTY session.
- **Web**: vitest for `core/api` mapping and store logic with mocked fetch/WS. No e2e framework in v1.

## Build & dev workflow

- `just dev` — `cargo run -- serve` + `pnpm dev` in `web/` (Vite proxies `/api`, `/ws` → 127.0.0.1:7800).
- `just build` — `pnpm build` then `cargo build --release` (rust-embed picks up `web/dist`).
- `just test` — `cargo test` + `pnpm test`.

## Performance notes

- Tree endpoint lists one directory level per call (no recursive walk) so huge repos stay fast.
- Language packs and xterm are lazy chunks; initial load is app shell + CodeMirror core.
- Watcher debounce coalesces editor-save echo storms; the daemon also filters events matching the write it just performed (mtime echo suppression) to avoid reload loops.

## Open questions

None blocking. (Untracked-file diff via `git add -N` is accepted as a small, documented index side effect.)
