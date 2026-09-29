# workbench — Design Spec (v2: local Lovable-style site builder)

Date: 2026-09-29
Status: Approved by user (brainstorming session; replaces the 2026-09-29 file-editor spec, rolled back)

## Summary

**workbench** is a local, Lovable-style marketing-site builder. A hosted Vue PWA (`app.<domain>`) drives a Rust daemon on the user's machine (Paseo model). The daemon talks to local coding agents over **ACP** (Agent Client Protocol) — Claude Code, Codex, Opencode via their ACP adapters — using the user's existing subscriptions. Agents edit an **Astro** project whose content lives in **content collections** (git-based CMS); a supervised Astro dev server is reverse-proxied into the UI as a live preview with working HMR. Every completed agent turn becomes a git **checkpoint**; any change is one-click revertable. Projects scaffold from an embedded Astro marketing template that is Cloudflare Pages–ready.

One-line pitch: chat on the right, live site on the left, git as the database, your own agent subscriptions as the engine.

## Goals

1. Lovable-like loop: prompt → agent edits → preview hot-reloads in ~1s.
2. Local-first: code and agent credentials never leave the machine; no server component required beyond static hosting of the SPA.
3. Small & fast: daemon cold start < 20 ms, idle RSS < 40 MB (excl. dev servers/agents), preview proxy overhead < 5 ms TTFB.
4. Modular: daemon modules and frontend modules are independently replaceable; agent support is data (registry), not code.
5. Trust: every agent turn is a git checkpoint; restore is trivial and safe.

## Non-goals (v1)

Full file editor and terminal UI in the builder (phase 2). Deploy execution from the UI (template is CF Pages-ready; you push and CF builds). Remote relay / tunneling. Multi-user. Undo/redo beyond git checkpoints. Theming of the builder UI. Agent prompt templating/library.

## Architecture

```
app.<domain> (Vue PWA, hosted)            http://127.0.0.1:7800 (same PWA, embedded via rust-embed)
        │ REST /api/* + WS /ws/events (token: Bearer header or wb_session cookie)
        └───────────────┬─────────────────────┘
                   Rust daemon (axum + tokio, single crate `workbench`)
        ┌───────────┬────┴──────────┬──────────────┐
        │ acp.rs    │ devserver.rs  │ git.rs       │
        │ ACP client│ free port,    │ checkpoints, │
        │ spawns &  │ spawns `npm   │ restore, log │
        │ drives    │ run dev` with │ watch.rs:    │
        │ agent     │ PUBLIC_BASE=/ │ notify →     │
        │ adapters  │ preview/{id}/ │ fs.change    │
        └───────────┴───────────────┴──────────────┘
        preview.rs: reverse proxy /preview/{id}/ → dev server (HTTP + WS)
        projects.rs · auth.rs · static.rs · http/ · fs.rs (read-only v1)
```

## Repository layout

```
/
  Cargo.toml            single crate `workbench`
  src/                  daemon modules
  web/                  Vue 3 + TS + Vite PWA (pnpm), hosted + embedded
  templates/
    astro-marketing/    the embedded scaffold template (an Astro project)
  justfile              dev / build / test
  docs/                 specs
```

## Daemon modules

| Module | Responsibility |
|---|---|
| `main.rs` | clap CLI: `workbench serve [--port 7800] [--origin URL] [--projects-dir DIR] [--no-auth] [--token T] [--agents-config PATH]`. Prints ready URL with token. |
| `state.rs` | AppState: projects, agent sessions, dev servers, event broadcaster. |
| `projects.rs` | Project registry (id → dir under projects-dir). `create` copies the embedded template, `git init` + initial commit. Delete = remove dir (guarded). |
| `acp.rs` | ACP client on Zed's `agent-client-protocol` crate. Per project: spawn subprocess, JSON-RPC over stdio: `initialize` → `newSession` (cwd = project dir) → `prompt`. Forwards `sessionUpdate` notifications (agent_message_chunk, tool_call, tool_call_update, plan) as WS events. Implements `request_permission`: file edits → always allow; command execution → surface as permission prompt in chat (Allow once / Deny); per-project `auto_approve` toggle skips prompts. Cancel via `cancel`. Persists each completed turn (user text, assistant text, tool summaries) to the history JSONL (see API). One live agent session per project; new session → old subprocess terminated. |
| `agents_config.rs` | Provider registry from `~/.config/workbench/agents.json`: `{ "claude": ["npx","-y","@zed-industries/claude-code-acp"], "codex": [...], "opencode": ["opencode","acp"] }` (exact adapter package names verified at plan time). UI lists providers from this file; adding any ACP-speaking agent is config-only. |
| `devserver.rs` | Allocate free port; spawn `npm run dev -- --port N` with env `PUBLIC_BASE=/preview/{id}/`; detect readiness on stdout; crash-restart with backoff (max 3). Lifecycle: dev server runs from project creation until daemon shutdown or project delete (no open/close API; "open project" is a UI-only concept) — plus manual `/devserver/restart`. |
| `preview.rs` | Reverse proxy `/preview/{id}/*` → `127.0.0.1:{port}` including WebSocket upgrade pass-through (HMR). Template's `PUBLIC_BASE` makes all asset/HMR URLs prefix-correct; no HTML rewriting. |
| `git.rs` | Checkpoints: after each agent turn settles (no running tool calls, 2s quiet), `git add -A && git commit -m "checkpoint: <turn summary>"`. `restore` = `git reset --hard <sha> && git clean -fd`. Log API returns checkpoints with turn metadata. Skips checkpoint if tree is clean. |
| `watch.rs` | `notify` watcher per project, running for the project's lifetime (same lifecycle as its dev server) → `fs.change` events (feeds UI file tree + checkpoint state). |
| `fs.rs` | Sandboxed read-only file ops for the UI (tree, get, raw blob). Canonicalize-and-contains check against project dir. |
| `http/` | Thin handlers: parse → module call → JSON. |
| `auth.rs` | Token middleware (REST: Bearer or `wb_session` cookie). WS: token via query param. Host-header allowlist (`localhost`, `*.localhost`, loopback IPs, `--origin` host) — DNS-rebinding protection. CORS: only `--origin`. Cookie flow: hitting `/?t=<token>` once sets `wb_session` so `<iframe>`-loaded preview and proxy URLs authenticate without headers. |
| `static.rs` | rust-embed of `web/dist` (SPA) and `templates/` (scaffold source). |
| `error.rs` | `{ "error": { "code", "message" } }`. Codes: `UNAUTH`, `SANDBOX`, `ENOENT`, `CONFLICT`, `AGENT_ERR`, `DEVSERVER_ERR`, `GIT_ERR`, `IO`, `BADREQ`. |

