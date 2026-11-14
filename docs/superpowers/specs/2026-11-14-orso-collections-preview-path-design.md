# Orso-owned collections + preview path

**Date:** 2026-11-14  
**Status:** Approved for planning  
**Related:** [Astro content integration](./2026-10-21-astro-content-integration-design.md), Content editor, Preview panel

## Problem

Content editing and the live Preview are disconnected. Blog/help (and custom) URLs live in the site’s Astro routes, but Orso has no per-collection way to open the right preview path for an entry. Users must type paths like `/blog/{slug}/` into the Preview address bar by hand.

We also want a single place to define collections for Orso-aware sites: real Astro `defineCollection` TypeScript owned through the `orso()` integration, not a parallel CMS schema language and not a hand-maintained duplicate in `content.config.ts`.

## Goals

- Authors define (or override) collections via `orso({ collections })` in `astro.config`, using real Astro APIs plus an optional `previewPath` function.
- `src/content.config.ts` is a thin re-export of a Vite virtual module registered by `orso()` — Astro’s required entrypoint, not a second source of truth.
- Content UI can **Open in preview**: resolve the path with the live `previewPath` function, then navigate the Preview panel to that site-relative path.
- Marketing template migrates onto a preset shipped by `orso-astro`.
- Existing sites that do not pass `collections` keep today’s behavior (on-disk `content.config.ts`, no virtual module).

## Non-goals

- Generating or deleting `content.config.ts` from the plugin.
- CMS Settings UI to edit schemas or `previewPath`.
- Resolving reference fields to non-id properties on the target entry (pass stored ids only).
- Serializing `previewPath` into `.astro/orso-content.json` or resolving paths with the preview down.
- Forcing all existing repos onto the virtual-module setup.
- SEO-audit “Open in preview” (separate feature; can reuse `setFramePath` later).

## Authoring model

### API shape

```ts
// astro.config.mjs
import { defineConfig } from "astro/config";
import { defineCollection } from "astro:content";
import { orso, orsoGlob, marketingCollections } from "orso-astro";
// or: import { marketingCollections } from "orso-astro/marketing";

export default defineConfig({
  integrations: [
    orso({
      collections: {
        ...marketingCollections,
        // override or add:
        blog: {
          collection: defineCollection({
            loader: orsoGlob({ base: "./src/content/blog/en", pattern: "*.md" }),
            schema: { /* zod + editor meta */ },
          }),
          previewPath: ({ slug, category }) => `/blog/${category}/${slug}/`,
        },
      },
    }),
  ],
});
```

Each entry in `collections` is:

| Field | Required | Role |
|--------|----------|------|
| `collection` | yes | Result of `defineCollection({ loader, schema })` (Astro-native) |
| `previewPath` | no | `(ctx) => string` — site-relative path for Preview |

`previewPath` context:

- `slug` — entry id (filename / content id), always present.
- Other keys — top-level entry field values as stored (references = target entry ids as strings).
- No deep resolve of referenced entries in v1.

### Virtual module

When `orso({ collections })` is provided, the integration registers a Vite virtual module (e.g. `virtual:orso/collections`) that exports:

```ts
export const collections = {
  blog: <AstroCollection>, // the `.collection` values only
  // …
};
```

Site bridge:

```ts
// src/content.config.ts
export { collections } from "virtual:orso/collections";
```

If the virtual module is imported without `orso({ collections })` configured, fail at config/Vite load with a clear error.

When `collections` is omitted, do not register the virtual module; keep loading on-disk `content.config.ts` for the manifest as today.

### Defaults / marketing preset

- Ship marketing blog + helpDocs definitions (current template schemas/loaders) from `orso-astro`, including:
  - `blog` → `previewPath: ({ slug }) => `/blog/${slug}/``
  - `helpDocs` → `previewPath: ({ slug }) => `/help/${slug}/``
- Prefer **explicit** `orso({ collections: marketingCollections })` in the marketing template so non-marketing sites are not surprised by an implicit default.
- Optional later: `orso()` with no args remains “legacy content.config only.”

### Manifest

Continue writing `.astro/orso-content.json` from the Astro collections (loaders, schemas, editor meta) as today.

**Do not** put `previewPath` functions in the JSON manifest. They remain in the integration runtime for the resolve endpoint only.

## Open in preview

### Resolve endpoint

During `astro dev`, extend the existing Orso Vite endpoint stack (`packages/astro/src/endpoint.js`) — same auth/token model as today’s `POST /__orso` — with a `previewPath` (name TBD in the plan) operation that takes `collection` + `id`. The handler runs under the site’s preview base / `PUBLIC_BASE` like other Orso endpoint traffic.

Behavior:

1. Load entry fields for that collection/id (same sources the content layer already uses).
2. Build `ctx = { slug: id, ...fields }`.
3. Call `previewPath(ctx)`.
4. Return `{ path }` where `path` is a site-relative path (leading `/`, no `/preview/{projectId}` prefix), or an error if missing collection config, no `previewPath`, throw, or empty/non-string result.

### Daemon / UI

- Content panel (entry view; optionally row menu): **Open in preview** when the collection has a resolvable `previewPath` and the project preview is ready.
- How the UI knows a collection has `previewPath`: either a boolean on the content catalog/manifest (e.g. `hasPreviewPath: true` without the function), or probe/disable on resolve failure. Prefer a **boolean (or equivalent) on the manifest** so the button can hide without calling resolve.
- On click: daemon proxies to the project preview origin (same trust model as other preview traffic) → resolve → `preview.setFramePath(projectId, path)` → navigate to the Preview panel route.
- Preview not ready, proxy failure, or resolve error → disable and/or toast; never invent a path.

## Package and template migration

### `orso-astro`

- Extend `orso(options?)` with optional `collections`.
- Register virtual module; wire resolve endpoint; preserve `orsoGlob` + manifest pipeline.
- Export marketing preset module.
- Package `exports` updated for the preset entrypoint as needed.

### Marketing template

- Pass marketing preset into `orso({ collections })`.
- Replace body of `src/content.config.ts` with the virtual-module re-export.
- Move schema/loader definitions out of the template into the preset.
- Update `AGENTS.md` / README: edit collections via `orso({ collections })` / the preset, not by growing `content.config.ts`.

### Compatibility

- Repos without `collections` option: unchanged.
- Opt-in migration for custom sites.

## Failure modes

| Case | Behavior |
|------|----------|
| No `previewPath` on collection | Hide/disable Open in preview |
| Preview not ready / endpoint down | Disable or toast |
| `previewPath` throws or bad return | Toast; stay on Content |
| Virtual import without `orso({ collections })` | Clear config-time error |
| Missing fields used inside `previewPath` | Author guards in the function; Orso does not default |

## Testing

- **orso-astro:** virtual module export shape; marketing preset; resolve endpoint with slug and a reference-valued field; manifest includes `hasPreviewPath` (or equivalent) but not the function; legacy mode without `collections` still builds the manifest from disk.
- **Web:** Open in preview → resolve → `setFramePath` → Preview route; disabled states when preview down or no path.
- **E2E (marketing):** from a blog entry, Open in preview lands the iframe on `/blog/{slug}/`.

## Implementation notes (for planning)

- Keep Astro as source of collection truth; Orso integrates, does not replace the content layer.
- Reuse Preview store `setFramePath` / address-bar path semantics (`framePath` / `frameHref`).
- Align resolve URL with existing preview proxy and auth constraints (same-site session as other preview requests).
- Document the `previewPath` ctx contract next to `orsoGlob` in `packages/astro/README.md`.
