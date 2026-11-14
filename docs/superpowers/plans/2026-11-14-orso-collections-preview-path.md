# Orso Collections + Preview Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let sites define Astro collections through `orso({ collections })` (virtual module + marketing preset) and let Content open the matching live Preview path via a `previewPath` function resolved on the running preview.

**Architecture:** `orso-astro` accepts per-collection `{ collection, previewPath? }`, exposes Astro-facing collections on `virtual:orso/collections`, writes `hasPreviewPath` into the manifest (not the function), and adds a `POST /__orso` op `previewPath`. The daemon proxies that op after loading entry fields; the Content UI calls a new API and `setFramePath` + navigates to Preview.

**Tech Stack:** `orso-astro` (Vite virtual module, existing `__orso` endpoint), Rust content/integration HTTP, Vue Content + Preview store, Vitest / Node test / Playwright.

**Spec:** `docs/superpowers/specs/2026-11-14-orso-collections-preview-path-design.md`

## Global Constraints

- Real Astro `defineCollection` / Zod only — no parallel schema DSL and no codegen of `content.config.ts`.
- `previewPath` functions stay in the integration runtime; never serialize them into `.astro/orso-content.json`.
- Manifest may expose `hasPreviewPath: boolean` only.
- Resolve only while the project preview is ready, over the existing loopback `POST /__orso` + `x-orso-content-token` path.
- Site-relative paths only (e.g. `/blog/welcome/`); no `/preview/{projectId}` prefix in `previewPath` results.
- `previewPath` context: `{ slug: entryId, ...topLevelFields }` with reference fields as stored target ids.
- Omitting `orso({ collections })` preserves today’s on-disk `content.config.ts` manifest behavior.
- Marketing template uses an **explicit** preset import, not an implicit default inside bare `orso()`.

## Review Focus

- Importing `virtual:orso/collections` without `orso({ collections })` must fail at config/Vite load with a clear error (not a silent empty export).
- A `previewPath` that throws or returns a non-string/empty value must surface as a user-visible failure, never a guessed URL.
- Legacy sites without `collections` must still list collections and must not register the virtual module.
- `hasPreviewPath: false` / absent must hide Open in preview even if someone calls the API.
- Open in preview must no-op navigate only after a successful resolve; a downed preview must not leave a stale path from a prior visit.

## File structure

| File | Responsibility |
|------|----------------|
| `packages/astro/src/collections-option.js` | Normalize `orso({ collections })` map; split Astro collections vs `previewPath` fns; `hasPreviewPath` flags |
| `packages/astro/src/virtual-collections.js` | Vite plugin for `virtual:orso/collections` |
| `packages/astro/src/marketing.js` | Marketing preset (`blog`, `helpDocs` + previewPath fns) |
| `packages/astro/src/index.js` | `orso(options?)` wires virtual module + passes previewPath map into endpoint/manifest setup |
| `packages/astro/src/manifest.js` | Emit `hasPreviewPath` on each collection when known |
| `packages/astro/src/endpoint.js` | `op: "previewPath"` |
| `packages/astro/package.json` | Export `./marketing` |
| `templates/astro-marketing/astro.config.mjs` | `orso({ collections: marketingCollections })` |
| `templates/astro-marketing/src/content.config.ts` | Re-export virtual module only |
| `src/content/catalog.rs` + `web/src/core/types.ts` | `hasPreviewPath` on `ContentCollection` |
| `src/content/integration.rs` | Client helper for `previewPath` op |
| `src/http/content.rs` | `POST /api/content/preview-path` |
| `web/src/core/api.ts` | API client method |
| `web/src/modules/content/ContentPanel.vue` (+ tests) | Open in preview action |
| `e2e/specs/…` | Marketing open-in-preview path |

---

### Task 1: Normalize `collections` option + `hasPreviewPath` in the manifest

**Files:**
- Create: `packages/astro/src/collections-option.js`
- Modify: `packages/astro/src/manifest.js`
- Test: `packages/astro/test/collections-option.test.mjs` (new), extend `packages/astro/test/manifest.test.mjs`

**Interfaces:**
- Consumes: existing manifest collection shape from `manifest.js`.
- Produces:
  - `normalizeCollectionsOption(input) -> { astroCollections, previewPaths: Map<string, Function>, hasPreviewPath: Record<string, boolean> }`
  - Each input value must be `{ collection, previewPath? }`; `previewPath` if present must be a function.
  - Manifest collection objects may include `hasPreviewPath: boolean` (default `false` when unknown/legacy).

- [ ] **Step 1: Write failing tests** for normalize + manifest flag

Assert: valid map splits `collection` vs `previewPath`; missing `previewPath` → `hasPreviewPath[name] === false`; non-function `previewPath` throws; manifest serialization includes `hasPreviewPath: true` when provided to the writer helper you add/extend.

