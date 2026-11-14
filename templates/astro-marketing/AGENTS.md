# Agent guide — Astro marketing site

Static Astro site. The landing page, navigation and footer are written directly in code; Blog and Help articles live in Markdown content collections.

## Content model

- Collection schemas and loaders live in `integrations/orso/marketing.js` as `marketingCollections` (generated from `packages/astro/src`; do not edit the copy). `astro.config.mjs` imports `orso` and `marketingCollections` from `./integrations/orso/` and passes them to `orso({ collections: marketingCollections })`; `src/content.config.ts` re-exports `collections` from `virtual:orso/collections`. `yaml`, `js-yaml`, `jsonc-parser`, and `linkedom` are project dependencies; there is no `orso-astro` npm package. Collections:
  - `blog` ← `src/content/blog/en/*.md`: `title`, `description`, `pubDate` (`YYYY-MM-DD`), optional `coverImage` + `coverAlt`, `tags[]`, `relatedPosts[]`.
  - `helpDocs` ← `src/content/helpDocs/en/*.md`: `title`, `description`, `section`, `order`.

## File map

- `src/site.ts` — site name, header `nav`, footer text/links and socials.
- `src/pages/index.astro` — the landing page: renders the section components in order.
- `src/components/sections/*.astro` — one component per landing-page section (`Hero`, `Logos`, `Features`, `Testimonials`, `Pricing`, `Faq`, `Cta`). Each holds its own copy; its root `<section id>` is the anchor used by nav links (`#features`, `#pricing`, …).
- `src/pages/blog/`, `src/pages/help/` — index and article routes for the collections.
- `src/layouts/Base.astro` — header and footer (from `src/site.ts`); imports `src/styles/global.css`.
- `src/styles/global.css` — Tailwind CSS v4 entry (`@import "tailwindcss"` plus the typography plugin). Theme tokens go in an `@theme` block here; there is no `tailwind.config.js`.
- `src/lib/href.ts` — base-aware link helper.
- `astro.config.mjs` — reads `PUBLIC_BASE` (default `/`) for `base` and the HMR socket path (needed for the live preview proxy). Keep both. Do not hard-code absolute paths; use `import.meta.env.BASE_URL` or `href()`.
- `integrations/orso/` — generated Orso integration sources. Relative imports only. Do not edit; a scaffold overwrites this folder.

## Conventions

- Landing-page copy: edit the section component directly. Nav, footer and socials: edit `src/site.ts`.
- New landing-page section: add a component in `src/components/sections/` and render it in `src/pages/index.astro`.
- New page: add an `.astro` route in `src/pages/` wrapped in `Base`.
- Blog posts and help docs: add or edit Markdown under `src/content/blog/en/` and `src/content/helpDocs/en/`.
- Styling: use Tailwind utility classes in components (with `dark:` variants for dark mode). Wrap rendered Markdown bodies in `prose dark:prose-invert`. Avoid `<style>` blocks.
- `DESIGN.md` is the design source of truth. When you change styles, update both `DESIGN.md` and the `@theme` block in `src/styles/global.css` (create it if missing).
- Keep the dev server (`npm run dev`) running as-is; it is managed externally. Edits hot-reload in the live preview — just save the files and stop. Do **not** run `npm run build` (or `astro build`) after ordinary content/component edits; a full build is only for deploy or when the user explicitly asks to verify the production build.

## Checking your work

In a orso agent session, check the live preview after editing with `"$ORSO_HOOK_EXE" ctl preview-status` (it prints JSON; `devserver` is `starting`, `ready`, or `crashed`). Edits often hot-reload; do not build. If it is `crashed`, run `"$ORSO_HOOK_EXE" ctl restart-preview` once, then tell the user in plain words what happened.

After adding or renaming content (or when the Content sidebar looks stale), run `"$ORSO_HOOK_EXE" ctl refresh-content` so the app reloads **collections** and entry lists, then `"$ORSO_HOOK_EXE" ctl refresh-preview` so the preview page remounts. Cursor has no turn-end hook — call both before saying you are done; other agents also get this refresh when the turn ends.

## Deployment

Static `dist/` only (no SSR adapter). Default CI is Cloudflare Workers via `.github/workflows/deploy.yml` + `wrangler.toml` (secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`) — see `README.md`.

**Any host is fine** if the deploy workflow records GitHub Deployments: on success, set status `success` with an `http(s)` `environment_url` (PR head SHA → Preview link; `main` tip → Open live site). Without that URL, orso has nothing to link. When editing or replacing the workflow, always keep those deployment statuses.

`npm run build` (and thus CI deploy) runs Orso preview checks on the built HTML via `integrations/orso/`: **error**-severity findings fail the build; warnings are logged only. `linkedom` is a production dependency for those checks.
