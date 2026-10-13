# Hosted site + `/app` with a local daemon — design

## Goal

Ship Workbench the way tabsh does: one public site with a landing page, docs, and the app at
`/app`. Users install and run the desktop daemon locally; the hosted `/app` talks to it from the
browser. The daemon keeps serving the same UI locally as an offline / Safari fallback.

## Decisions

| Topic | Decision |
|---|---|
| Where `/app` runs | Hosted on the public site; browser calls the local daemon at `http://127.0.0.1:7800` |
| Pairing | Daemon prints a one-time link `https://<site>/app#t=<token>`; token stored in `localStorage`; daemon also enforces an Origin allowlist |
| Offline fallback | Daemon still embeds the SPA and serves it at `http://127.0.0.1:7800/app/` |
| Structure | Two builds, one deploy: Astro `site/` + Vite/Vue `web/` built under `/app/`, merged in CI |
| Safari | Not supported on the hosted app in v1 (blocks HTTPS → `http://127.0.0.1`); gate points users to the local fallback |

## Architecture

```
https://<site>/            Astro landing          (site/)
https://<site>/docs/*      Astro + Starlight      (site/)
https://<site>/connect     Install/start daemon   (site/)
https://<site>/app/*       Vue SPA                (web/, copied into site/dist/app)
        │  fetch + Bearer token, WS + ?t=
        ▼
http://127.0.0.1:7800      Rust daemon (API, WS, /preview, embedded SPA at /app/)
```

One Cloudflare Worker (static assets) serves `site/dist`. Unknown paths under `/app/*` fall back to
`/app/index.html` so deep links and reloads work.

## Components

### `web/` (Vue SPA)

- **Base path.** Vite `base: "/app/"`; `createWebHistory("/app/")`. PWA `scope` and `start_url`
  move to `/app/`. Route definitions in `src/router.ts` are unchanged (they become
  `/app/p/:id/...`).
- **`src/lib/daemon.ts`** — the only place that knows where the daemon is:
  - `daemonBase()`: `localStorage["wb_daemon_url"]` if set; else the page origin when the page is
    served by the daemon (loopback host); else `http://127.0.0.1:7800`.
  - `daemonUrl(path)`, `wsUrl(path)` (adds `?t=<token>` when a token is stored),
    `authHeaders()` (`Authorization: Bearer <token>` when a token is stored).
  - `readPairingFragment()`: if `location.hash` has `t=`, store it in
    `localStorage["wb_token"]` and strip it with `history.replaceState`.
- **Call sites.** Every `fetch("/api…")`, WebSocket URL, and `/preview` iframe `src` goes through
  the helpers. When served by the daemon on loopback, the existing cookie auth keeps working; the
  Bearer header is additive.
- **`src/app/ConnectGate.vue`** wraps `<RouterView>` and resolves to one state:
  - `connected` → render the app.
  - `daemon-down` (`/api/health` unreachable) → "Start the daemon" with install/run command and a
    retry button; link to `/connect`.
  - `unpaired` (no token, or any API call returns 401) → clear stored token, "Pair again: open
    the link your daemon printed".
  - `unsupported` (Safari on a non-loopback origin) → "Use the local app" link to
    `http://127.0.0.1:7800/app/`.
- **WebSocket drops** reconnect with exponential backoff; after 5 consecutive failures the gate
  re-runs the health check.

### Daemon (Rust)

- **`src/static.rs`** serves the embedded SPA under `/app/`: `SPA_ROUTE_PREFIXES` becomes
  `["/app/"]` (assets and icons live under it), and `GET /` redirects to `/app/`.
- **`src/config.rs`** `origin` defaults to the production site URL (compile-time constant
  `DEFAULT_SITE_ORIGIN`); `--origin <url>` overrides it (preview branches), `--no-origin` disables
  it. `ui_url` defaults to `<origin>/app`.
- **`src/http/mod.rs`** CORS layer additionally:
  - allows request header `Access-Control-Request-Private-Network` on preflight, and
  - responds with `Access-Control-Allow-Private-Network: true`
  (Chrome Private Network Access, required for a public HTTPS page to call loopback).
- **Startup output** prints both links: hosted `<origin>/app#t=<token>` and local
  `http://127.0.0.1:7800/app/?t=<token>`.
- Host / WS-origin checks in `src/auth.rs` are unchanged; they already admit the configured
  origin.

### `site/` (Astro)

- Created from `templates/astro-marketing` (the template stays as-is for its own use).
- Adds Starlight for `/docs/*`; adds `/connect` (install + start + pair instructions).
- `wrangler.toml`: `[assets] directory = "./dist"`, `not_found_handling = "404-page"` (site-wide
  SPA fallback would swallow real 404s), plus a minimal Worker script that rewrites unmatched
  `/app/*` requests to `/app/index.html` and serves everything else from assets.

### Build and CI

- `justfile`: `build-site` builds `web` then `site`, copies `web/dist` → `site/dist/app`.
- The existing astro-marketing deploy workflow is pointed at `site/` with per-branch previews.
  Branch previews have their own origin, so testing one requires starting the daemon with
  `--origin <preview url>`.
- The daemon build keeps embedding `web/dist` (now built with base `/app/`).

## Data flow: pairing

1. User runs the daemon. It prints `https://<site>/app#t=<token>`.
2. Browser opens the link. `readPairingFragment()` stores the token and strips the hash (the token
   never reaches the Worker: fragments are not sent to servers).
3. `ConnectGate` calls `GET http://127.0.0.1:7800/api/health` → reachable.
4. App calls APIs with `Authorization: Bearer <token>`; WS uses `?t=<token>`. Daemon checks
   Origin/Host allowlist and token.

## Error handling

| Failure | Behaviour |
|---|---|
| Daemon not running | Gate `daemon-down`, retry button |
| Token missing/rejected (401) | Clear token, gate `unpaired` |
| Origin not allowed (403 from host/WS check) | Gate `daemon-down` variant explaining `--origin` |
| Safari on hosted origin | Gate `unsupported`, link to local app |
| WS drops | Backoff reconnect, then re-check health |

## Testing

- **Vitest:** `daemon.ts` (base resolution, header/WS token attachment, fragment parsing and
  stripping); `ConnectGate` state transitions with mocked fetch.
- **Rust:** preflight with `Access-Control-Request-Private-Network: true` from the configured
  origin gets `Access-Control-Allow-Private-Network: true`; other origins get no CORS headers;
  SPA served under `/app/` with deep-link fallback; `/` redirects to `/app/`.
- **Playwright e2e:** serve `site/dist` on a different port (acts as the public origin), start the
  daemon with `--origin` set to it, open the pairing link, load a project's preview tab.

## Out of scope (v1)

Accounts, local HTTPS certificates for Safari, daemon auto-update, OS installers/menubar app.
Docs cover manual install.
