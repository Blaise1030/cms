# Preview checks (SEO / AEO / performance) — design

Date: 2026-11-14  
Status: draft for review  
Scope: `packages/astro` (rules, CI runner, in-page panel), template sync + deploy workflow.  
Supersedes (product approach): `docs/superpowers/specs/2026-10-31-seo-audit-design.md` — that design’s daemon crawl + CMS **SEO** tab are not the path. Rule *ideas* from it are reused.

## Problem

Writers need publish-oriented feedback on the page they are looking at, and the same
bar must run when they Share preview / Publish so GitHub checks stay trustworthy.
Third-party Astro SEO plugins audit `dist/` for developers and do not plug into the
orso preview or agent loop. Duplicating rules in the daemon and in CI will drift.

## Goal

One rule pack owned by the **orso Astro plugin**, used in two places:

1. **In-page panel** inside the rendered Astro app during `astro dev` (the CMS preview
   iframe). This is the only writer-facing checks UI — no CMS SEO tab.
2. **CI** via the same plugin on `astro:build:done`, so `npm run build` in the site
   workflow produces a consistent check next to Deploy.

Success: a writer opens the panel on `/pricing/`, sees “missing meta description”,
fixes it (or asks the agent from the panel); later Share preview / Publish runs the
same rule against `dist/` and the PR checks strip stays green only when those rules
pass at the configured severity.

## Decisions

| # | Decision |
| --- | --- |
| 1 | Orso owns the feature through `packages/astro` (`orso()`), not a third-party SEO integration and not a CMS-only Rust crawler. |
| 2 | Writer UI is **only** an in-page panel in the preview (injected into the Astro app). No `SeoPanel` route/tab in the CMS web app for v1. |
| 3 | CI checks live in the orso plugin (`astro:build:done` over `dist/`). The existing site `deploy.yml` keeps calling `npm run build`; the plugin fails or warns as part of that job. |
| 4 | Performance belongs in the **same** in-page panel (heuristics first), not a separate CMS module. |
| 5 | One shared rule module; both runners call it. Findings use stable `rule` ids so copy and severity match in panel and CI logs. |
| 6 | Panel injects in **`dev` only** (like today’s `bridge.js`). Production / Workers preview deploys do not ship the panel UI. |
| 7 | Default CI gate: **errors fail the build**; warnings print and do not fail. Configurable later; v1 can hard-code that split. |
| 8 | No Lighthouse / real CWV lab run in v1. Static HTML + light resource heuristics only. Full Lighthouse can be a later optional runner. |
| 9 | Steal check *ideas* from packages such as `@nuasite/checks` / `@casoon/astro-post-audit`; do not add them as template dependencies. |

## Architecture

```
packages/astro/src/
  checks/
    rules.js          # pure: HTML string (+ optional page URL) → Finding[]
    site.js           # cross-page / site files (robots, sitemap, duplicates)
    report.js         # shared formatting (terminal + panel JSON shape)
  panel/
    panel.js          # in-page UI + “run on this page” / summary
  index.js            # orso(): inject panel+bridge in dev; audit on build:done
  bridge.js           # existing pick/appearance; optional agent postMessage hook
```

Template copy stays the sync story: `just sync-orso-integration` →
`templates/astro-marketing/integrations/orso/`.

### Runner A — in-page panel (writer)

- Injected with `injectScript` during `astro:config:setup` when `command === "dev"`,
  same gate as `PREVIEW_BRIDGE`. May ship as one bundle with the bridge or a sibling
  IIFE; must not run meaningful work until the user opens the panel (or an explicit
  refresh), so preview stays snappy.
- Scope for v1: **current document** (`document.documentElement.outerHTML` / parsed
  DOM). Site-wide rules that need other pages (duplicate titles, crawl) are **CI /
  build only** in v1, or shown as “run a build to check site-wide” notes — do not
  crawl the whole site from the browser panel in v1.
- Panel chrome: collapsed trigger (e.g. corner control) → expanded drawer with
  summary counts and a list of findings for this page. Group by severity.
- Optional v1 stretch: **Fix with agent** posts a plain-language prompt to the parent
  via the existing preview `postMessage` channel (extend `bridge.js` / `frameBridge`),
  reusing the agent outbox. If that bridge work slips, the panel still ships as
  read-only findings.

### Runner B — CI (build)

- Hook `astro:build:done` (or equivalent that sees the final `dir` / pages).
- Walk HTML under the build output; run page rules per file; run site rules once
  (`robots.txt`, `sitemap.xml`, duplicate title/description across pages, broken
  internal links resolved against files on disk).
- Print a grouped report to stdout. Exit non-zero if any **error**-severity finding
  exists. Warnings never fail v1.
- No network fetch of external URLs. No headless browser.

CMS already aggregates GitHub check-runs on Share preview / Publish; a failed build
surfaces there without a new CMS findings UI.

## Findings model

Shared shape (panel JSON and CI log both derive from this):

```ts
type Finding = {
  rule: string;       // stable id, e.g. "description-missing"
  severity: "error" | "warning";
  page: string;       // path like "/" or "/pricing/" ; "." for site-wide
  message: string;    // plain language, no jargon in the headline
  detail?: string;    // optional examples (srcs, missing tags)
  count?: number;
};
```

