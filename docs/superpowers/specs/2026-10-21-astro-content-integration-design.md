# Astro content integration — design

Date: 2026-10-21

Status: draft, awaiting review.

## Intent

Replace the template-local content helper with one reusable Astro integration, `orso-astro`, that
reads the project's real `src/content.config.ts` and exposes its collections, schemas and save
operations to the daemon.

The package lives in this repository and is copied into every scaffolded project, so a generated
project stays self-contained. It is not published to npm in this version.

Success means:

1. No `scripts/orso-content.mjs` or `src/content-schemas.mjs` exists in the template.
2. Collection definitions, Zod schemas and editor metadata live in `src/content.config.ts`.
3. `astro.config.mjs` adds one `orso()` integration and content loaders use
   `orsoGlob({ base, pattern })`.
4. The daemon lists editable collections from the integration's manifest and validates/saves
   through the integration's dev-only HTTP endpoint.
5. A scaffolded project installs, builds and runs `npm ci` without reaching outside its own
   directory.

## What is wrong today

`templates/astro-marketing/scripts/orso-content.mjs` is a second content configuration. It imports a
hand-maintained registry from `src/content-schemas.mjs`, hard-codes `blog` and `helpDocs` as the only
editable collections, and duplicates schema descriptions and source-preserving patch logic. The
daemon spawns that script for every catalog refresh and save, so the template owns implementation
details that should belong to a reusable integration.

## Decisions

| Area | Decision |
| --- | --- |
| Packaging | A real ESM package at `packages/astro/`, named `orso-astro`, with no build step. |
| Project dependency | The template depends on `"orso-astro": "file:../../packages/astro"`; scaffolds copy the package to `packages/astro` and rewrite the path in the manifest and lockfile. |
| Install mode | `.npmrc` sets `install-links=true` so npm copies the local package instead of symlinking it; a symlink would make the package resolve `astro/loaders` from the wrong directory. |
| Collection API | `orso()` is added to `astro.config.mjs`; loaders use `orsoGlob({ base, pattern, kind?, singleton? })` because Astro's `glob()` hides its options. |
| Schemas | Schemas stay in `src/content.config.ts`. The integration reads the exported `collections` object through Astro's Vite runner. |
| Catalog | The integration writes `.astro/orso-content.json`; Rust reads that manifest and no longer spawns Node for listing. |
| Saves | The integration mounts a dev-only `POST /__orso` endpoint. Rust sends exact `validate` and `patch` operations with a per-project token. |
| Failure mode | The last manifest keeps collections readable; without a ready dev server or manifest, content is read-only. No hard-coded collection fallback remains. |
| Editable collections | The integration decides `supportedForEditing` from `orsoGlob` metadata, schema support and required unsupported nodes. Rust no longer knows `blog`/`helpDocs` by name. |

## Package

```
packages/astro/
  package.json
  README.md
  src/index.js        orso(), orsoGlob()
  src/manifest.js     config loading, describeSchema(), manifest generation
  src/patch.js        schema validation and source-preserving JSON/YAML patches
  src/endpoint.js     POST /__orso handler
  test/               Node tests and endpoint fixtures
```

`package.json` is `"type": "module"` and exposes only `"."` (mapped to `src/index.js`). It is marked
private because this version is not published. Its runtime dependencies are `yaml`, `js-yaml` and
`jsonc-parser`; `astro` is a peer dependency (`>=7.3.0 <8`). The package has its own lockfile for its
tests, which is not copied into scaffolds.

`orsoGlob` wraps Astro's `glob()` and attaches a non-enumerable
`Symbol.for("orso.glob")` value:

```js
{ base, pattern, kind, singleton }
```

It passes `generateId`, `retainBody` and `deferRender` through unchanged. `kind` is inferred from the
pattern when omitted (`*.md` → `markdown`, `*.mdx` → `mdx`, `*.json` → `json`). The wrapper throws a
clear config-time error for a missing/non-string `base` or `pattern`; a collection that uses plain
`glob()` is still listed but is read-only because the manifest cannot recover its loader options.

## Template shape

`templates/astro-marketing/src/content.config.ts` is the only content configuration:

