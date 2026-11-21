---
name: setup-orso
description: >-
  Use when preparing an existing Astro site for orso (not created from the orso
  template), when orso preview fails to load, when Content is empty
  or read-only after opening a GitHub repo, when a repo still depends on
  orso-astro / file:packages/astro, or when asked to add or update orso(),
  orsoGlob, integrations/orso or orso.json.
---

# Set up a site for orso

For day-to-day editing inside orso after this, use `working-in-orso`.

orso runs a site's own dev server behind a proxy, edits its content collections through an
Astro integration, and publishes through GitHub pull requests. A repo works in orso when it
meets the requirements below. Work through the phases in order, tell the user what you found
in plain words, and **stop to ask before anything destructive** (rewriting a working deploy
workflow, renaming a branch, deleting files).

Talk to the user about pages, content, preview and publishing. Use git and build terms only
when they ask.

## Phase 1 — Check

Read `package.json`, `astro.config.*`, `src/content.config.*` (or `src/content/config.*`),
`.gitignore`, `.github/workflows/*`, and any `orso.json`. Report each item as ✓, ✗ or n/a.

| # | Requirement | Why | If missing |
|---|---|---|---|
| 1 | Default branch is `main` | orso hardcodes `main` for changes, sync and publish | Ask the user to rename the default branch on GitHub (Settings → Branches). Do not do it yourself. |
| 2 | Astro `>=7.3.0 <8` | The Content tab's integration needs it | Older Astro: offer to upgrade (`npx @astrojs/upgrade`). Not Astro (Next, Hugo…): stop — only Preview works, and only for Vite-based dev servers. |
| 3 | `package.json` has a `dev` script running `astro dev` | orso runs `npm run dev -- --port {port} --host {host} --ignore-lock` | Add `"dev": "astro dev"`. |
| 4 | Install works with the right package manager | orso runs `npm install` unless `orso.json` says otherwise, and opening fails if install fails | pnpm/yarn/bun lockfile → set `install` and `dev` in `orso.json` (Phase 2). |
| 5 | No preview-only `base`, `vite.server.hmr` or link-helper code | orso serves the preview at `/` and sets the HMR port itself | Phase 2, step 2. |
| 6 | orso integration installed (`integrations/orso/` + `orso()`) | Content tab, page checks | Phase 2, step 1. |
| 7 | Content collections use `orsoGlob` with a `*.md`, `*.mdx` or `*.json` pattern and a schema | Only those are editable in the Content tab | Phase 2, step 3. |
| 8 | `DESIGN.md` at the repo root | Design tab | Phase 2, step 4. |
| 9 | Deploy workflow records GitHub Deployments with `environment_url` | orso's Preview and Open live site links | Phase 3. |
| 10 | `.gitignore` covers `node_modules/`, `dist/`, `.astro/`, `.env` | orso writes `.astro/orso-content.json` and `node_modules/.orso-deps` | Add the missing lines. |
| 11 | `npm run build` at the repo root produces `dist/index.html` in under 90 s | Side-by-side previews when two people change the same thing | Tell the user; previews show "Unavailable" otherwise. |
| 12 | Pull request preview builds set `ORSO_PREVIEW=1` (production builds don't) | Review screenshots without a screen-sharing prompt | Phase 3. |

## Phase 2 — Wire it in

Make these edits on a new branch. Commit each step separately so the user can review.

### 1. Install the integration

The integration is plain ESM copied into the site; there is no npm package.

```sh
tmp=$(mktemp -d)
git clone --depth 1 --filter=blob:none --sparse https://github.com/Blaise1030/cms "$tmp"
git -C "$tmp" sparse-checkout set packages/astro/src
mkdir -p integrations/orso
rsync -a --delete --exclude 'marketing*.js' "$tmp/packages/astro/src/" integrations/orso/
rm -rf "$tmp"
```

`marketing*.js` are the starter template's collections; leave them out. Add a one-line
`integrations/orso/README.md` saying the folder is copied from orso's `packages/astro/src`
and is replaced on update — do not hand-edit it.

Add the runtime dependencies with the repo's package manager:
`yaml`, `js-yaml`, `jsonc-parser`, `linkedom` (all required; the config fails to load
without `linkedom`).

**Migrating from `orso-astro`** (a `"orso-astro": "file:packages/astro"` dependency, a
`sync:orso-astro`/`postinstall` script, or `import … from "orso-astro"`):

1. Before replacing it, diff the repo's `packages/astro/src` against the fresh copy. If the
   repo has fixes orso lacks, tell the user (they belong upstream in orso) and keep them in
   `integrations/orso/` for now.
2. Remove the `orso-astro` dependency, the `sync:orso-astro` and `postinstall` scripts, and
   `packages/astro/`.
3. Rewrite imports: `"orso-astro"` → `"./integrations/orso/index.js"` from the repo root,
   `"../integrations/orso/index.js"` from `src/`, and so on (relative paths only).
4. Reinstall so the lockfile drops `orso-astro`.

### 2. Add `orso()`

In `astro.config.mjs` add `orso()` **last** among integrations (it checks the built pages
after the others write theirs). Merge into the existing config; do not drop the user's settings.

```js
import { orso } from "./integrations/orso/index.js";

export default defineConfig({
  integrations: [/* existing integrations */, orso()],
});
```

orso previews the site at the root of its own address, so links work as written. Leave
root-relative links (`/about`, `src="/logo.svg"`, `url("/bg.png")`) alone, in components,
Markdown, scripts and CSS. `orso()` sets the HMR connection itself when orso runs the dev server.

Remove settings that only served an older orso:

- a `base` read from `PUBLIC_BASE`;
- a `vite.server.hmr` block read from `ORSO_DEV_PORT` or `ORSO_PROXY_PORT`;
- an `href()` link helper that prefixes `import.meta.env.BASE_URL` (its links become plain paths);
- code that strips a `/preview/{id}` prefix from `Astro.url` or generated URLs.

If the live site itself needs a `base` (for example a project page on GitHub Pages), keep it
and tell the user the preview opens at `/`, so they type the base path in the preview's address bar.

### 3. Content collections

For each collection the user wants to edit in orso, switch its loader to `orsoGlob`:

```ts
import { defineCollection } from "astro:content";
import { z } from "astro/zod";
import { orsoGlob } from "../integrations/orso/index.js";

const blog = defineCollection({
  loader: orsoGlob({ base: "./src/content/blog", pattern: "*.md" }),
  schema: z.object({ title: z.string(), pubDate: z.coerce.date() }),
});
```

- The pattern must be exactly `*.md`, `*.mdx` or `*.json`. Nested folders (`**/*.md`) are
  read-only in orso; give each folder its own collection, or tell the user.
- The collection needs a schema, and `base` must be inside the repo.
- A single settings-style file: `orsoGlob({ base, pattern: "*.json", singleton: "site" })`.
- So "Open preview" opens the right page, pass `orso({ collections: { blog: { collection: blog, previewPath: ({ slug }) => `/blog/${slug}/` } } })`
  in `astro.config.mjs` and re-export from `src/content.config.ts`:
  `export { collections } from "virtual:orso/collections";`.
  Otherwise keep the collections in `src/content.config.ts` as usual.

Collections the user doesn't need in orso can stay on Astro's `glob()`; they show as
read-only.

### 4. `DESIGN.md` and `orso.json`

- `DESIGN.md` at the repo root: YAML front matter for colours, fonts and spacing, then a short
  Markdown description of the look. If the site uses Tailwind v4, keep the same values in an
  `@theme` block in the main CSS file; orso's Design tab asks the assistant to keep the two in
  sync.
- `orso.json` only when the defaults don't fit. All fields are optional; `dev` must contain
  `{port}`:

```json
{
  "root": "site",
  "installDir": ".",
  "install": "pnpm install",
  "dev": "pnpm dev --port {port} --host {host}"
}
```

  `root` is the folder holding `astro.config.*` (default `.`).

### 5. Agent guide (only if asked)

If the user wants one and the repo has no `AGENTS.md`, add a short one: what the site is, the file map (pages,
components, content folders, styles), and these orso rules:

- The dev server is managed by orso; don't start, stop or build. Save files and stop.
- After editing, check `"$ORSO_HOOK_EXE" ctl preview-status`; if it is `crashed`, run
  `"$ORSO_HOOK_EXE" ctl restart-preview` once.
- After adding or renaming content, run `"$ORSO_HOOK_EXE" ctl refresh-content`, then
  `"$ORSO_HOOK_EXE" ctl refresh-preview`.
- Don't edit `integrations/orso/`.

## Phase 3 — Preview and live links (check, then ask)

Check whether the deploy already records GitHub Deployments and report it. Change or add
a workflow only when the user asks.

orso shows **Preview** for a change and **Open live site** for `main` from GitHub
Deployments: the newest status must be `success` with an http(s) `environment_url`, recorded
for the PR head commit (preview) and for `main` (live).

- **Already deploying through GitHub Actions** (Pages, Cloudflare, Netlify CLI, Vercel CLI,
  Fly…): add steps that create a deployment and set its status with `environment_url`, using
  `actions/github-script` and `permissions: deployments: write`. Keep the existing deploy
  steps. Show the user the diff before committing.
- **Deploying through a hosting provider's GitHub app** (Vercel, Netlify): those already
  create GitHub Deployments with URLs; check one in the repo's Environments page.
- **No deploy yet:** offer orso's Cloudflare Workers workflow
  (`templates/astro-marketing/.github/workflows/deploy.yml` and
  `.github/scripts/preview-alias.sh` in Blaise1030/cms, plus a `wrangler.toml`). The user
  sets secrets `CLOUDFLARE_API_TOKEN` (Workers Scripts: Edit) and `CLOUDFLARE_ACCOUNT_ID`,
  and optionally the variable `CLOUDFLARE_WORKER_NAME`.

For **pull request preview builds only**, set `ORSO_PREVIEW=1` in the build step's env (e.g.
`ORSO_PREVIEW: ${{ github.event_name == 'pull_request' && '1' || '' }}`). `orso()` then adds
its preview bridge to the preview, so reviewers in orso can screenshot areas of it without a
screen-sharing prompt. Never set it for production builds.

Recommend protecting `main` so publishing needs a review (orso shows a hint until it is).

## Phase 4 — Verify

Run these and fix what fails before reporting done:

1. Install with the repo's package manager.
2. `npx astro sync`, then confirm `.astro/orso-content.json` lists every collection you
   converted, with no `readOnlyReason` you didn't expect.
3. Start the dev server the way orso does, confirm it prints a `Local` URL and the home page
   loads at `/`, then stop it:
   `npm run dev -- --port 4399 --host 127.0.0.1`
4. `npm run build`: it must pass. orso's page checks run at the end; **errors** fail the
   build and need fixing, warnings are fine to report.

## Report

Finish with a short summary for the user:

- What now works in orso (Preview, Content with which collections, Design, Preview/live links).
- What they must do themselves (rename the branch, add secrets, protect `main`).
- Anything left as is, and why.
- How to open it: in orso, **New project → pick the repo from GitHub**, or re-open it if it
  is already there.

## Do not

- Rewrite the site to match the orso marketing template, or add its sample content
- Add or replace deploy workflows or `AGENTS.md` unless asked
- Push to `main`, force-push, or commit secrets

## Humans: install this skill

```bash
npx skills add Blaise1030/cms --skill setup-orso
```