CI may add file paths relative to `dist/` in the log line; the panel does not need
`dist` paths.

## Rules (v1)

### Per page (panel + CI)

| Rule | Severity | Fires when |
| --- | --- | --- |
| `title-missing` | error | No / blank `<title>` |
| `title-length` | warning | Title over 60 characters |
| `description-missing` | warning | No / blank meta description |
| `description-length` | warning | Description over 160 characters |
| `heading` | error / warning | No `h1` (error); more than one (warning) |
| `image-alt` | warning | `img` missing `alt` (empty `alt=""` allowed) |
| `html-lang` | warning | `<html>` missing non-blank `lang` |
| `social-preview` | warning | Missing `og:title`, `og:description`, or `og:image` |
| `noindex` | warning | Robots meta contains `noindex` |
| `canonical-missing` | warning | No `link[rel=canonical]` |
| `json-ld-missing` | warning | No JSON-LD script (document types that expect it can be tuned later; v1: warn if none on the page) |
| `json-ld-invalid` | error | JSON-LD present but not parseable JSON |

### AEO / answer-engine oriented (per page)

| Rule | Severity | Fires when |
| --- | --- | --- |
| `json-ld-type` | warning | JSON-LD present but no useful `@type` among a small allowlist (`Organization`, `WebSite`, `WebPage`, `Article`, `BlogPosting`, `FAQPage`, `HowTo`, `BreadcrumbList`) |
| `faq-structure` | warning | Visible FAQ pattern (details/summary or heading+Q copy heuristics) without `FAQPage` JSON-LD — keep heuristic conservative; prefer skip over false positives |

### Performance heuristics (panel + CI; not Lighthouse)

| Rule | Severity | Fires when |
| --- | --- | --- |
| `img-dimensions` | warning | `img` lacking both `width` and `height` |
| `sync-script` | warning | Render-blocking classic `script src` in `head` without `defer`/`async`/`type=module` |
| `huge-inline` | warning | Inline `<script>` or `<style>` over a size cap (e.g. 50 KiB) |

Panel may also show a simple **resource summary** for the current document (counts of
scripts, stylesheets, images, approx. HTML size). That summary is informational and
not a CI-failing rule unless mapped to a rule above.

### Site-wide (CI only in v1)

| Rule | Severity | Fires when |
| --- | --- | --- |
| `broken-link` | error | Internal `a[href]` target missing from `dist` |
| `duplicate-title` | warning | Same non-blank title on multiple pages |
| `duplicate-description` | warning | Same non-blank description on multiple pages |
| `robots-missing` | warning | No `robots.txt` in output |
| `sitemap-missing` | warning | No `sitemap.xml` in output |

## Panel UX (preview)

- Collapsed by default; does not cover the page until opened.
- Shows findings for the **current URL** only; re-run on navigation or via a refresh
  control (listen to Astro view transitions / `popstate` / bridge `path` if needed).
- Copy matches CI rule messages where possible.
- Visual language: compact developer-tool drawer, readable in light and dark preview
  appearance modes (respect existing appearance bridge).
- Must not break element pick mode: when pick is active, panel interaction is disabled
  or the panel is collapsed.

## CI / template

- No new workflow file required if build already runs in
  `templates/astro-marketing/.github/workflows/deploy.yml`. Document that SEO/AEO
  errors fail `npm run build` via `orso()`.
- After implementation, sync the integration into the template so new and updated
  projects get the hooks.
- Forks / sites that vendor an old `integrations/orso` without checks will not get CI
  gating until they sync — acceptable; scaffolding and sync remain the distribution
  path (no separate npm publish required for v1 beyond the existing copy model).

## Relationship to the CMS app

- **No** new project tab, route, or `POST /api/seo/audit` for this feature in v1.
- Preview iframe already hosts the panel; Share preview / Publish already show check
  status from GitHub.
- Optional later: richer check annotations in the PR bar; out of scope here.

## Testing

**Plugin unit**

- Table-driven tests for each rule over HTML fixtures (missing title, long description,
  bad JSON-LD, image without alt, etc.).
- Build-hook test: temp `dist/` with one good and one bad page → expect error exit and
  finding ids.
- Integration test updates: `astro build` is no longer a total no-op for checks; assert
  the hook runs and that **dev** still injects the panel/bridge while **build** does
  not inject panel UI into output HTML.

**Manual / e2e (lightweight)**

- Open CMS preview, open panel, confirm a known missing description appears.
- Optional e2e can wait until the panel has stable `data-testid`s.

## Out of scope

- CMS SEO tab, daemon HTML crawler API, saved report history.
- Lighthouse, lab CWV, full axe/Playwright a11y (may return as a later runner).
- External link checking, Search Console, rankings.
- Shipping the panel into production or Workers preview deployments.
- Editing the starter template’s default SEO assets (robots/sitemap/OG) — follow-up so
  new projects start clean under the new CI rules.
- Requiring a hosted orso service for CI.

## Migration note

Mark `2026-10-31-seo-audit-design.md` as **superseded for product UX and ownership**
(point here). Keep it as historical reference for rule copy and severity choices that
this spec reuses.
