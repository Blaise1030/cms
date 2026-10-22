# orso-astro

Astro integration for Orso-managed sites. It connects Astro content collections to the
Orso editor: it records the loader options Astro hides, publishes a manifest of the site's
collections, and serves the editor's schema validation and source-preserving patches from a
dev-only endpoint.

The package is plain ESM with no build step. `astro` is a peer dependency
(`>=7.3.0 <8`); `yaml`, `js-yaml` and `jsonc-parser` are runtime dependencies.

## Install

Scaffolded projects ship a copy of the package at `packages/astro/` and depend on it by
path:

```json
"orso-astro": "file:../../packages/astro"
```

## Usage

`astro.config.mjs`:

```js
import { defineConfig } from "astro/config";
import mdx from "@astrojs/mdx";
import { orso } from "orso-astro";

export default defineConfig({
  integrations: [mdx(), orso()],
});
```

`src/content.config.ts`:

```ts
import { defineCollection } from "astro:content";
import { orsoGlob } from "orso-astro";
import { z } from "astro/zod";

export const collections = {
  blog: defineCollection({
    loader: orsoGlob({ base: "./src/content/blog/en", pattern: "*.md" }),
    schema: z.object({ title: z.string() }),
  }),
};
```

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
endpoint. `astro build` does nothing.

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
      "fields": { "type": "object", "required": true, "properties": {} }
    }
  ]
}
```

A collection whose loader is not `orsoGlob` is listed with `loader: null` and is
read-only, because its base and pattern cannot be recovered.

## Dev-only endpoint

With `ORSO_CONTENT_TOKEN` set, the dev server mounts `POST /__orso`. The endpoint is
loopback-only, takes a JSON body of at most 2 MiB, and answers the `validate` and `patch`
operations with a constant-time token comparison. Patches are
source-preserving: the bytes the editor did not change are kept as they were.

## Tests

```sh
npm --prefix packages/astro test
```
