---
title: Set up your site by hand
description: The manual steps to make an existing Astro site work with orso.
category: existing-site
order: 3
---

If you prefer not to use the agent skill, these are the steps it would take. Do them on a new branch.

## 1. Copy the integration

The integration is plain JavaScript copied into your site. There is no npm package.

```sh
tmp=$(mktemp -d)
git clone --depth 1 --filter=blob:none --sparse https://github.com/Blaise1030/cms "$tmp"
git -C "$tmp" sparse-checkout set packages/astro/src
mkdir -p integrations/orso
rsync -a --delete --exclude 'marketing*.js' "$tmp/packages/astro/src/" integrations/orso/
rm -rf "$tmp"
```

Then add its dependencies: `yaml`, `js-yaml`, `jsonc-parser` and `linkedom`. The config fails to load without `linkedom`.

Don't edit files in `integrations/orso/`. They are replaced when you update.

## 2. Update `astro.config.mjs`

Add `orso()` as the last integration, and route `base` and the HMR socket through orso's settings. Keep your existing settings.

```js
import { orso } from "./integrations/orso/index.js";

const base = process.env.PUBLIC_BASE ?? "/";
const hmrPort = Number(process.env.ORSO_DEV_PORT || process.env.ORSO_PROXY_PORT || 0);

export default defineConfig({
  base,
  integrations: [/* your integrations */, orso()],
  vite: {
    server: {
      hmr: {
        path: `${base}vite-hmr`.replace(/\/{2,}/g, "/"),
        ...(hmrPort > 0 ? { clientPort: hmrPort } : {}),
      },
    },
  },
});
```

## 3. Make links base-aware

Links that start with `/` skip the preview's base path and break in orso. Route them through a small helper that adds `import.meta.env.BASE_URL`. Start with the header, footer and navigation.

## 4. Make collections editable

See [Make your content collections editable](../make-collections-editable/).

## 5. Add `DESIGN.md` and, if needed, `orso.json`

- Put a `DESIGN.md` at the repo root. It holds your colours, fonts and spacing in YAML front matter, followed by a short description of the look.
- Add `orso.json` only when the defaults don't fit. See **Configure orso.json**.

## 6. Check it

Run the dev server, then open the repo in orso. The preview should load, and the Content tab should list your collections.
