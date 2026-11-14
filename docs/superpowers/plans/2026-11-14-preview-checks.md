# Preview Checks (SEO / AEO / Performance) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship one orso-astro rule pack that powers an in-page preview checks panel (`astro dev`) and fails `astro build` on error-severity findings so Share preview / Publish CI stays consistent.

**Architecture:** Pure DOM-shaped `auditPage(doc)` rules shared by the browser panel and Node CI. Node uses `linkedom` only to parse `dist/**/*.html` and run site-wide rules. `orso()` injects the panel in `dev` and runs `auditDist` on `astro:build:done`. No CMS SEO tab and no daemon crawl.

**Tech Stack:** `packages/astro` ESM, `linkedom`, Astro integration hooks, Node `node:test`, template sync via `scripts/sync-orso-integration.sh`.

**Spec:** `docs/superpowers/specs/2026-11-14-preview-checks-design.md`

## Global Constraints

- Writer UI is **only** the in-page panel in preview — no CMS tab, route, or `POST /api/seo/audit`.
- Panel injects in **`dev` only**; production / Workers HTML must not include the panel.
- CI and panel share stable `rule` ids and plain-language `message` strings from the spec.
- Errors fail the build; warnings print and do not fail (v1 hard-coded).
- No Lighthouse, axe, external URL fetches, or live-site crawling.
- Site-wide rules (duplicates, broken links, robots/sitemap) run in **CI only** in v1.
- Template continues to vendor `integrations/orso/` (no `orso-astro` npm dependency on sites).
- “Fix with agent” via `postMessage` is **out of this plan** (spec stretch / follow-up).

## Review Focus

- `astro build` must not inject panel/bridge scripts into emitted HTML even when checks run.
- Pages under a non-root Astro `base` must still produce `page` paths like `/pricing/` for findings.
- Malformed JSON-LD must yield `json-ld-invalid` (error) without aborting the rest of the page audit.
- Empty `alt=""` must **not** fire `image-alt`; missing `alt` must.
- A dist with only warnings must exit 0; one error must exit non-zero.

## File structure

| File | Responsibility |
|------|----------------|
| `packages/astro/src/checks/findings.js` | `Finding` shape helpers / severity sort |
| `packages/astro/src/checks/page.js` | Per-page rules → `auditPage(doc, { path })` |
| `packages/astro/src/checks/site.js` | Site-wide rules over `{ pages, files }` |
| `packages/astro/src/checks/audit.js` | `auditDist(dir)` walk + parse + combine |
| `packages/astro/src/checks/report.js` | Terminal report string + `exitCode(findings)` |
| `packages/astro/src/panel/panel.js` | In-page drawer UI; calls `auditPage(document, …)` |
| `packages/astro/src/index.js` | Dev inject + `astro:build:done` |
| `packages/astro/package.json` | Add `linkedom` dependency |
| `packages/astro/test/checks-page.test.mjs` | Page rule fixtures |
| `packages/astro/test/checks-site.test.mjs` | Site rule fixtures |
| `packages/astro/test/checks-audit.test.mjs` | Temp `dist/` audit + exit code |
| `packages/astro/test/integration.test.mjs` | Inject / build-hook behavior |
| `scripts/sync-orso-integration.sh` | Copy `checks/` + `panel/` tree |
| `templates/astro-marketing/package.json` | Add `linkedom` |
| `templates/astro-marketing/integrations/orso/` | Synced copy |
| `packages/astro/README.md` + template `AGENTS.md` | Document build-time checks |

---

### Task 1: Per-page rules (`auditPage`)

**Files:**
- Create: `packages/astro/src/checks/findings.js`, `packages/astro/src/checks/page.js`
- Test: `packages/astro/test/checks-page.test.mjs`
- Modify: `packages/astro/package.json` (add `linkedom`)

**Interfaces:**
- Produces:
  - `auditPage(doc: { querySelector, querySelectorAll }, opts: { path: string }) -> Finding[]`
  - `Finding = { rule: string, severity: "error" \| "warning", page: string, message: string, detail?: string, count?: number }`
  - Rules implemented (exact ids from spec): `title-missing`, `title-length`, `description-missing`, `description-length`, `heading`, `image-alt`, `html-lang`, `social-preview`, `noindex`, `canonical-missing`, `json-ld-missing`, `json-ld-invalid`, `json-ld-type`, `faq-structure`, `img-dimensions`, `sync-script`, `huge-inline` (50 KiB cap).
- Consumes: `linkedom` `parseHTML` in tests only to build `doc`.

- [ ] **Step 1: Write failing tests** for the table below (one HTML fixture each, assert `rule` + `severity`)

