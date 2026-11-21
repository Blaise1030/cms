---
title: Set up your site with the agent skill
description: Let a coding agent check your repo and wire orso in for you.
category: existing-site
order: 2
---

orso ships a skill that teaches coding agents how to set up a site for orso. This is the recommended way to bring an existing site in.

## 1. Install the skills

In your site's repo, run:

```sh
npx skills add Blaise1030/cms --skill setup-orso       # adopt or update orso in a repo
npx skills add Blaise1030/cms --skill working-in-orso  # day-to-day editing rules
```

## 2. Ask your agent

Open your coding agent (such as Claude Code, Codex or Cursor) in the repo and ask:

> Set up this site for orso.

## What the skill does

1. **Checks the repo** against every requirement in **Is my site compatible?** and reports each one as passing, failing or not applicable.
2. **Wires everything in on a new branch,** with a separate commit for each step so you can review them:
   - copies orso's integration into `integrations/orso/` and adds its dependencies
   - routes `base` and the HMR socket through orso's settings
   - makes links base-aware
   - switches the collections you want to edit to `orsoGlob`
   - adds `DESIGN.md` and, if needed, `orso.json`
3. **Migrates older sites** that still use the `orso-astro` package.
4. **Verifies** that the dev server starts and the content manifest builds before it reports back.

## 3. Review and merge

Read through the branch, open a pull request and merge it into `main`. Then open the repo in orso with **Open a project**.

## Updating later

Run the same request again to update orso's integration to the latest version.