```ts
import { defineCollection } from "astro:content";
import { orsoGlob } from "orso-astro";
import { z } from "astro/zod";

const blog = defineCollection({
  loader: orsoGlob({ base: "./src/content/blog/en", pattern: "*.md" }),
  schema: ({ image }) => z.object({
    title: z.string(),
    description: z.string(),
    pubDate: z.string().regex(...).meta({ editor: { type: "date", knownRefinement: true } }),
    coverImage: image().meta({ editor: { type: "image" } }).optional(),
    coverAlt: z.string().optional(),
    tags: z.array(z.string()).default([]),
    relatedPosts: z.array(z.string().meta({ editor: { type: "reference", collection: "blog" } })).default([]),
  }).superRefine(...).meta({ editor: { knownRefinement: true } }),
});

export const collections = { blog, helpDocs };
```

The Help docs schema is likewise inline. `templates/astro-marketing/astro.config.mjs` adds `orso()`
beside `mdx()`:

```js
import { orso } from "orso-astro";
// ...
integrations: [mdx(), orso()],
```

`templates/astro-marketing/package.json` adds the local dependency and `.npmrc` adds
`install-links=true`; `package-lock.json` is regenerated and committed.

The template keeps its existing `npm test`, which builds the pages and runs the remaining `scripts/*.test.mjs`
page tests. The helper tests are deleted with the helper.

## Manifest

The integration writes `.astro/orso-content.json` under the Astro project root. It is generated during
`astro sync` and dev-server startup, and regenerated when `src/content.config.*` changes. The write is
atomic (same-directory temporary file plus rename). The first write is awaited during dev-server
setup, so when it succeeds a ready dev server implies the manifest exists; a write failure is logged
and does not block the site.

```json
{
  "version": 1,
  "collections": [
    {
      "name": "blog",
      "loader": { "base": "src/content/blog/en", "pattern": "*.md" },
      "kind": "markdown",
      "singleton": null,
      "supportedForEditing": true,
      "reason": null,
      "fields": {
        "type": "object",
        "required": true,
        "properties": { "pubDate": { "type": "date", "required": true } }
      }
    }
  ]
}
```

- `base` is normalized relative to the site root (leading `./` removed; absolute paths and `..`
  components make the collection read-only).
- A collection whose loader is not `orsoGlob` is written with `loader: null`; it is listed but
  read-only and has no entries because its base/pattern are unknown.
- `supportedForEditing` is false when the loader is not `orsoGlob`, the pattern/kind is unsupported,
  there is no schema, or a required field is an unsupported schema node. `reason` explains why. A
  collection whose loader or schema cannot be inspected at all is still listed read-only with the
  error as its reason; one broken collection does not hide the others.
- `fields` is the `describeSchema()` projection already used by the editor, including `editor`
  metadata, defaults and requiredness. It is not a validation schema.
- The manifest contains no secrets. It is gitignored with the rest of `.astro/`.

Rust reads the manifest from `<site>/.astro/orso-content.json`, prefixes each loader base with
`settings::site_prefix`, and caches the result keyed by `(prefix, manifest mtime/size)`. It still
applies its own path-safety and supported-pattern checks before marking a collection editable.

## Endpoint

The integration mounts an exact `POST /__orso` handler only in dev (and in the temporary Vite server
used by `astro sync`), and only when `ORSO_CONTENT_TOKEN` is set. `orso()` reads that token from the
environment; the daemon generates a fresh random token for each dev-server start and passes it to the
child process.

Request headers:

- `content-type: application/json`
- `x-orso-content-token: <token>`

Requests:

```json
{ "op": "validate", "collection": "blog", "fields": { "title": "Hello" } }
{ "op": "patch", "collection": "blog", "source": "---\n...", "fields": { "title": "Hello" }, "unset": [], "body": "..." }
```

`fields` is a complete candidate for `validate`, and a partial top-level update for `patch`. `body`
is optional on `patch`; omitting it preserves the current body. `unset` names top-level keys to
remove and may not overlap `fields`.

Responses:

- `200 {"valid":true,"fields":{...},"errors":[]}` for a valid `validate`.
- `200 {"valid":false,"fields":{...},"errors":[{"path":["pubDate"],"message":"..."}]}` for an invalid
  `validate`; validation itself succeeded.
- `200 {"source":"---\n..."}` for a successful `patch`.
- `422 {"error":"invalid_schema","issues":[{"path":["pubDate"],"message":"..."}]}` when the complete
  candidate fails the live schema.
- `400 {"error":"bad_request","message":"..."}` for malformed input or a patch that cannot safely
  preserve the source.
- `401 {"error":"unauthorized"}` for a missing or wrong token.
- `404 {"error":"unsupported_collection","message":"..."}` for a collection absent from the current
  manifest, and `400` with the same error for a collection that is listed but not
  `supportedForEditing`.