| Fixture idea | Expect |
| --- | --- |
| no `<title>` | `title-missing` error |
| title 61 chars | `title-length` warning |
| no meta description | `description-missing` warning |
| description 161 chars | `description-length` warning |
| no h1 | `heading` error |
| two h1s | `heading` warning |
| `<img>` without `alt` | `image-alt` warning |
| `<img alt="">` | no `image-alt` |
| no `lang` | `html-lang` warning |
| missing og tags | `social-preview` warning |
| `noindex` | `noindex` warning |
| no canonical | `canonical-missing` warning |
| no JSON-LD | `json-ld-missing` warning |
| broken JSON-LD | `json-ld-invalid` error |
| JSON-LD `@type: Thing` only | `json-ld-type` warning |
| FAQ `<details>` without FAQPage LD | `faq-structure` warning (or skip if heuristic abstains — assert either finding or empty, document choice in test name) |
| img without width/height | `img-dimensions` warning |
| sync script in head | `sync-script` warning |
| inline script > 50 KiB | `huge-inline` warning |

- [ ] **Step 2: Run tests — expect FAIL**

Run: `npm test --prefix packages/astro -- test/checks-page.test.mjs`

- [ ] **Step 3: Implement `auditPage` + findings helpers**

Use only DOM APIs the browser and linkedom share. Cap `huge-inline` at **50 KiB**. Allowlist for `json-ld-type`: `Organization`, `WebSite`, `WebPage`, `Article`, `BlogPosting`, `FAQPage`, `HowTo`, `BreadcrumbList` (walk `@graph` arrays). Title/description length caps: **60** / **160**.

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add packages/astro/package.json packages/astro/package-lock.json \
  packages/astro/src/checks/findings.js packages/astro/src/checks/page.js \
  packages/astro/test/checks-page.test.mjs
