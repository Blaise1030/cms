# Hosted Site + `/app` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Serve a public site (landing, docs, `/app`) whose `/app` drives the user's local daemon, with the daemon still serving the same UI locally.

**Architecture:** The Vue SPA moves to base `/app/` and is served both by the daemon (embedded) and by a new Astro `site/` on Cloudflare. The hosted app reaches `http://127.0.0.1:7800` cross-origin using the existing token/origin machinery plus Chrome Private Network Access headers; the live preview iframe authenticates with a single-use ticket exchanged for a partitioned cookie.

**Tech Stack:** Rust (axum, tower-http), Vue 3 + Vite + vite-plugin-pwa, Vitest, Astro + Starlight, Cloudflare Workers (wrangler), Playwright.

**Spec:** `docs/superpowers/specs/2026-10-14-hosted-site-and-app-design.md`

## Global Constraints

- SPA base path is exactly `/app/`; routes become `/app/p/:id/...`.
- Default daemon origin for a non-loopback page: `http://127.0.0.1:7800` (`DEFAULT_DAEMON_ORIGIN`).
- Pairing link format: `<origin>/app/?daemon=http://<bound>#t=<token>` (the `daemon` param makes non-default `--port` work; the token stays in the fragment); local link: `<ui_url or http://bound>/app/?t=<token>`.
- Default `--origin` comes from build-time `option_env!("ORSO_SITE_ORIGIN")`; unset → no default.
- Preview ticket: 32-byte hex, single-use, TTL 60 s, query param `pt`.
- Preview cookie: `wb_preview=<secret>; Path=/; HttpOnly; SameSite=None; Secure; Partitioned`; accepted only on `/preview/*` and `is_vite_dev_path` paths.
- `templates/astro-marketing/` must not change (it is the embedded project scaffold).
- Safari on a non-loopback origin is unsupported → local-app link `http://127.0.0.1:7800/app/`.

## Review Focus

- **`wb_preview` cookie used against `/api` or `/ws`** must be 401 — a leaked preview cookie must not grant API access. Test in Task 6.
- **Replay of a consumed or expired ticket** must be 401. Test in Task 6.
- **Deep link reload on the hosted site** (`/app/p/x/code/src/index.astro`, dotted last segment) must return the SPA, not 404. Tests in Task 2 (daemon) and Task 8 (worker).
- **Old bookmarks to `/` or `/p/:id/...` on the daemon** must still land in the app (redirect to `/app/...`), not a JSON 404. Test in Task 2.
- **Pairing link opened while a different daemon's token is stored** must replace the stored token. Test in Task 4.

---

### Task 1: Private Network Access on CORS preflight

**Files:**
- Modify: `src/http/mod.rs:143-164`
- Test: `tests/cors.rs` (create; copy the `app()`/`req()` helpers from `tests/static.rs`, with `origin: Some("https://site.test".into())`)

**Interfaces:** none.

- [ ] **Step 1: Write failing tests**

```rust
#[tokio::test]
async fn preflight_from_configured_origin_allows_private_network() {
    // OPTIONS /api/projects, Host: 127.0.0.1:7800, Origin: https://site.test,
    // Access-Control-Request-Method: GET, Access-Control-Request-Private-Network: true
    // => 2xx, access-control-allow-origin == "https://site.test",
    //    access-control-allow-private-network == "true"
}

#[tokio::test]
async fn preflight_from_other_origin_gets_no_cors_grant() {
    // same request with Origin: https://evil.test
    // => no access-control-allow-origin and no access-control-allow-private-network header
}
```

- [ ] **Step 2:** `cargo test --test cors` → FAIL (header missing).
- [ ] **Step 3:** Add `.allow_private_network(true)` to the existing `CorsLayer` (tower-http ≥ 0.5 supports it; bump if needed). It only answers when the preflight carries `Access-Control-Request-Private-Network`.
- [ ] **Step 4:** `cargo test --test cors` → PASS.
- [ ] **Step 5:** `git commit -m "feat(daemon): allow private network access for the hosted origin"`

### Task 2: Move the SPA to `/app/` (web build + daemon serving)

**Files:**
- Modify: `web/vite.config.ts` (`base: "/app/"`), `web/src/router.ts:90` (`createWebHistory(import.meta.env.BASE_URL)`)
- Modify: `web/public/manifest.webmanifest` (`id`, `start_url`, `scope` → `/app/`; icon `src` → `/app/icons/...`)
- Modify: `web/pwa.config.ts` (`navigateFallback: "/app/index.html"`; denylist entries unchanged)
- Modify: `web/scripts/check-pwa.mjs` only if its manifest/icon assertions hard-code `/`
- Modify: `src/static.rs` (`SPA_ROUTE_PREFIXES`, `serve`, `is_public_shell_request`)
- Modify: `justfile` `dev` recipe comment only (Vite now serves at `http://localhost:5173/app/`)
- Test: `tests/static.rs`, `web/src/core/pwa.test.ts`