- `413` for bodies over 2 MiB (entries themselves are capped at 1 MiB).
- `405` for any method other than POST.
- `500 {"error":"internal","message":"..."}` for an unexpected failure. The integration logs the
  server-side detail.

The handler rejects non-loopback socket addresses when one is available, requires a JSON content
type, and compares the token in constant time. It returns no CORS headers and never puts the token in
a URL or log line.

The endpoint uses the schemas loaded from the current content config. On a config change the
integration re-imports the config through the Vite runner, rebuilds the manifest, and uses the new
schemas for subsequent requests.

## Rust daemon

### Catalog

`src/content/catalog.rs` no longer runs a helper process.

- `list_collections` reads `<site>/.astro/orso-content.json`; a missing or malformed manifest returns
  an empty collection list with no hard-coded fallback.
- `ContentCollection` still contains `editable`; it is computed as
  `supportedForEditing && pathIsSafe && patternSupported && fieldsPresent && !hasRequiredUnsupported`.
  The manifest's `reason` is preserved when a collection is read-only.
- `entries` and `read_entry` are unchanged: they walk the declared base, read files within the project,
  and keep the existing symlink/path protections.
- If the dev server is unavailable, the last manifest remains usable for reading. The HTTP collection
  endpoint overlays runtime availability: it returns `editable: false` with a "start the preview"
  reason when the project is not `ready` or has no content endpoint, so the editor stays read-only
  instead of offering a form that cannot save. Saves also fail with a clear "preview is not running"
  error and never write.

### Integration client

`src/content/helper.rs` becomes `src/content/integration.rs` (or an equivalently named client). It
does not execute project files. `patch`:

1. Resolves the project's content endpoint from `AppState`.
2. POSTs `{op:"patch", collection, source, fields, unset, body?}` with the token.
3. Returns the response `source`.
4. Maps `422 issues` to the existing `invalid_schema` API error and detail shape.
5. Maps `404`, `400`, auth and transport failures to clear `ApiError`s.

The client has a 10-second timeout and a 2 MiB response limit. No write occurs unless a 200 response
contains a string source.

`AppState::content_endpoint(project)` requires the project to exist, its dev server to be `ready`, and
a token to be present. The save and lifecycle code resolve it before calling the endpoint; if it is
unavailable, the draft stays in the browser and the daemon reports the failure.

### Dev server token

`DevServerManager` generates a 32-byte random token on each `start`, stores it for the project, and
passes it as `ORSO_CONTENT_TOKEN` to the child. The token is removed on `stop` and shutdown; automatic
crash restarts inside one supervision keep the same token. Tests use the existing custom-launcher hook
with a local fake server that reads the injected token and prints a ready line.

## Scaffold

`src/static.rs` embeds the package separately:

- `Template` remains `templates/astro-marketing/`.
- `OrsoAstro` embeds `packages/astro/`, excluding `node_modules/**`, `test/**`, `package-lock.json`
  and `.DS_Store`.

`extract_template` copies the template, copies the package to `<project>/packages/astro`, then rewrites:

- `package.json`: `"file:../../packages/astro"` → `"file:packages/astro"`.
- `package-lock.json`: every `"file:../../packages/astro"` → `"file:packages/astro"`.

The package source is committed in the scaffold. Scaffold's normal install then validates the
rewritten lockfile and installs the package with `install-links=true`; generated projects can run
`npm ci` in GitHub Actions.

`template_paths()` continues to describe the template only; a new list (for example
`orso_astro_paths()`) describes the package. Static tests assert both, and assert the deleted helper
files are absent.

## Frontend

- `web/src/modules/content/store.ts` drops the deleted helper/schema paths from `CONTENT_FILES` and
  watches `astro.config.mjs`, `src/content.config.ts` and `packages/astro` instead.
- The same store refreshes collections when a project's `devserver.status` becomes `ready`, so a
  panel opened before the first manifest appears fills in without a manual reload.
- No editor field format changes: the manifest uses the existing `FieldDescriptor` shape and the
  existing collection API payload.

## Failure handling

| Failure | Behaviour |
| --- | --- |
| Manifest missing or invalid | Content lists no collections; Code, Preview and the site are unaffected. |
| Content config fails to load | Integration logs the error, leaves the last manifest in place; the endpoint keeps using the last successfully loaded schemas. |
| Dev server not ready / no token | Read-only last manifest; saves return `DEVSERVER_ERR` with a "start the preview" message. |
| Token rejected | Same as unavailable; do not retry with another credential. |
| Endpoint schema failure | `422` issues become field errors; no file is written. |
| Endpoint source-preservation refusal | `400` message; no file is written. |
| Endpoint dies mid-save | Save fails; the daemon's journal and re-check keep the entry unchanged. |
| Config change race | The integration rebuilds the manifest on the watcher event; the Content store refreshes on config changes and on dev-server readiness. |