## REST API (JSON; auth as above)

- `GET /api/health` — no auth.
- `GET /api/agents` → `{ providers: [{ id, command }] }` (from registry).
- `GET /api/projects` → `{ projects: [{ id, name, port, devserver: "starting"|"ready"|"crashed" }] }`
- `POST /api/projects` `{ name }` → scaffolds from template, git init, starts dev server. → `{ id, ... }`
- `DELETE /api/projects/{id}` — stops dev server, removes dir (confirm in UI).
- `POST /api/projects/{id}/devserver/restart` — manual restart affordance (e.g. after crashes exhausted backoff).
- `PATCH /api/projects/{id}` `{ autoApprove?: bool, name?: string }` — project settings (e.g. the agent auto-approve toggle).
- `POST /api/agent/session` `{ project, provider }` → `{ sessionId }` (spawn + initialize + newSession). **One active session per project** — starting a new one terminates the previous cleanly. Provider is chosen per session; the UI remembers the last-used provider per project (client-side preference).
- `POST /api/agent/prompt` `{ project, sessionId, text }` → `{ turnId }` (streamed over WS).
- `POST /api/agent/prompt/{turnId}/cancel`
- `POST /api/agent/permission/{requestId}` `{ decision: "allow"|"deny" }`
- `GET /api/agent/history?project=&session=?` → `{ turns: [{ id, sessionId, provider, ts, userText, assistantText, tools: [{ tool, summary, state }] }] }` — chat history, optionally filtered by session. Persisted by `acp.rs` as JSONL at `{projects-dir}/.workbench/{id}/history.jsonl` (daemon state dir, deliberately outside the site repo so checkpoints never see it). Survives daemon restarts; deleted with the project.
- `GET /api/checkpoints?project=` → `{ checkpoints: [{ sha, summary, ts }] }`
- `GET /api/checkpoints/{sha}/diff?project=` → unified diff of that commit
- `POST /api/checkpoints/{sha}/restore` `{ project }`
- `GET /api/tree?project=&path=` · `GET /api/file?project=&path=` — read-only browsing.

## WebSocket `/ws/events?token=`

Tagged events; client reconciles by `projectId`/`sessionId`/`turnId`:

- `chat.chunk` `{ sessionId, turnId, delta }` (assistant text)
- `chat.tool` `{ sessionId, turnId, tool, state: "running"|"done"|"failed", summary }` (e.g. `edit src/content/sections/hero.mdx +12 -3`)
- `chat.done` / `chat.error` `{ sessionId, turnId, ... }`
- `permission.request` `{ requestId, sessionId, tool, command? }`
- `devserver.status` `{ projectId, state, port? }`
- `checkpoint.created` `{ projectId, sha, summary }`
- `fs.change` `{ projectId, path, kind }`

## Auth model (unchanged from v1 design, plus cookie)

1. Bind 127.0.0.1:7800 default; token = 32-byte random hex per start (or `--token`), printed as `http://127.0.0.1:7800/?t=<token>`.
2. `--origin` hosted PWA allowed via CORS + Host allowlist; `--no-auth` for localhost-only.
3. `wb_session` cookie (HttpOnly, SameSite=Lax, daemon origin) minted from `?t=` links — makes preview iframes and proxy URLs work header-less.
4. Path sandboxing for all file APIs; agent subprocesses run as the user (their credentials, their machine — the git checkpoint is the safety net for edits).

