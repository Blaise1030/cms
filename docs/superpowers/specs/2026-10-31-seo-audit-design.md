# SEO audit for the local preview — design

Date: 2026-10-31
Status: superseded for product UX and ownership — see `2026-11-14-preview-checks-design.md` (rules live in `packages/astro`; writer UI is an in-page preview panel; CI runs on `astro:build:done`). Kept as historical reference for rule copy and severities.
Scope (original): `src/seo/`, `src/http/seo.rs`, one daemon route, one web tab. No template change.

## Problem

orso can build, preview and publish a site, but nothing tells the writer whether the
pages they are about to publish are complete. Missing titles, empty descriptions, images
without alt text, broken links and missing `robots.txt`/`sitemap.xml` are invisible in the
preview, and the writer is not expected to know they matter. The agent can fix these, but
only if it is told what is wrong.

## Goal

A **SEO tab** that audits the running local preview on demand and lists plain-language
issues, each with **Open in preview** and **Fix with agent**.

Success: a writer publishes a page, opens SEO, runs the audit, sees "The pricing page has
no description", clicks **Fix with agent**, and the agent fixes it in a scoped turn.

## Decisions

| # | Decision |
| --- | --- |
| 1 | Audit the dev server at `127.0.0.1:<port>/preview/{id}/`, the same HTML the preview frame shows. |
| 2 | Crawl the site over HTTP and parse HTML in Rust. No browser, no Playwright, no Node dependency. |
| 3 | Run on demand only. No background crawl, no saved reports, no history. |
| 4 | Continue past individual page failures; only a not-ready preview or a failed home-page request aborts the run. |
| 5 | Rules are deterministic and publish-oriented. No Lighthouse scores, rankings or live-web checks. |
| 6 | Fix prompts are built by the daemon so the UI and any future agent tool send the same text. |
| 7 | The starter template is not changed by this work; its own SEO gaps are follow-ups. |

## Backend

### Module layout

- `src/seo/mod.rs` — public types (`Report`, `Page`, `Finding`, `Severity`), the
  `audit(state, project)` entry point, source-file mapping, and fix-prompt construction.
- `src/seo/crawl.rs` — URL normalization, link extraction, the fetch loop, limits.
- `src/seo/rules.rs` — pure functions over one parsed page plus a cross-page pass; unit
  tests live here.

Dependencies: `scraper` for HTML parsing (new). `reqwest`, `futures-util` and `tokio` are
already in use. No browser dependency.

### Fetching

- Resolve the project through `AppState::projects`; refuse unless
  `devserver == DevserverStatus::Ready`, with `DEVSERVER_ERR` and the copy
  "Start the preview to run the audit" (status 409).
- Base URL: `http://127.0.0.1:{port}/preview/{id}/`. The dev server already runs with
  `PUBLIC_BASE=/preview/{id}/` (`src/devserver.rs`), so this is the canonical rendered
  site. Fetch directly, not through the daemon proxy: no auth cookie is needed on
  loopback, and the audit must not depend on the caller's session.
- A dedicated `reqwest::Client` per run: `no_proxy()`, 5 s timeout, redirects limited to 3,
  and a custom redirect policy that rejects any origin other than `127.0.0.1:{port}`.
  Responses are read up to 2 MiB and parsed from that prefix.
- Only `text/html` responses are parsed. Other internal URLs are skipped.

### Crawl

- Breadth-first from the base URL. The root page is always fetched first; a transport
  failure there aborts the run with `DEVSERVER_ERR`. A non-2xx root is reported as
  `page-status` and the crawl stops.
- Links come from `<a href>` and are resolved against the current URL. Keep only
  same-origin URLs whose path starts with the base prefix.
- Normalize for identity: strip the base prefix, drop query and fragment, collapse
  duplicate slashes, and treat `/x` and `/x/` as the same page. The stored page path is
  project-relative (`/` for home, `/pricing/` for a page).
- Skip assets by extension (css, js, map, json, xml, txt, images, fonts, pdf, zip, mp4,
  webm), and the dev-server internals (`/__orso`, `/@vite`, `/@id`, `/@fs`,
  `/node_modules`, `/src`). Do not follow `mailto:`, `tel:`, `javascript:` or bare `#`.
- A URL that responds 2xx is a page: it is listed in `pages` and the page rules run. A URL
  that responds non-2xx, or whose request fails, is cached as a broken target and is
  reported as `broken-link` on every page that links to it. Broken targets never appear
  in `pages`.
- Limits: 50 pages, 8 concurrent requests, 30 s total budget. Hitting either cap ends the
  crawl and sets `truncated` (page cap) or `timedOut` (budget) on the report; findings
  collected so far are returned.