**Interfaces:**
- Produces: `pub const APP_BASE: &str = "/app/";` in `src/static.rs`. Embedded files keep their `web/dist`-relative keys; `serve` strips `/app/` before lookup.

- [ ] **Step 1: Update/extend `tests/static.rs`** (existing tests that request `/`, `/p/...`, `/assets/...` move under `/app/`) and add:

```rust
// GET /                       => 308, location "/app/"
// GET /p/site/code             => 308, location "/app/p/site/code"   (old bookmarks)
// GET /app/                    => 200 text/html, no token needed
// GET /app/p/site/code/src/index.astro => 200 index.html (dotted last segment is still a page)
// GET /app/assets/missing.js   => 404 JSON
// GET /api/projects without token => 401 (unchanged)
```

- [ ] **Step 2:** `pnpm -C web build && cargo test --test static` → FAIL.
- [ ] **Step 3:** Implement. `SPA_ROUTE_PREFIXES` becomes `["/app/"]`; public shell = `/`, any `/app/` path, or legacy `/p/` (redirect only). In `serve`: `/` and `/p/*` → `308` to `/app` + path; under `/app/`, strip the prefix, treat `p/...` as pages (the existing `looks_like_file` exemption now keys on `/app/p/`), else asset-or-404, else `index.html`. Paths outside `/app/` that are not daemon paths → JSON 404.
- [ ] **Step 4:** Update `pwa.test.ts` expectations for `navigateFallback` and run `pnpm -C web test && pnpm -C web build && cargo test --test static` → PASS.
- [ ] **Step 5:** Manually: `just dev`, open `http://localhost:5173/app/?t=<token>` → home loads, project deep link reload works.
- [ ] **Step 6:** `git commit -m "feat: serve the web app under /app/"`

### Task 3: Default origin, `--no-origin`, ready links

**Files:**
- Modify: `src/main.rs` (add `--no-origin` flag; resolve origin), `src/config.rs` (pure helpers), `src/lib.rs:99-106` (print links)
- Test: `src/config.rs` unit tests

**Interfaces:**
- Produces in `src/config.rs`:
  - `pub const DEFAULT_SITE_ORIGIN: Option<&str> = option_env!("ORSO_SITE_ORIGIN");`
  - `pub fn resolve_origin(cli: Option<String>, no_origin: bool, default: Option<&str>) -> Option<String>` — `no_origin` wins, then `cli`, then `default`; trailing `/` trimmed.
  - `pub fn ready_links(origin: Option<&str>, daemon_base: &str, ui_base: &str, token: Option<&str>) -> Vec<String>` — `daemon_base` is `http://<bound>`, `ui_base` is `ui_url` or `daemon_base`.

- [ ] **Step 1: Failing unit tests**

```rust
assert_eq!(resolve_origin(None, false, Some("https://s.dev/")), Some("https://s.dev".into()));
assert_eq!(resolve_origin(Some("https://p.s.dev".into()), false, Some("https://s.dev")), Some("https://p.s.dev".into()));
assert_eq!(resolve_origin(Some("https://p.s.dev".into()), true, Some("https://s.dev")), None);
assert_eq!(resolve_origin(None, false, None), None);

let d = "http://127.0.0.1:7801";
assert_eq!(ready_links(Some("https://s.dev"), d, d, Some("ab")),
    vec!["https://s.dev/app/?daemon=http%3A%2F%2F127.0.0.1%3A7801#t=ab", "http://127.0.0.1:7801/app/?t=ab"]);
assert_eq!(ready_links(None, d, "http://localhost:5173", Some("ab")),
    vec!["http://localhost:5173/app/?t=ab"]);
assert_eq!(ready_links(None, d, d, None),
    vec!["http://127.0.0.1:7801/app/ (auth disabled)"]);
```

- [ ] **Step 2:** `cargo test --lib config` → FAIL.
- [ ] **Step 3:** Implement helpers; `main.rs` passes `resolve_origin(origin, no_origin, DEFAULT_SITE_ORIGIN)` into `Config.origin`; `lib.rs` prints each link as `orso ready: <link>` (hosted line first). Existing e2e fixtures parse the ready line — check `e2e/fixtures/daemon.ts` and make it pick the line containing `/app/?t=`.
- [ ] **Step 4:** `cargo test` → PASS.
- [ ] **Step 5:** `git commit -m "feat(daemon): default hosted origin and print pairing link"`