- [ ] **Step 2: Run tests — expect FAIL**

Run: `npm test --prefix packages/astro` (or the repo’s usual `orso-astro` test command)

- [ ] **Step 3: Implement `normalizeCollectionsOption` and manifest `hasPreviewPath`**

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add packages/astro/src/collections-option.js packages/astro/src/manifest.js packages/astro/test/collections-option.test.mjs packages/astro/test/manifest.test.mjs
git commit -m "feat(orso-astro): normalize collections option and hasPreviewPath"
```

---

### Task 2: `orso({ collections })` + `virtual:orso/collections`

**Files:**
- Create: `packages/astro/src/virtual-collections.js`
- Modify: `packages/astro/src/index.js` (`orso()` currently takes no args)
- Test: `packages/astro/test/virtual-collections.test.mjs` (new), extend `packages/astro/test/integration.test.mjs` if present

**Interfaces:**
- Consumes: `normalizeCollectionsOption` from Task 1.
- Produces:
  - `orso(options?: { collections?: Record<string, { collection: unknown, previewPath?: Function }> })`
  - Vite virtual id `virtual:orso/collections` resolving to `export const collections = { ...astroCollections }`
  - Clear error if the virtual module is resolved when no `collections` option was passed
  - When `collections` omitted: no virtual plugin; existing content.config → manifest path unchanged
  - When `collections` provided: register virtual plugin; keep previewPath map in integration runtime for Task 3; set `hasPreviewPath` when writing the manifest (from the normalized flags)

- [ ] **Step 1: Write failing tests** for virtual module export and misconfig error

- [ ] **Step 2: Run tests — expect FAIL**

- [ ] **Step 3: Implement Vite plugin + `orso(options)` wiring**

Register the plugin in `astro:config:setup` via `updateConfig({ vite: { plugins: [...] } })` alongside existing sync/dev setup. Hold `previewPaths` on closure state shared with server setup (Task 3 will read it).

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add packages/astro/src/virtual-collections.js packages/astro/src/index.js packages/astro/test/virtual-collections.test.mjs packages/astro/test/integration.test.mjs
git commit -m "feat(orso-astro): virtual:orso/collections from orso({ collections })"
```

---

### Task 3: `POST /__orso` op `previewPath`

**Files:**
- Modify: `packages/astro/src/endpoint.js`
- Modify: `packages/astro/src/index.js` (pass `previewPaths` into endpoint lookup/context)
- Test: extend `packages/astro/test/endpoint.test.mjs` (or equivalent)

**Interfaces:**
- Consumes: runtime `previewPaths` map from Task 2; existing endpoint auth.
- Produces:
  - Request: `{ op: "previewPath", collection: string, id: string, fields: object }`
  - Success `200`: `{ path: string }` where `path` is non-empty, starts with `/`, and is whatever the function returned after trim — reject if not a string or empty
  - `ctx` passed to the function: `{ slug: id, ...fields }` (fields must be a plain object)
  - Errors: missing collection / no function → `400`/`404` with stable `error` codes; function throw → `400` `{ error: "preview_path_failed", message }` (message from `Error.message` only)
  - Collection need not be `supportedForEditing` to resolve a path (preview can exist for read-only collections); still require the collection to exist in the previewPaths map or manifest lookup you choose — **prefer previewPaths map** as source of truth for the function

- [ ] **Step 1: Write failing endpoint tests** (happy path with reference field id in ctx; throw; empty return; missing previewPath)

- [ ] **Step 2: Run tests — expect FAIL**

- [ ] **Step 3: Implement `previewPath` in `operation()` and wire `previewPaths` from `orso()` setup**

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add packages/astro/src/endpoint.js packages/astro/src/index.js packages/astro/test/endpoint.test.mjs
git commit -m "feat(orso-astro): previewPath op on POST /__orso"
```

---

### Task 4: Marketing preset + template migration

**Files:**
- Create: `packages/astro/src/marketing.js`
- Modify: `packages/astro/package.json` (`exports["./marketing"]`)
- Modify: `templates/astro-marketing/astro.config.mjs`
- Modify: `templates/astro-marketing/src/content.config.ts`
- Modify: `templates/astro-marketing/AGENTS.md`, `packages/astro/README.md`
- Test: `packages/astro/test/marketing.test.mjs` (new); template still typechecks/builds via existing CI/scripts

**Interfaces:**
- Consumes: Tasks 1–2 API.
- Produces:
  - `marketingCollections` with `blog` and `helpDocs` matching today’s template schemas/loaders
  - `blog.previewPath: ({ slug }) => `/blog/${slug}/``
  - `helpDocs.previewPath: ({ slug }) => `/help/${slug}/``
  - Template: `orso({ collections: marketingCollections })` and `export { collections } from "virtual:orso/collections"`

- [ ] **Step 1: Write failing preset test** (keys, hasPreviewPath flags via normalize, blog path for `{ slug: "welcome" }`)

- [ ] **Step 2: Run test — expect FAIL**

- [ ] **Step 3: Move schemas into `marketing.js`, wire template + docs**

Keep `file:../../packages/astro` (or scaffold copy) resolution working; no new publish step.

- [ ] **Step 4: Run orso-astro tests + template `astro sync` / content config load — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add packages/astro templates/astro-marketing
git commit -m "feat(astro-marketing): collections via orso marketing preset"
```