git commit -m "feat(orso-astro): add per-page preview check rules"
```

---

### Task 2: Site rules + `auditDist` + report

**Files:**
- Create: `packages/astro/src/checks/site.js`, `packages/astro/src/checks/audit.js`, `packages/astro/src/checks/report.js`
- Test: `packages/astro/test/checks-site.test.mjs`, `packages/astro/test/checks-audit.test.mjs`

**Interfaces:**
- Consumes: `auditPage` from Task 1.
- Produces:
  - `auditSite({ pages: { path, title?, description? }[], files: Set<string>, internalLinks: { from, href }[] }) -> Finding[]` for `broken-link`, `duplicate-title`, `duplicate-description`, `robots-missing`, `sitemap-missing`
  - `auditDist(dir: string, opts?: { base?: string }) -> Promise<{ findings: Finding[], pages: { path: string }[] }>`
  - `formatReport(findings) -> string`
  - `exitCode(findings) -> 0 | 1` (1 if any `severity === "error"`)
  - `base` defaults to `"/"`; when set (e.g. `/preview/site-1/`), internal hrefs under that prefix resolve to the same dist files as unprefixed site paths.

- [ ] **Step 1: Write failing tests**

`checks-site`: duplicate titles; missing robots/sitemap; broken internal link.  
`checks-audit`: temp dir with `index.html` (good) + `pricing/index.html` (no description) + link to missing `/gone/` → expect `description-missing` + `broken-link`; `exitCode` 1 when an error exists, 0 when only warnings; with `base: "/preview/x/"`, an href `/preview/x/pricing/` to an existing file must **not** report `broken-link`.

- [ ] **Step 2: Run tests — expect FAIL**

Run: `npm test --prefix packages/astro -- test/checks-site.test.mjs test/checks-audit.test.mjs`

- [ ] **Step 3: Implement site audit + dist walk + report**

Walk `**/*.html` under `dir`. Derive `path` from file path (`index.html` → `/`, `pricing/index.html` → `/pricing/`). Resolve internal links against output files (ignore `http(s):`, `mailto:`, `tel:`, `javascript:`, `#`). Check `robots.txt` / `sitemap.xml` at dist root.

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add packages/astro/src/checks/site.js packages/astro/src/checks/audit.js \
  packages/astro/src/checks/report.js packages/astro/test/checks-site.test.mjs \
  packages/astro/test/checks-audit.test.mjs
git commit -m "feat(orso-astro): audit dist and format CI check report"
```

---

### Task 3: Wire `orso()` build hook

**Files:**
- Modify: `packages/astro/src/index.js`
- Modify: `packages/astro/test/integration.test.mjs`
- Modify: `packages/astro/README.md` (build no longer a no-op for checks)

**Interfaces:**
- Consumes: `auditDist`, `formatReport`, `exitCode` from Task 2.
- Produces: `astro:build:done` hook that audits `dir` (Astro’s output dir from the hook), prints the report, and `process.exitCode = 1` (or throws) when `exitCode(findings) === 1`. Still **no** script inject when `command === "build"`.

- [ ] **Step 1: Write failing integration tests**

Assert: `command: "build"` injects nothing; calling the new `astro:build:done` with a fixture dir that has an error sets a failing exit / thrown error; a warnings-only fixture does not fail.

- [ ] **Step 2: Run tests — expect FAIL**

Run: `npm test --prefix packages/astro -- test/integration.test.mjs`

- [ ] **Step 3: Implement `astro:build:done` in `orso()`; update README**

Replace “does nothing during `astro build`” with a short “runs preview checks on `dist/`” note.

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add packages/astro/src/index.js packages/astro/test/integration.test.mjs packages/astro/README.md
git commit -m "feat(orso-astro): run preview checks on astro build"
```

---

### Task 4: In-page panel (dev only)

**Files:**
- Create: `packages/astro/src/panel/panel.js`
- Modify: `packages/astro/src/index.js`
- Modify: `packages/astro/test/integration.test.mjs`

**Interfaces:**
- Consumes: `auditPage` from Task 1 (panel imports it; Vite resolves via absolute `import.meta.url` inject).
- Produces: collapsed-by-default drawer; on open, runs `auditPage(document, { path })` for the current path; lists findings; refresh control; `data-testid="orso-checks-panel"` on root. Does not run until opened. Collapses / ignores input while element pick is active if pick overlay id `__orso-picker` is present (spec: must not break pick mode).

- [ ] **Step 1: Write failing integration test**

`command: "dev"` injects bridge **and** a page/panel entry that references checks (assert injected content or second inject matches `orso-checks` / panel marker). `command: "build"` still injects nothing.

- [ ] **Step 2: Run test — expect FAIL**

- [ ] **Step 3: Implement panel + dev inject**

Prefer `injectScript("page", \`import ${JSON.stringify(new URL("./panel/panel.js", import.meta.url).href)}\`)` so the panel ESM can `import { auditPage } from "../checks/page.js"`. Keep bridge as today’s head-inline IIFE. Panel UI: compact corner trigger + drawer; show severity + message; optional resource summary counts (scripts/styles/images/HTML size) as informational text, not findings.

- [ ] **Step 4: Run `npm test --prefix packages/astro` — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add packages/astro/src/panel/panel.js packages/astro/src/index.js \
  packages/astro/test/integration.test.mjs
git commit -m "feat(orso-astro): add in-page preview checks panel"
```

---

### Task 5: Sync template + document CI behavior

**Files:**
- Modify: `scripts/sync-orso-integration.sh` (copy entire `src` JS tree including `checks/` and `panel/`, not only top-level files)
- Modify: `templates/astro-marketing/package.json` (+ lockfile) — add `linkedom`
- Run sync into `templates/astro-marketing/integrations/orso/`
- Modify: `templates/astro-marketing/AGENTS.md` — note that `npm run build` runs orso preview checks; errors fail the build
- Modify: `packages/astro/README.md` if sync instructions need the new folders

**Interfaces:**
- Consumes: Tasks 1–4 sources.
- Produces: template vendor copy that CI already invokes via `npm run build` in `deploy.yml` (no workflow file change required).

- [ ] **Step 1: Write a failing check** that sync would miss folders — e.g. extend sync script test if one exists, or a small assert in `packages/astro` README/sync by running sync in the task and asserting `integrations/orso/checks/page.js` exists after sync (scriptable in Step 3). Prefer: add `scripts/sync-orso-integration` coverage via a one-off command in Step 4.

- [ ] **Step 2: Update sync script to copy `checks/` and `panel/` (and keep existing top-level files)**

- [ ] **Step 3: Add `linkedom` to the marketing template; run `just sync-orso-integration` (or the script); commit vendored files

- [ ] **Step 4: Verify**

Run:
```bash
npm test --prefix packages/astro
npm --prefix templates/astro-marketing run build
```
Expected: orso-astro tests pass; template build runs checks (may warn on starter gaps; **must not error-fail** unless the starter currently triggers an error rule — if it does, either fix the minimal starter HTML for errors only, or temporarily note required starter fixes in the commit message and apply the smallest template fix so CI stays green). Warnings-only on the starter is the success bar.

- [ ] **Step 5: Commit**

```bash
git add scripts/sync-orso-integration.sh templates/astro-marketing \
  packages/astro/README.md
git commit -m "chore: sync preview checks into marketing template"
```

---

## Spec coverage checklist

| Spec item | Task |
| --- | --- |
| Shared rule pack in `packages/astro` | 1–2 |
| Per-page SEO + AEO + perf heuristic rules | 1 |
| Site-wide rules CI-only | 2 |
| `astro:build:done` fail on errors | 3 |
| In-page panel, dev only | 4 |
| No CMS SEO tab / daemon API | (explicit non-goals; no tasks) |
| Template sync + existing deploy workflow | 5 |
| Fix with agent | deferred |
| Lighthouse / a11y | out of scope |