### Task 4: Pairing fragment and non-loopback fallback in `config.ts`

**Files:**
- Modify: `web/src/core/config.ts` (`resolveDaemonConfig`)
- Test: `web/src/core/config.test.ts`

**Interfaces:**
- Produces: `export const DEFAULT_DAEMON_ORIGIN = "http://127.0.0.1:7800";` and `export function isLoopbackHost(hostname: string): boolean` (`localhost`, `*.localhost`, `127.0.0.1`, `::1`/`[::1]`).

- [ ] **Step 1: Failing tests**

```ts
it("consumes #t= from the hash and strips it", () => {
  // location { origin: "https://s.dev", pathname: "/app/", search: "", hash: "#t=abc" }
  // => { origin: "http://127.0.0.1:7800", token: "abc" }; replaceUrl("/app/"); stored token "abc"
});
it("hash token replaces a stored token from another daemon", () => {
  // stored { token: "old" }, hash "#t=new" => token "new"
});
it("falls back to the page origin on loopback", () => {
  // origin "http://127.0.0.1:7800", nothing stored => origin "http://127.0.0.1:7800"
});
it("falls back to DEFAULT_DAEMON_ORIGIN off loopback", () => {
  // origin "https://s.dev", nothing stored => origin "http://127.0.0.1:7800", no token
});
it("keeps other hash content", () => { /* hash "#t=abc&x=1" => replaceUrl ends with "#x=1" */ });
```

- [ ] **Step 2:** `pnpm -C web exec vitest run src/core/config.test.ts` → FAIL.
- [ ] **Step 3:** Implement: parse `location.hash.slice(1)` as `URLSearchParams`; `t` there counts like `?t=`; the rebuilt URL keeps remaining hash params. `saveDaemonConfig` already omits `origin` when it equals the page origin — keep that.
- [ ] **Step 4:** Run tests → PASS.
- [ ] **Step 5:** `git commit -m "feat(web): pair from #t= link and default to the local daemon"`

### Task 5: Safari "unsupported" state

**Files:**
- Modify: `web/src/core/config.ts` (detector), `web/src/App.vue:40-44`, `web/src/app/DaemonUnreachable.vue`
- Test: `web/src/core/config.test.ts`, `web/src/app/DaemonUnreachable.test.ts` (create, `// @vitest-environment happy-dom`)

**Interfaces:**
- Consumes: `isLoopbackHost` (Task 4).
- Produces: `export function isUnsupportedBrowser(userAgent: string, pageHostname: string): boolean` — true when UA is Safari (contains `Safari/` and not `Chrome/`, `Chromium/`, `CriOS/`, `FxiOS/`, `Edg/`) and the host is not loopback. `DaemonUnreachable` prop `reason` widens to `UnreachableReason | "unsupported"`.

- [ ] **Step 1: Failing tests**

```ts
expect(isUnsupportedBrowser(SAFARI_UA, "s.dev")).toBe(true);
expect(isUnsupportedBrowser(SAFARI_UA, "127.0.0.1")).toBe(false);
expect(isUnsupportedBrowser(CHROME_UA, "s.dev")).toBe(false);   // Chrome UA also contains "Safari/"
expect(isUnsupportedBrowser(IOS_CHROME_UA, "s.dev")).toBe(false);
// DaemonUnreachable reason="unsupported" renders a link with href "http://127.0.0.1:7800/app/"
// reason="network" renders a link to "/connect"
// reason="rejected" text contains "pairing link"
```

- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement. `App.vue` checks `isUnsupportedBrowser(navigator.userAgent, location.hostname)` once at setup; when true it renders `DaemonUnreachable` with `reason="unsupported"` and does not start the daemon socket. Copy per spec Error handling table.
- [ ] **Step 4:** Run → PASS.
- [ ] **Step 5:** `git commit -m "feat(web): route Safari on the hosted site to the local app"`

### Task 6: Preview tickets and partitioned preview cookie (daemon)

**Files:**
- Create: `src/preview_auth.rs`
- Modify: `src/lib.rs` (module), `src/state.rs` (field), `src/http/mod.rs` (route), `src/auth.rs` (`require_auth`), `src/preview.rs:255` (`wb_vite` attrs), `src/preview.rs:320` (`strip_credentials`)
- Test: `src/preview_auth.rs` unit tests, `tests/preview.rs`