## Frontend (Vue 3 + TS + Vite, pnpm) — installable PWA

PWA layer: `vite-plugin-pwa` with manifest (name, maskable icons, theme color, standalone display) and a service worker using an auto-update strategy. **Precache the app shell only** (hosted-origin assets: JS/CSS/icons); all live data and previews come from the daemon origin at runtime and are never cached — offline state is an honest "daemon unreachable" screen with retry, not stale content. Result: instant loads, home-screen install on iOS/Android/desktop, and a native-app feel for the builder chrome.

```
web/src/
  core/            api.ts · socket.ts · types.ts   — the ONLY daemon-aware layer
  modules/
    projects/      grid of projects, create flow (name + provider), delete
    chat/          session thread: streaming assistant text, tool-call cards,
                   permission prompts (Allow/Deny), stop button, turn history
                   (restored on reload via GET history per session)
    preview/       iframe → {daemonOrigin}/preview/{id}/ ; device size toggles
                   (desktop/tablet/mobile), manual reload, dev-server status dot
    checkpoints/   timeline per project; click → @codemirror/merge diff view;
                   restore button with confirm
    files/         read-only tree + syntax-highlighted viewer (shiki) (phase 2:
                   editing)
  app/             shell: PREVIEW LEFT · CHAT RIGHT (user-specified), collapsible
  components/ui/   shadcn-vue primitives (button, card, dialog, sonner, …)
  lib/utils.ts     `cn()` helper (clsx + tailwind-merge)
```

Module rules: modules import only `core/*`, `components/ui/*`, `lib/*`, own files. Each owns its pinia store slice. Daemon origin for the SPA = `window.location` when embedded, or the configured daemon URL when hosted (stored after the user pastes/opens the daemon link).

Styling: Tailwind CSS v4 (`@tailwindcss/vite`) with `shadcn-vue` (New York / Neutral / CSS variables), dark mode via root `.dark`. Keep custom CSS to small overrides and layout glue only. Init with `pnpm dlx shadcn-vue@latest init`; add primitives with `pnpm dlx shadcn-vue@latest add …`.

## The scaffold template (`templates/astro-marketing/`)

- Astro static site; sections composed from content collections:
  - `src/content/config.ts` — zod schemas: `site` (name, nav, footer, socials), `pages`, `sections` (discriminated union: hero / features / pricing / testimonials / faq / cta / logos).
  - `src/content/sections/*.mdx` — actual copy, one file per section instance.
- `src/pages/index.astro` renders `pages`+`sections` via typed components in `src/components/sections/`.
- `astro.config.mjs` reads `PUBLIC_BASE` env (default `/`) → sets `base`, so the dev server works under the proxy prefix.
- `AGENTS.md` at repo root: the content model, file map, conventions ("content edits → edit MDX in src/content; new section type → add schema variant + component; run `npm run build` to verify"). This is what makes small-prompt, high-quality agent turns possible.
- `.github/workflows/deploy.yml`: build + deploy to Cloudflare Pages (via CF Pages git integration or `wrangler-pages-action`), plus `wrangler.toml` if needed. Repo is push-and-live.
- `package.json` pinned, `npm install` at scaffold time (daemon runs it before first dev server start).

## Error handling

Daemon: uniform error envelope per `error.rs`; agent subprocess death → `AGENT_ERR` + session marked dead with restart option; dev server crash → `DEVSERVER_ERR` + auto-restart (≤3, then UI banner). UI: toasts transient, inline banners persistent, chat errors inline in the thread.

## Testing

- **Rust**: fixture-repo tests for checkpoint/restore; proxy integration test (hyper test upstream, assert HTTP + WS passthrough + prefix base); fake ACP agent (scripted JSON-RPC over stdio) for acp.rs session lifecycle + permission flow; sandbox unit tests.
- **Web**: vitest for `core/api` mapping, socket reconnection, store logic with mocked fetch/WS.
- **Template**: scaffold → `npm run build` passes in CI.

## Build & dev workflow

- `just dev` — `cargo run -- serve` + `pnpm dev` (Vite proxy `/api`,`/ws`,`/preview` → 127.0.0.1:7800).
- `just build` — `pnpm build` → `cargo build --release` (embeds SPA + template).
- `just test`.

## Performance targets

Daemon cold start < 20 ms; idle RSS < 40 MB; preview proxy TTFB overhead < 5 ms; checkpoint commit < 150 ms on typical turn; SPA first load < 300 KB gzipped core (shiki/langs lazy).

## Phasing

- v1 (this spec): projects, ACP chat, live preview, checkpoints + diffs, read-only files, template.
- Phase 2 candidates (explicitly out of v1): in-UI editor (CodeMirror) + terminal (xterm), deploy button (wrangler), multi-file review before checkpoint, prompt library, relay for remote access.

## Open questions

None blocking. Verify at plan time: exact ACP adapter package names/versions for Claude Code / Codex / Opencode; Vite `base` behavior for client-side-routed Astro templates (v1 template is single-page, no client router, so low risk).