## Testing

CI installs `packages/astro`'s locked dependencies before the package and Rust tests; the existing
template dependency install remains required for the template tests and scaffold tests.

### Package (`npm test` in `packages/astro/`)

- `orsoGlob` attaches metadata, preserves pass-through options, infers kinds and rejects invalid
  options.
- `describeSchema` covers strings, dates, optional/default, images, references, arrays, objects,
  discriminated unions and unsupported transforms/refinements; required unsupported nodes disable
  editing.
- `patch` covers JSON and YAML source preservation (comments, quoting, key order, line endings,
  array splices), schema validation, defaults and refusal to edit unsafe shapes.
- The endpoint handler covers auth, method/content-type checks, body limit, `validate`, `patch`, 422
  issues and unsupported collections.
- A fixture schema mirrors the Blog/Help Docs rules used by Rust's endpoint fixture.

### Template (`npm test`)

- `astro sync` writes `.astro/orso-content.json` with `blog` and `helpDocs`, their loader bases and
  patterns, `supportedForEditing: true`, and the `pubDate`/`coverImage` editor descriptors.
- Existing page builds still render Blog, Help, navigation and markdown.
- No test imports the deleted helper or schema registry.

### Rust (`cargo test`)

- Catalog tests write manifest fixtures and assert collection listing, field descriptors, subfolder
  prefixing, read-only unsupported loaders, required unsupported schemas, missing/invalid manifests,
  cache invalidation on manifest change and raw entry reads.
- Integration-client tests use a local fake endpoint to assert request shape, token use, 200/422/400
  mapping, timeouts and unavailable dev servers.
- Content mutation, lifecycle, history and image tests use a fake dev-server process that prints a
  ready line and serves `/__orso` with the package's patch function and fixture schemas. This
  exercises the manager's token injection and the daemon's HTTP client without starting Astro for
  every test.
- One end-to-end test starts the real Astro dev server for a scaffolded project, waits for `ready`,
  reads the generated manifest, creates a change, saves an entry through `/__orso`, and asserts the
  file and checkpoint. The scaffold install/build test continues to verify package copying, path
  rewriting and `npm ci` compatibility.
- Static tests assert the package is embedded and the old helper files are gone.

## Migration

Delete:

- `templates/astro-marketing/scripts/orso-content.mjs`
- `templates/astro-marketing/scripts/orso-content.test.mjs`
- `templates/astro-marketing/src/content-schemas.mjs`

Update:

- `templates/astro-marketing/src/content.config.ts`, `astro.config.mjs`, `package.json`,
  `package-lock.json`, `.npmrc`, `AGENTS.md`
- `src/content/{mod,catalog,mutation,lifecycle}.rs`, `src/state.rs`, `src/devserver.rs`,
  `src/static.rs`
- `web/src/modules/content/store.ts`, its refresh test and any fixtures
- Rust content tests, template tests and static tests

`rg` after the change must find no `orso-content.mjs`, `content-schemas.mjs` or
`supportedForEditing` allowlist by collection name.

## Out of scope

- npm publishing or a release pipeline for `orso-astro`.
- Automatically adding the integration to cloned or imported projects that were not scaffolded from
  the template; those projects stay read-only until they add the package and `orso()` themselves.
- Supporting arbitrary loader shapes that cannot expose `base` and `pattern`.
- Rich Markdown editing for collections other than the template's existing prose collections.
- Generating the manifest during `astro build`; `astro sync` and dev are the supported generators.
- Server-side rendering or a change to the static Cloudflare deployment.

## Delivery slices

1. **Package and template:** add `packages/astro`, move schemas/loaders into `content.config.ts`, add
   `orso()`, copy the package in scaffolds, delete the old helper and update template tests.
2. **Manifest catalog:** switch Rust collection listing to `.astro/orso-content.json`, remove the
   hard-coded fallback and helper spawn, and update catalog tests.
3. **Save endpoint:** add the token/dev-server plumbing and Rust HTTP client, switch mutations and
   lifecycle to it, add fake-endpoint tests and the single real-dev-server end-to-end test.

Each slice should leave the app buildable; slice 2 intentionally makes content read-only until the
dev server has written a manifest.
