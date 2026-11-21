---
title: The preview is blank or broken
description: Fix a preview that won't load, or loads without styles.
category: troubleshooting
order: 4
---

## The preview never loads

- **Install failed.** When you open a project, orso installs the site's dependencies first. If the install fails, the preview can't start. Check the message orso shows, and make sure `orso.json` ([Configure orso.json](../configure-orso-json/)) uses the right install command if you use pnpm or yarn.
- **No `dev` script.** The site's `package.json` needs a `dev` script that runs `astro dev`.
- **The site isn't at the repo root.** Set `root` in `orso.json` to the folder that holds `astro.config.*`.

## The page loads without styles, or links go to the wrong place

orso serves your site's preview under `/preview/{id}/`. Sites that assume they live at `/` break there.

- `base` in `astro.config.mjs` must read `PUBLIC_BASE`.
- Links and images must use `BASE_URL`, not a bare `/`.

This mostly affects existing sites. See [Is my site compatible?](../is-my-site-compatible/) and [Set up your site with the agent skill](../set-up-with-the-agent-skill/).

## The preview doesn't update when I edit

Live updates need the HMR socket set up as described in [Set up your site by hand](../set-up-by-hand/). Reloading the preview shows your latest saved edits in the meantime.

## The page shows an error

orso shows build and page errors over the preview. Read the message: it usually names the file and line. You can ask the assistant to fix it: "fix the error shown in the preview".
