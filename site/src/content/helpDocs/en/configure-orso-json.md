---
title: Configure orso.json
description: Tell orso how to install and run your site when the defaults don't fit.
category: existing-site
order: 5
---
By default, orso runs `npm install` and `npm run dev` at the root of your repo. Add an `orso.json` at the repo root when your site needs something else. See [Is my site compatible?](/help/is-my-site-compatible/) for the full checklist.

## Example

```json
{
  "root": "site",
  "installDir": ".",
  "install": "pnpm install",
  "dev": "pnpm dev --port {port} --host {host}",
  "prLabels": ["orso", "content"]
}
```

## Fields

Every field is optional.

| Field        | What it does                                     | Default                                 |
| ------------ | ------------------------------------------------ | --------------------------------------- |
| `root`       | The folder that holds `astro.config.*`           | `.`                                     |
| `installDir` | Where to run the install command                 | The repo root                           |
| `install`    | The install command                              | `npm install`                           |
| `dev`        | The dev server command. It must contain `{port}` | `npm run dev` with orso's port and host |
| `prLabels`   | Labels added to pull requests orso opens         | `["orso"]`. Use `[]` for none           |

## When you need it

- You use pnpm, yarn or bun.
- Your Astro site is in a subfolder, as in a monorepo.
- You want different labels on orso's pull requests, so reviewers can filter them.