- Site-wide checks fetch `/preview/{id}/robots.txt` and `/preview/{id}/sitemap.xml` once.

### Rules

Element rules run per page; duplicate rules run once all pages are known.

| Rule | Scope | Severity | Fires when |
| --- | --- | --- | --- |
| `page-status` | root | error | The home page is not 2xx |
| `title-missing` | page | error | No `<title>`, or it is blank |
| `title-length` | page | warning | Title is over 60 characters |
| `description-missing` | page | warning | No `meta[name=description]`, or it is blank |
| `description-length` | page | warning | Description is over 160 characters |
| `heading` | page | error / warning | No `<h1>` (error), or more than one (warning) |
| `image-alt` | page | warning | Images missing the `alt` attribute; one finding with a count |
| `html-lang` | page | warning | `<html>` has no non-blank `lang` |
| `social-preview` | page | warning | `og:title`, `og:description` or `og:image` is missing; one finding listing the missing tags |
| `noindex` | page | warning | A robots meta tag contains `noindex` |
| `broken-link` | page | error | An internal link target returned a non-2xx status or failed to load; one finding per source page, up to 5 targets listed |
| `duplicate-title` | cross-page | warning | More than one page shares the same non-blank title; reported on each page |
| `duplicate-description` | cross-page | warning | More than one page shares the same non-blank description; reported on each page |
| `robots-missing` | site | warning | `robots.txt` is not 200 |
| `sitemap-missing` | site | warning | `sitemap.xml` is not 200 |

Element findings carry `count` and, for repeated items, a `detail` string with the first
five examples (image srcs, link targets, missing tags). Each rule produces at most one
finding per page. Cross-page and site findings are anchored to the page where they were
observed (the first page with that title/description, or `/`).

### Source mapping

`file` is best-effort and may be `null`.

- Static routes: try `<root>/src/pages{path}index.astro` and `<root>/src/pages{path}.astro`
  (`/` → `src/pages/index.astro`). `<root>` comes from `ProjectSettings::load` so monorepo
  layouts work.
- Dynamic and content routes are not guessed. The URL is still named by the fix prompt, so
  the agent can locate them.

### Fix prompt

Built by the daemon for each finding; the UI sends it verbatim as a submitted chat message.

```text
The SEO audit found an issue on /pricing/: the page has no meta description.

Fix it in the site and keep the existing copy and design. File: src/pages/pricing.astro
Rely on HMR; do not run a build or publish.
```

The `File:` sentence is omitted when `file` is `null`, so content and dynamic routes still
get a URL-anchored request.

## API

`POST /api/seo/audit`, body `{ "project": "site-1" }`. Authenticated like every daemon
route. Synchronous; the 30 s crawl budget bounds the response time.

```json
{
  "project": "site-1",
  "durationMs": 812,
  "truncated": false,
  "timedOut": false,
  "summary": { "pages": 7, "errors": 3, "warnings": 9 },
  "pages": [
    { "path": "/", "status": 200, "title": "Acme" },
    { "path": "/pricing/", "status": 200, "title": "Pricing" }
  ],
  "findings": [
    {
      "rule": "description-missing",
      "severity": "warning",
      "page": "/pricing/",
      "message": "The pricing page has no description.",
      "detail": null,
      "count": 1,
      "file": "src/pages/pricing.astro",
      "fix": "The SEO audit found an issue on /pricing/: …"
    }
  ]
}
```

- `pages` is sorted by path; `findings` is sorted by page, then errors before warnings,
  then rule.
- Errors: `ENOENT` for an unknown project, `DEVSERVER_ERR` (409) when the preview is not
  ready, `BADREQ` for a malformed body.

## Web

### Route and tab

- `ProjectTab` gains `"seo"`; route `project-seo` at `/p/:id/seo`; `projectTabFromRoute`
  maps it. Files under `/p/` already fall back to the SPA shell.
- `ProjectRail.vue` adds a **SEO** tab with the `ScanSearch` icon, after **History**.

### `web/src/modules/seo/`

- `store.ts`: `report`, `running`, `error`, `ranFor`; `run(projectId)` calls the API and
  keeps the last report for that project; `reset()` on project change.
