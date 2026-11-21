---
title: The preview is blank or broken
description: Fix a preview that won't load, or loads without styles.
category: troubleshooting
order: 4
---

## The preview never loads

- **Install failed.** When you open a project, orso installs the site's dependencies first. If the install fails, the preview can't start. Check the message orso shows, and make sure `orso.json` ([Configure orso.json](/help/configure-orso-json/)) uses the right install command if you use pnpm or yarn.
- **No `dev` script.** The site's `package.json` needs a `dev` script that runs `astro dev`.
- **The site isn't at the repo root.** Set `root` in `orso.json` to the folder that holds `astro.config.*`.

## The page loads without styles, or links go to the wrong place

orso serves your site's preview at the root of its own address, so plain links like `/about/` work. If styles or links break only in orso, look for settings left over from an older orso:

- a `base` in `astro.config.mjs` read from `PUBLIC_BASE`;
- a link helper that adds `BASE_URL` or strips a `/preview/{id}` prefix.

Remove them. See [Set up your site by hand](/help/set-up-by-hand/) for the current setup.

## The preview doesn't update when I edit

`orso()` connects live updates when orso runs the dev server. Check that `orso()` is in your config's integrations, and remove any `vite.server.hmr` block left over from an older orso. Reloading the preview shows your latest saved edits in the meantime.

## The page shows an error

orso shows build and page errors over the preview. Read the message: it usually names the file and line. You can ask the assistant to fix it: "fix the error shown in the preview".