**Interfaces:**
- Produces in `src/preview_auth.rs`:
  - `pub const PREVIEW_COOKIE: &str = "wb_preview";` `pub const TICKET_TTL: Duration = Duration::from_secs(60);` `pub const TICKET_PARAM: &str = "pt";`
  - `pub struct PreviewAuth` with `pub fn new() -> Self` (random 32-byte hex secret), `pub fn issue(&self) -> String`, `pub fn redeem(&self, ticket: &str) -> bool` (single-use, false after TTL; expired entries pruned on `issue`), `pub fn secret(&self) -> &str`, and `#[cfg(test)] fn issue_at(&self, now: Instant) -> String` / `redeem_at(&self, t: &str, now: Instant) -> bool` for clock control.
  - `#[derive(Clone, Copy)] pub struct ViaPreviewCookie;` — request extension set by `require_auth` when auth came from `pt`/`wb_preview`.
  - `pub fn preview_cookie_header(secret: &str) -> HeaderValue` → `wb_preview=<secret>; Path=/; HttpOnly; SameSite=None; Secure; Partitioned`.
- `AppState.preview_auth: Arc<PreviewAuth>`.
- Route: `POST /api/preview/ticket` → `200 {"ticket": "<hex>"}` (normal auth).

- [ ] **Step 1: Failing unit tests (`preview_auth.rs`)**

```rust
let a = PreviewAuth::new();
let t = a.issue();
assert_eq!(t.len(), 64);
assert!(a.redeem(&t));
assert!(!a.redeem(&t));                       // single use
let t2 = a.issue_at(now);
assert!(!a.redeem_at(&t2, now + TICKET_TTL + Duration::from_secs(1))); // expired
assert!(!a.redeem("nope"));
```

- [ ] **Step 2: Failing integration tests (`tests/preview.rs`, reuse its `env()` helper)**

```rust
// POST /api/preview/ticket with Bearer => 200 {"ticket": 64 hex}
// GET /preview/{id}/?pt=<ticket> (no other creds) => 200 page; Set-Cookie contains
//   "wb_preview=" "SameSite=None" "Secure" "Partitioned"; and the wb_vite Set-Cookie also has
//   "SameSite=None; Secure; Partitioned"
// upstream echo saw neither "pt=" in the query nor "wb_preview" in Cookie
// second GET with same ?pt= and no cookie => 401
// GET /preview/{id}/ with Cookie wb_preview=<secret> => 200
// GET /@vite/client with Cookie wb_preview=<secret>; wb_vite=<id> => forwarded (200)
// GET /api/projects with Cookie wb_preview=<secret> => 401
// GET /ws/events upgrade with Cookie wb_preview=<secret> => 401
// GET /preview/{id}/?t=<token> (legacy path) => wb_vite Set-Cookie is still "SameSite=Lax" only
```

- [ ] **Step 3:** `cargo test preview` → FAIL.
- [ ] **Step 4:** Implement. In `require_auth`, before the 401: if path starts with `/preview/` or `preview::is_vite_dev_path(path)`, accept a redeemable `pt` (then append `preview_cookie_header`) or a `wb_preview` cookie matching `secret()` via `ct_eq`; insert `ViaPreviewCookie` into request extensions. In `preview::forward`, choose the `wb_vite` cookie attributes by `req.extensions().get::<ViaPreviewCookie>()`. Extend `strip_credentials` to drop `pt` and `wb_preview` (update its existing unit tests).
- [ ] **Step 5:** `cargo test` → PASS.
- [ ] **Step 6:** `git commit -m "feat(daemon): preview tickets for cross-site preview frames"`

### Task 7: Hosted preview uses tickets (web)

**Files:**
- Modify: `web/src/core/api.ts` (add `previewTicket`; update the `previewUrl` doc comment), `web/src/modules/preview/PreviewPanel.vue:66-75` and its cross-site notice template
- Modify: `web/src/test/fakeDaemon.ts` (add `previewTicket`)
- Test: `web/src/core/api.test.ts`, `web/src/modules/preview/PreviewPanel.test.ts` (create or extend if present)

**Interfaces:**
- Consumes: `POST /api/preview/ticket` (Task 6).
- Produces: `previewTicket: () => Promise<string>` on `Api` (returns `.ticket`).

- [ ] **Step 1: Failing tests**

```ts
// api.previewTicket() POSTs `${origin}/api/preview/ticket` with Bearer and resolves "abc"
// PreviewPanel, page origin https://s.dev, daemon origin http://127.0.0.1:7800:
//   iframe src === "http://127.0.0.1:7800/preview/site/?pt=abc"; no cross-site notice rendered
// PreviewPanel same-origin: iframe src has no "pt=", previewTicket not called
// Reload button on cross-site fetches a new ticket (previewTicket called twice)
```

- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement: when `isCrossSite(window.location.origin, daemonOrigin)`, `src` is set asynchronously from a fresh ticket on mount, project change, and reload; remove the cross-site notice and `forceFrame`. On ticket fetch failure show the existing preview error state.
- [ ] **Step 4:** Run `pnpm -C web test` → PASS.
- [ ] **Step 5:** `git commit -m "feat(web): live preview from the hosted app via preview tickets"`

### Task 8: `site/` (landing, docs, connect) + Worker + CI

**Files:**
- Create: `site/` — copy of `templates/astro-marketing/` minus `node_modules/`, `dist/`, `.astro/`, `.github/`, `scripts/orso-content*`, `AGENTS.md`, `DESIGN.md`; `astro.config.mjs` without `PUBLIC_BASE`/`ORSO_PROXY_PORT`, plus `@astrojs/starlight` with docs at `/docs`.
- Create: `site/src/pages/connect.astro`, `site/src/content/docs/docs/index.md` (+ `install.md`, `pairing.md`), `site/worker.js`, `site/worker.test.mjs`
- Modify: `site/wrangler.toml`, `justfile` (`build-site`, `setup`), `.github/workflows/site.yml` (create, from `templates/astro-marketing/.github/workflows/deploy.yml`), `.github/workflows/release.yml` (`ORSO_SITE_ORIGIN: ${{ vars.SITE_ORIGIN }}` on the cargo build step), `.github/workflows/ci.yml` (job that runs `just build-site` and `node --test site/worker.test.mjs`)

**Interfaces:**
- Produces: `site/worker.js` default export `{ fetch(request, env) }`, using `env.ASSETS.fetch`.

- [ ] **Step 1: Failing worker test (`node:test`, stub `env.ASSETS.fetch` from a path→status map)**

```js
// GET /app/p/x/code             asset 404 => serves /app/index.html (200)
// GET /app/p/x/code/src/a.astro asset 404 => serves /app/index.html (dotted last segment under /app/p/)
// GET /app/assets/missing.js    asset 404 => 404 (not index.html)
// GET /app/assets/a.js          asset 200 => passed through
// GET /pricing                  asset 404 => 404 page passed through (no SPA fallback outside /app/)
```

- [ ] **Step 2:** `node --test site/worker.test.mjs` → FAIL.
- [ ] **Step 3:** Implement `worker.js`; `wrangler.toml`: `main = "worker.js"`, `[assets] directory = "./dist"`, `binding = "ASSETS"`, `not_found_handling = "404-page"`, `run_worker_first = ["/app/*"]`.
- [ ] **Step 4:** Content: landing copy adapted from the template home; `/connect` = install, `orso serve`, open the printed pairing link, Safari note; docs pages mirror `/connect` plus `--origin`/`--no-origin`.
- [ ] **Step 5:** `justfile` `build-site`: `pnpm -C web build && npm --prefix site ci && npm --prefix site run build && rm -rf site/dist/app && cp -R web/dist site/dist/app`. Run it → `site/dist/index.html`, `site/dist/docs/index.html`, `site/dist/app/index.html` exist.
- [ ] **Step 6:** `node --test site/worker.test.mjs` → PASS; `git status` shows no change under `templates/astro-marketing/`.
- [ ] **Step 7:** `git commit -m "feat(site): landing, docs and hosted /app on Cloudflare"`

### Task 9: e2e — hosted origin pairs and previews

**Files:**
- Create: `e2e/specs/hosted-app.spec.ts`
- Modify: `e2e/fixtures/daemon.ts` (accept extra args, expose both ready links)

**Interfaces:**
- Consumes: Tasks 1–7 (and `web/dist` built with base `/app/`).

- [ ] **Step 1: Write the spec**

```ts
// Serve web/dist under /app/ from a static server on http://localhost:<port> (the "public"
//   origin: a different site from 127.0.0.1) with SPA fallback for /app/*.
// Start daemon with --origin http://localhost:<port>.
// page.goto(the hosted ready link printed by the daemon: `.../app/?daemon=...#t=...`)
//   => URL query and hash cleared; home lists projects.
// Open a project => preview iframe src starts with "http://127.0.0.1:" and contains "?pt=";
//   frameLocator("iframe").locator("body") contains the template's home heading.
// localStorage "orso.daemon" has the token.
```

- [ ] **Step 2:** `just e2e` → new spec FAILs before Tasks 1–7 are in, PASSes after (Chromium).
- [ ] **Step 3:** `git commit -m "test(e2e): hosted app pairs with the daemon and renders the preview"`
