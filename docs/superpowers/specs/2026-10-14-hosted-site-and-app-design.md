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
- **Daemon location — `src/core/config.ts` (existing).** Already the single place that resolves
  the daemon origin + token (`?daemon=`, `?t=`, stored config, page origin) and already feeds the
  Bearer header (`core/api.ts`) and `?t=` WebSocket URLs (`core/socket.ts`,
  `modules/terminal/socket.ts`). Changes:
  - `resolveDaemonConfig` also consumes `#t=<token>` from `location.hash` (pairing link) and
    strips it, same as `?t=`.
  - The fallback origin when nothing is stored is the page origin only when the page host is
    loopback (`localhost`, `*.localhost`, `127.0.0.1`, `[::1]`); otherwise
    `DEFAULT_DAEMON_ORIGIN = "http://127.0.0.1:7800"`.
- **Gate — `src/app/DaemonUnreachable.vue` + `src/App.vue` (existing).** The app already shows an
  unreachable screen with reasons `network` (daemon not running) and `rejected` (token/origin
  refused). Add an `unsupported` state: Safari on a non-loopback origin, with a link to
  `http://127.0.0.1:7800/app/`. The `network` copy links to `/connect`; the `rejected` copy says
  "open the pairing link your daemon printed". Existing socket backoff/reconnect is unchanged.

### Daemon (Rust)

- **`src/static.rs`** serves the embedded SPA under `/app/`: `SPA_ROUTE_PREFIXES` becomes
  `["/app/"]` (assets and icons live under it), and `GET /` redirects to `/app/`.
- **`src/main.rs` / `src/config.rs`** `origin` defaults to the build-time env
  `WORKBENCH_SITE_ORIGIN` (`option_env!`; release CI sets it to the production site, local builds
  leave it unset → no default). `--origin <url>` overrides it (preview branches), `--no-origin`
  disables it.
- **`src/http/mod.rs`** CORS layer additionally:
  - allows request header `Access-Control-Request-Private-Network` on preflight, and
  - responds with `Access-Control-Allow-Private-Network: true`
  (Chrome Private Network Access, required for a public HTTPS page to call loopback).
- **Startup output** (`src/lib.rs`) prints the hosted link
  `<origin>/app/?daemon=http://<bound>#t=<token>` (so a non-default `--port` works) when an
  origin is configured, and always the local link `<ui_url or http://bound>/app/?t=<token>`.
- Host / WS-origin checks in `src/auth.rs` are unchanged; they already admit the configured
  origin.

### Live preview from the hosted app

An iframe cannot send `Authorization`, and the `SameSite=Lax` `wb_session` cookie is neither stored
nor sent in a cross-site iframe, so today `PreviewPanel` shows a notice instead of the preview
when the daemon is cross-site. Fix with a short-lived ticket exchanged for a partitioned cookie:

- **`POST /api/preview/ticket`** (normal auth) → `{ "ticket": "<32-byte hex>" }`. Tickets are
  single-use, expire after 60 s, held in memory.
- **Auth (`src/auth.rs`).** On `/preview/*` and Vite root paths (`is_vite_dev_path`), a request
  is also authenticated by:
  - `?pt=<ticket>` — consumes the ticket and sets
    `wb_preview=<preview secret>; Path=/; HttpOnly; SameSite=None; Secure; Partitioned`, or
  - a `wb_preview` cookie equal to the preview secret (random per daemon run, distinct from the
    main token, and accepted only on those paths — never on `/api` or `/ws`).
- **`wb_vite` cookie (`src/preview.rs`)** is set with `SameSite=None; Secure; Partitioned` when
  the request was authenticated via the preview ticket/cookie; unchanged (`SameSite=Lax`)
  otherwise, so the same-origin local app keeps working in every browser.
- **Credential stripping** also removes `pt` from the query and `wb_preview` from cookies before
  forwarding to the dev server.
- **Frontend.** `api.previewTicket()` calls the endpoint. When `isCrossSite(page, daemon)`,
  `PreviewPanel` loads `previewUrl(id) + "?pt=" + ticket` (fetching a fresh ticket on each
  (re)load) instead of showing the notice.