- `SeoPanel.vue`:
  - **Idle**: a short explanation and a **Run audit** button. Disabled with the reason
    when the preview is not ready.
  - **Running**: spinner and "Checking your pages…".
  - **Report**: summary chips (pages, errors, warnings) and a per-page list. Each page
    header shows its path and, on hover, **Open in preview**, which sets the preview
    store's path for the project and navigates to the Preview tab.
  - Each finding shows a severity icon, its message, and `detail` when present. **Fix with
    agent** sends `{ kind: "text", text: finding.fix, submit: true }` through
    `useAgentOutboxStore`; `ProjectPage` already opens the chat on `outbox.sent`.
  - **Clean**: "No SEO issues found." **Truncated/timeout**: a footer note explaining the
    cap and offering **Run audit** again.
  - If the preview restarts after a run, show "Preview restarted — run the audit again"
    above the report. Do not auto-run.
- `api.ts` / `core/types.ts`: request and report types, matching the daemon JSON.

The panel is read-only: it never edits the site. The agent makes every fix.

## Copy

| Moment | Text |
| --- | --- |
| Tab | "SEO" |
| Idle lead | "Check the pages you're about to publish." |
| Run button | "Run audit" |
| Running | "Checking your pages…" |
| Preview down | "Start the preview to run the audit." |
| Clean | "No SEO issues found." |
| Truncated | "Stopped after 50 pages." |
| Timed out | "Stopped after 30 seconds." |
| Stale | "Preview restarted — run the audit again." |
| Fix action | "Fix with agent" |
| Page action | "Open in preview" |

Finding messages are written by the daemon in plain language, with no rule names, file
paths in the headline, or git vocabulary.

## Test ids

| Id | Meaning |
| --- | --- |
| `seo-panel` | Module root |
| `seo-run` | Run button |
| `seo-summary` | Summary chips |
| `seo-page` | A page group, with `data-page-path` |
| `seo-finding` | A finding row, with `data-rule` |
| `seo-open` | Open in preview |
| `seo-fix` | Fix with agent |
| `seo-note` | Truncated / timed-out / stale note |

## Edge cases

- **Preview not ready**: the panel explains why and offers the existing preview restart
  path; no request is made until ready.
- **Preview restarts mid-run**: in-flight failures become `broken-link` findings on the
  pages that linked to them, and the run still returns. A transport failure on the home
  page is the only abort.
- **Monorepo (`orso.json` root)**: source mapping resolves under the configured root;
  nothing else changes.
- **Nested/static routes**: `/x` and `/x/` collapse to one page; links with queries and
  fragments are fetched once.
- **Large pages**: responses are read up to 2 MiB; the prefix is parsed, so a truncated
  page can still produce element findings.
- **Dynamic routes** (blog posts, docs): discovered by crawling links, which is exactly
  what a reader sees. Not discovered if nothing links to them, which is also a finding a
  writer can act on later (sitemap work, out of scope).
- **No internal links**: the report contains the home page only, which is still useful.
- **Site-wide files**: checked against the preview base, so a missing `robots.txt` and
  `sitemap.xml` are reported as warnings even for the starter template. Fixing the starter
  template is a separate change.

## Testing

**Rust**

- `rules.rs`: table-driven tests for each rule, including boundary lengths, a blank title,
  two `h1`s, `alt=""` (passes; only a missing attribute fails), `noindex`, duplicate titles
  across pages, and the five-example cap on `detail`.
- `crawl.rs`: base-prefix normalization, query/fragment stripping, asset and dev-internal
  skips, `/x` vs `/x/` dedupe, and the 50-page cap.
- Integration (`tests/seo.rs`): spin an axum fixture server on a free loopback port
  with a good page, a 404 link, a missing description and a duplicate title; assert the
  report shape and ordering. A second test asserts `DEVSERVER_ERR` when the project's
  devserver is not ready.
- No test hits the network or a real Astro install.

**Web**

- `store.test.ts`: success, HTTP error, project switch reset, preview-down error copy.
- `SeoPanel.test.ts`: idle/running/report/clean/truncated states; Open in preview writes
  the preview path and navigates; Fix with agent pushes the submitted outbox message.

**E2E**

- `e2e/specs/seo-audit.spec.ts`: in the cloned fixture project, write an `index.html` with
  no description, an image without `alt`, and one broken internal link (on a change, then
  switch to it so the audit runs on that tree); open **SEO**, run, and expect the three
  findings. Click **Fix with agent** and expect the chat composer to hold the prompt.

## Out of scope

- Lighthouse scores, Core Web Vitals, accessibility and performance checks.
- JS-rendered content, external-link checking and live-site crawling.
- Saved reports, trends, scheduled audits and rank/Search Console data.
- Blocking publish on findings. The audit informs; it does not gate.
- Editing or auto-fixing the template. Follow-up: give the starter `robots.txt`,
  `sitemap.xml` and OG tags so new projects start clean.