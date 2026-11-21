---
title: Is my site compatible?
description: Check what an existing Astro site needs before you use it with orso.
category: existing-site
order: 1
---

orso works best with sites made from its template, but an existing Astro site works too once it meets a few requirements. Use this checklist before you start.

## Must have

| Requirement | Why |
| --- | --- |
| The site is on GitHub and you have write access | orso clones, pushes and opens pull requests |
| The default branch is `main` | Changes, sync and publishing all work against `main` |
| Astro 7.3 or later (below 8) | The Content tab's integration needs it |
| A `dev` script in `package.json` that runs `astro dev` | orso starts the dev server for the live preview |
| `base` and the HMR socket read `PUBLIC_BASE` and `ORSO_DEV_PORT`, and links use `BASE_URL` | The preview is served under `/preview/{id}/` |

## Needed for each feature

| Requirement | Unlocks |
| --- | --- |
| orso's integration in `integrations/orso/`, `orso()` in the config, and the `yaml`, `js-yaml`, `jsonc-parser` and `linkedom` packages | The Content tab and page checks |
| Collections loaded with `orsoGlob` ([how](../make-collections-editable/)) (`*.md`, `*.mdx` or `*.json`, with a schema) | Editable content |
| A `DESIGN.md` at the repo root | The Design tab |
| A deploy that records GitHub deployments with an `environment_url` | Preview and **Open live site** links |
| An `orso.json` ([how](../configure-orso-json/)), if you use pnpm or yarn or the site isn't at the repo root | The right install and dev commands |

## Optional

- `prLabels` in `orso.json` sets the labels on pull requests orso opens. The default is `["orso"]`. Use `[]` for none.
- Pull request preview builds with `ORSO_PREVIEW=1` (never in production) let orso take review screenshots without a screen-sharing prompt.
- Push access to create an `orso-review-assets` branch lets orso attach screenshots to review comments. orso deletes the branch when no open change needs it, and you can delete it at any time.

## Not using Astro?

If your site uses Next.js, Hugo or another framework, only the live preview can work, and only for Vite-based dev servers. Content editing needs Astro.

## Next step

The easiest way to meet these requirements is [Set up your site with the agent skill](../set-up-with-the-agent-skill/). If you would rather make the changes yourself, see [Set up your site by hand](../set-up-by-hand/).