- Chrome and Firefox accept `Secure` cookies on `http://127.0.0.1` (a potentially trustworthy
  origin); Safari is already routed to the local app.

### `site/` (Astro)

- Copied from `templates/astro-marketing`. The template is the scaffold the daemon embeds into
  every new project (`static.rs` `Template`), so it is **not modified**; `site/` drops the
  scaffold-only parts (`scripts/workbench-content*`, `.github/`, `AGENTS.md`, `DESIGN.md`,
  `PUBLIC_BASE`/`WORKBENCH_PROXY_PORT` handling in `astro.config.mjs`).
- Adds Starlight for `/docs/*`; adds `/connect` (install + start + pair instructions).
- `wrangler.toml`: `main = "worker.js"`, `[assets] directory = "./dist"`, `binding = "ASSETS"`,
  `not_found_handling = "404-page"` (a site-wide SPA fallback would swallow real 404s).
  `worker.js` serves `/app/*` requests from assets and, on a 404 for an extension-less path,
  returns `/app/index.html`; everything else passes through to assets.

### Build and CI

- `justfile`: `build-site` builds `web` then `site`, copies `web/dist` → `site/dist/app`.
- New `.github/workflows/site.yml` (modelled on the template's `deploy.yml`) builds with
  `just build-site` and deploys `site/` to Cloudflare Workers with per-branch previews.
  Branch previews have their own origin, so testing one requires starting the daemon with
  `--origin <preview url>`.
- `release.yml` sets `WORKBENCH_SITE_ORIGIN` from the repo variable `SITE_ORIGIN` when building
  the daemon.
- The daemon build keeps embedding `web/dist` (now built with base `/app/`).

## Data flow: pairing

1. User runs the daemon. It prints `https://<site>/app/#t=<token>`.
2. Browser opens the link. `resolveDaemonConfig` stores the token and strips the hash (the token
   never reaches the Worker: fragments are not sent to servers). Origin falls back to
   `http://127.0.0.1:7800` because the page is not on loopback.
3. The event socket connects to `ws://127.0.0.1:7800/ws/events?t=<token>`; on failure the
   existing probe calls `/api/health` to pick `network` vs `rejected`.
4. App calls APIs with `Authorization: Bearer <token>`. Daemon checks Origin/Host allowlist and
   token.
5. Preview: app fetches a ticket, iframe loads `/preview/{id}/?pt=<ticket>`, daemon sets the
   partitioned `wb_preview` cookie, later frame requests use the cookie.

## Error handling

| Failure | Behaviour |
|---|---|
| Daemon not running | `DaemonUnreachable` reason `network`, retry, link to `/connect` |
| Token missing/rejected, or origin refused | Reason `rejected`, "open the pairing link your daemon printed" (mentions `--origin` for preview branches) |
| Safari on hosted origin | Reason `unsupported`, link to local app |
| Preview ticket expired/reused | Daemon 401; panel fetches a fresh ticket on reload |
| WS drops | Existing backoff reconnect |

## Testing

- **Vitest:** `resolveDaemonConfig` (hash token consumed and stripped, loopback vs non-loopback
  fallback origin); `isUnsupportedBrowser`; `PreviewPanel` uses a ticket URL when cross-site.
- **Rust:** preflight with `Access-Control-Request-Private-Network: true` from the configured
  origin gets `Access-Control-Allow-Private-Network: true`; SPA served under `/app/` with
  deep-link fallback; `/` redirects to `/app/`; ticket is single-use and expires; `wb_preview`
  cookie admits `/preview/*` but not `/api/*`; `pt` and `wb_preview` are stripped before
  forwarding.
- **Playwright e2e:** serve `web/dist` under `/app/` on a different port (acts as the public
  origin), start the daemon with `--origin` set to it, open the pairing link, load a project's
  preview tab and assert the framed page renders.

## Out of scope (v1)

Accounts, local HTTPS certificates for Safari, daemon auto-update, OS installers/menubar app.
Docs cover manual install.
