---
title: Set up your site by hand
description: The manual steps to make an existing Astro site work with orso.
category: existing-site
order: 3
---

If you prefer not to use [the agent skill](/help/set-up-with-the-agent-skill/), these are the steps it would take. Do them on a new branch.

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

Add `orso()` as the last integration. Keep your existing settings.

```js
import { orso } from "./integrations/orso/index.js";

export default defineConfig({
  integrations: [/* your integrations */, orso()],
});
```

orso previews your site at the root of its own address, so links that start with `/` work as written. `orso()` connects live updates itself when orso runs the dev server.

If your config still has settings for an older orso, remove them: a `base` read from `PUBLIC_BASE`, a `vite.server.hmr` block read from `ORSO_DEV_PORT` or `ORSO_PROXY_PORT`, and any link helper that adds `import.meta.env.BASE_URL`.

## 3. Check your links

There's nothing to change: write links the plain way (`/about/`, `/help/install-orso/`) in components, Markdown, scripts and CSS. If your live site itself needs a `base` (for example a project page on GitHub Pages), keep it. The preview still opens at `/`.

## 4. Make collections editable

See [Make your content collections editable](/help/make-collections-editable/).

## 5. Add `DESIGN.md` and, if needed, `orso.json`

- Put a `DESIGN.md` at the repo root. It holds your colours, fonts and spacing in YAML front matter, followed by a short description of the look.
- Add `orso.json` only when the defaults don't fit. See [Configure orso.json](/help/configure-orso-json/).

## 6. Check it

Run the dev server, then open the repo in orso. The preview should load, and the Content tab should list your collections. If it doesn't, see [The preview is blank or broken](/help/preview-not-loading/).