---

### Task 5: Daemon `POST /api/content/preview-path`

**Files:**
- Modify: `src/content/catalog.rs` (`has_preview_path` / `hasPreviewPath`)
- Modify: `src/content/integration.rs` (new `preview_path` client fn)
- Modify: `src/http/content.rs` + `src/http/mod.rs`
- Modify: `web/src/core/types.ts` (`hasPreviewPath?: boolean`)
- Test: Rust tests next to integration/http content (follow existing patterns in `tests/` or `src/content/`)

**Interfaces:**
- Consumes: Task 3 `__orso` op; existing `ContentEndpoint` + entry read.
- Produces:
  - `integration::preview_path(endpoint, collection, id, fields) -> Result<String, ApiError>`
  - Body to `__orso`: `{ "op":"previewPath", "collection", "id", "fields" }`
  - HTTP: `POST /api/content/preview-path` with `{ project, collection, id }` → load entry on the active/readable branch as other content reads do → call integration → `{ path }`
  - If preview/endpoint unavailable: same class of error as content save (`PREVIEW_REASON` / devserver errors)
  - Catalog passthrough of `hasPreviewPath` from manifest (default `false`)

- [ ] **Step 1: Write failing Rust tests** for client JSON shape + catalog default/`true` passthrough

- [ ] **Step 2: Run tests — expect FAIL**

- [ ] **Step 3: Implement client, route, and type fields**

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add src/content src/http web/src/core/types.ts
git commit -m "feat(content): preview-path API via __orso"
```

---

### Task 6: Content UI “Open in preview”

**Files:**
- Modify: `web/src/core/api.ts`
- Modify: `web/src/modules/content/ContentPanel.vue` (entry toolbar and/or row menu)
- Modify: `web/src/modules/content/ContentPanel.test.ts`
- Optional small helper: `web/src/modules/content/openInPreview.ts` if it keeps the panel thinner

**Interfaces:**
- Consumes: `POST /api/content/preview-path`; `usePreviewStore().setFramePath`; `projectPath(id, "preview")` from router.
- Produces:
  - `api.resolveContentPreviewPath(project, collection, id) -> Promise<{ path: string }>`
  - Button/menu item **Open in preview** visible when `collection.hasPreviewPath === true` and preview state is ready (reuse existing editable/preview-ready signals where possible)
  - On success: `setFramePath(projectId, path)` then `router.push(projectPath(projectId, "preview"))`
  - On failure: toast with the API error message; do not change frame path

- [ ] **Step 1: Write failing ContentPanel tests** (hidden without flag; ready+flag calls API and navigates; failure toasts and skips setFramePath)

- [ ] **Step 2: Run tests — expect FAIL**

Run: the web package’s vitest filter for `ContentPanel`

- [ ] **Step 3: Implement API method + UI action**

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add web/src/core/api.ts web/src/modules/content
git commit -m "feat(content): Open in preview from collection previewPath"
```

---

### Task 7: E2E marketing path

**Files:**
- Modify or create: `e2e/specs/content-editor.spec.ts` (existing `previewAt` helper) or a focused sibling spec

**Interfaces:**
- Consumes: Tasks 4–6 end state on marketing template.

- [ ] **Step 1: Add failing e2e** — open blog entry `welcome` (or create slug), click Open in preview, assert preview address/iframe path is `/blog/{slug}/`

- [ ] **Step 2: Run e2e — expect FAIL** until UI wired (if Task 6 done, expect PASS)

- [ ] **Step 3: Fix only regressions if needed; do not expand scope**

- [ ] **Step 4: Commit**

```bash
git add e2e/specs
git commit -m "test(e2e): open blog entry in preview via previewPath"
```

---

## Spec coverage checklist

| Spec item | Task |
|-----------|------|
| `orso({ collections })` + `{ collection, previewPath }` | 1–2 |
| `virtual:orso/collections` + thin `content.config.ts` | 2, 4 |
| Manifest without function; `hasPreviewPath` | 1–2, 5 |
| Resolve via live preview / `__orso` | 3, 5 |
| Marketing preset explicit in template | 4 |
| Open in preview UI | 6–7 |
| Legacy omit `collections` | 2 |
| Non-goals (CMS schema UI, codegen, offline resolve) | intentionally unscheduled |
