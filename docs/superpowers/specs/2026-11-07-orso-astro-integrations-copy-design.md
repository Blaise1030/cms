# orso-astro → `integrations/orso` copy — design

Date: 2026-11-07

Status: draft, awaiting review.

## Intent

Stop treating the Astro plugin as a local npm package (`file:packages/astro` / future registry
publish). Instead, ship it the shadcn way: copy runtime sources into a fixed project folder
`integrations/orso/` so scaffolds stay self-contained and maintainers never need an npm token.

Success means:

1. Scaffolded projects have no `orso-astro` dependency and no `packages/astro/` tree.
2. Plugin runtime lives at `integrations/orso/*.js` and is imported via relative paths.
3. Runtime deps (`yaml`, `js-yaml`, `jsonc-parser`) are declared on the project itself.
4. Fresh scaffold still installs, builds, and saves content through `/__orso`.
5. Canonical development and unit tests remain in-repo at `packages/astro/` (source of truth).

## What is wrong today

Scaffolds vendor a full mini-package under `packages/astro` and depend on
`"orso-astro": "file:packages/astro"`, with Rust rewriting
`file:../../packages/astro` → `file:packages/astro` in manifests and `.npmrc`
`install-links=true` so npm copies instead of symlinking. That avoids the registry but still
keeps package/lockfile/`install-links` friction, and a future public release would require npm
auth the maintainer wants to avoid.

## Decisions

| Area | Decision |
| --- | --- |
| Delivery | Copy runtime JS into `integrations/orso/` (Approach A). |
| Canonical source | Keep `packages/astro/` in this repo for authoring + `npm test`. |
| Template tree | Commit a synced copy at `templates/astro-marketing/integrations/orso/` so in-repo template `astro`/`npm` work without a `file:` dep. A small sync step (just recipe or script) copies `packages/astro/src/*.js` → that folder; a test fails if they drift. Scaffold extract still embeds from `packages/astro/src` (not from the template tree) so one authoring path feeds both. |
| Imports | Relative paths to `integrations/orso/index.js`. |
| Project deps | Hoist `yaml`, `js-yaml`, `jsonc-parser` onto the template; remove `orso-astro`. |
| Manifest rewrite | Delete `ORSO_ASTRO_*` path rewrite in `extract_template`. |
| `.npmrc` | Remove `install-links=true` if it exists only for the local package. |
| Existing projects | Out of scope: no auto-upgrade/re-copy in this change. |
| npm publish | Out of scope; not required. |

## Layout

```
integrations/orso/
  index.js      # orso(), orsoGlob() (and re-exports)
  manifest.js
  patch.js
  endpoint.js
  bridge.js
```

Flattened from `packages/astro/src/*.js`. Exclude `test/`, `node_modules/`, `package-lock.json`,
and do not ship a nested plugin `package.json`.

### Imports (template)

`astro.config.mjs`:

```js
import { orso } from "./integrations/orso/index.js";
```

`src/content.config.ts`:

```ts
import { orsoGlob } from "../integrations/orso/index.js";
```

### Dependencies (template)

Remove:

```json
"orso-astro": "file:../../packages/astro"
```

Add (versions match current `packages/astro/package.json`):

```json
"js-yaml": "^4.3.2",
"jsonc-parser": "^3.3.1",
"yaml": "^2.9.1"
```

Refresh `templates/astro-marketing/package-lock.json` after the change.

## Scaffold / embed

In `src/static.rs`:

1. Retarget `OrsoAstro` extract destination from `<dest>/packages/astro` to
   `<dest>/integrations/orso`.
2. Embed/copy only the runtime `.js` files (from `packages/astro/src/`), written flat into
   `integrations/orso/`.
3. Remove `ORSO_ASTRO_REPO_SPEC` / `ORSO_ASTRO_SCAFFOLD_SPEC` and the string replace on
   `package.json` / `package-lock.json`.
4. Keep `Template` embed as today. Because the synced `integrations/orso/` files are committed
   under the template, they would be embedded twice unless excluded: **exclude
   `integrations/orso/**` from the `Template` embed** (same idea as excluding `node_modules`),
   and only write them via the `OrsoAstro` extract. Alternatively, stop embedding plugin
   sources via a second rust-embed and let the template tree carry them once — pick **exclude
   from Template + extract from `packages/astro/src`** so scaffolds always get the canonical
   copy even if someone forgets to run sync before commit.
5. Update helpers (`orso_astro_paths`, tests asserting embed contents) for the new prefix and
   flattened filenames.
6. Add `just sync-orso-integration` (or equivalent) used by maintainers after editing
   `packages/astro/src`; add a unit/integration assertion that template files match canonical
   sources byte-for-byte (or fail CI).

Call site `Projects::scaffold` → `extract_template` stays the same entry point.

## Docs and skill

Update to describe relative imports and `integrations/orso/`:

- `packages/astro/README.md` (still documents the module API; install section becomes “copied into
  projects as `integrations/orso/`”)
- `skills/setup-orso/SKILL.md`
- `templates/astro-marketing/AGENTS.md`
- Pointers in older content-integration docs only if they would mislead operators

## Tests

Update:

| Test | Change |
| --- | --- |
| `extract_template_copies_orso_astro_and_rewrites_manifests` | Assert `integrations/orso/index.js` (and siblings); assert no `orso-astro` / `file:packages/astro`; no rewrite step. Rename if helpful. |
| `orso_astro_contains_package_sources_only` | Assert flattened runtime files under the new embed mapping. |
| Path helpers / `template_does_not_embed_orso_astro` | Still true: template embed must not include plugin sources; extract adds them. |
| `real_npm_install_and_site_builds` (`#[ignore]`) | Expect `integrations/orso/…`; project deps include hoisted YAML stack; no `file:packages/astro`. |
| `real_astro_saves_content_through_the_integration` (`#[ignore]`) | Assert plugin path under `integrations/orso/`. |

`packages/astro` unit tests stay as-is (they import the package locally).

## Non-goals

- Publishing `orso-astro` to the npm registry.
- Migrating already-scaffolded user projects.
- Letting end users treat `integrations/orso/` as a customization surface (Orso may overwrite it
  on future upgrade tooling; document as generated).
- Changing the `/__orso` protocol or manifest shape.

## Implementation outline

1. Add sync script; copy `packages/astro/src/*.js` → `templates/astro-marketing/integrations/orso/`.
2. Template: switch imports; hoist deps; drop `orso-astro`; drop unused `.npmrc` install-links;
   refresh lockfile.
3. Rust embed/extract: exclude template `integrations/orso/**` from `Template`; extract
   flattened canonical sources to `integrations/orso/`; remove rewrites; fix `src/static.rs`
   tests (including sync-drift check).
4. Docs/skill/AGENTS updates.
5. Adjust ignored e2e assertions; run package tests + relevant cargo tests.

## Open follow-ups (later)

- Optional “refresh Orso integration” action that re-copies `integrations/orso/` into an existing
  project from the current embed.
