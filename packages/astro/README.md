# orso-astro

Astro integration for Orso-managed sites. It connects Astro content collections to the
Orso editor: it records the loader options Astro hides, publishes a manifest of the site's
collections, and serves the editor's schema validation and source-preserving patches from a
dev-only endpoint.

The package is plain ESM with no build step. `astro` is a peer dependency
(`>=7.3.0 <8`); `yaml`, `js-yaml` and `jsonc-parser` are runtime dependencies.

## Install

Scaffolded projects receive a flattened copy of this package's `src/*.js` at
`integrations/orso/`. That folder is generated: scaffolding overwrites it, and it is not
an npm package. Import it with relative paths (from the project root, and from
`src/content.config.ts`):

```js
import { orso } from "./integrations/orso/index.js";
```

```ts
import { orsoGlob } from "../integrations/orso/index.js";
```

`yaml`, `js-yaml`, and `jsonc-parser` are dependencies of the site. There is no
`orso-astro` dependency and no `file:packages/astro` path.

Canonical sources stay in `packages/astro/src/` (top-level modules plus `checks/` and
`panel/`). `just sync-orso-integration` replaces `integrations/orso/` in
`templates/astro-marketing/` and `site/` with the full `.js` tree, so files removed here do not
linger there. To vendor the same
files into an existing site, sparse-checkout `packages/astro/src` and copy those `.js`
files (including subfolders) into `integrations/orso/`. The marketing template needs
`linkedom` as a dependency for build-time preview checks.

## Preview bridge

During `astro dev`, `orso()` injects a small inline script into every page. The Orso dev UI
frames the project's dev server directly (so Astro islands and other client modules hydrate);
the script lets that cross-origin frame pick elements and apply the app's appearance
overrides over `postMessage`. It does nothing unless the app posts an `orso:preview` message,
and it is not part of a build.

## Usage

`astro.config.mjs`:

```js
import { defineConfig } from "astro/config";
import mdx from "@astrojs/mdx";
import { orso } from "./integrations/orso/index.js";

export default defineConfig({
  integrations: [mdx(), orso()],
});
```

### `orso({ collections })` and `virtual:orso/collections`

The package ships no content models: each site defines its own collections, loads them with
`orsoGlob`, and passes them to `orso()`. Keep them in a file the site owns (the marketing
template and orso's own site use `src/content/collections.js`), not in `integrations/orso/`,
which scaffolding overwrites.

`src/content/collections.js`:

```js
import { z } from "astro/zod";
import { orsoGlob } from "../../integrations/orso/index.js";

const blog = {
  type: "content_layer",
  loader: orsoGlob({ base: "./src/content/blog/en", pattern: "*.md" }),
  schema: ({ image }) =>
    z.object({
      title: z.string(),
      description: z.string(),
      coverImage: image().meta({ editor: { type: "image" } }).optional(),
    }),
};

const authors = {
  type: "content_layer",
  loader: orsoGlob({ base: "./src/content/authors", pattern: "*.json" }),
  schema: z.object({ name: z.string() }),
};

/** `orso({ collections })` entries, in sidebar order. */
export const collections = {
  blog: { collection: blog, previewPath: ({ slug }) => `/blog/${slug}/` },
  authors: { collection: authors },
};
```

`astro.config.mjs`:

```js
import { orso } from "./integrations/orso/index.js";
import { collections } from "./src/content/collections.js";

export default defineConfig({
  integrations: [orso({ collections })],
});
```

`src/content.config.ts` only re-exports the virtual module:

```ts
export { collections } from "virtual:orso/collections";
```

Each value is `{ collection, previewPath? }`: `collection` is an Astro collection definition
(`astro:content` cannot be imported from `astro.config.mjs`, so write the object
`defineCollection` would return, with `type: "content_layer"`), and the key is the collection
name. The editor lists collections in this order. Collections whose loader is not `orsoGlob`
are allowed but read-only in the editor.

`previewPath(ctx)` receives one object and must return a site-relative path with a leading `/`
(for example `/blog/welcome/`):

- `ctx.slug` is always the entry id. It replaces a stored field named `slug`.
- Every other key is a top-level stored field.
- Reference fields are the stored entry ids, not resolved entries.

Collections that define `previewPath` are flagged `hasPreviewPath: true` on the manifest.
The editor can open those entries in preview even when the collection is read-only.

### `orso({ hide })`

Some collections are not meant to be edited in orso, such as translated copies another tool
(Crowdin, for example) writes. List them in `hide` to keep them out of the editor on purpose:

```js
export default defineConfig({
  integrations: [orso({ hide: ["blogI18n"] })],
});
```

A hidden collection is still in the manifest, flagged `hidden: true` and read-only. The app
leaves it out of the content list without the "collection is hidden" warning it shows for
collections it cannot edit (such as ones that do not use `orsoGlob`). It can name any
collection, `orsoGlob` or not. A name that is not a collection is ignored with a warning when
the manifest is written; `hide` that is not an array of non-empty strings throws.

### `orsoGlob({ base, pattern, kind?, singleton?, ...options })`

Wraps Astro's `glob()` loader and records what Astro hides, under the non-enumerable,
frozen symbol `Symbol.for("orso.glob")`:

```js
{ base, pattern, kind, singleton }
```

- `base` is normalized to a site-root-relative, forward-slash path (`./src/content` →
  `src/content`). It is required.
- `pattern` is required and is passed to Astro unchanged.
- `kind` is inferred from the pattern when omitted (`*.md` → `markdown`, `*.mdx` → `mdx`,
  `*.json` → `json`, otherwise `unknown`) and kept as given when supplied.
- `singleton` is the entry id for a single-entry collection, or `null`.
- `generateId`, `retainBody` and `deferRender` (and every other `glob()` option) are passed
  through unchanged.

Invalid configuration throws at config time: `base` and `pattern` must be non-empty
strings, and `singleton` must be a string or `null`.

### `orso()`

The Astro integration. `astro dev` and `astro sync` write the content manifest; the dev
server also reloads it when the content config changes and mounts the dev-only content
endpoint. `astro sync` skips `astro:server:setup`, so for sync the integration adds a Vite
plugin whose `configureServer` writes the manifest once. On `astro build`, the
integration runs preview checks over the output directory (errors fail the build;
warnings do not) and does not inject the dev preview bridge.

## Manifest

The integration writes `.astro/orso-content.json` under the Astro project root (the file
is gitignored with the rest of `.astro/`):

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
      "hidden": false,
      "hasPreviewPath": true,
      "fields": { "type": "object", "required": true, "properties": {} }
    }
  ]
}
```

A collection whose loader is not `orsoGlob` is listed with `loader: null` and is
read-only, because its base and pattern cannot be recovered. A collection named in
`orso({ hide })` is listed with `hidden: true`, `supportedForEditing: false` and the reason
`Collection is hidden from orso`.

## Dev-only endpoint

With `ORSO_CONTENT_TOKEN` set, the dev server mounts `POST /__orso`. The endpoint is
loopback-only, takes a JSON body of at most 2 MiB, and answers the `validate`, `patch`,
and `previewPath` operations with a constant-time token comparison. Patches are
source-preserving: the bytes the editor did not change are kept as they were.
`previewPath` calls the collection's handler with the ctx above and returns
`{ "path": "/blog/welcome/" }`. It does not require the collection to be editable.

## Tests

```sh
npm --prefix packages/astro test
```
