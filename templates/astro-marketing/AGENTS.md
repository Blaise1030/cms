# Agent guide — Astro marketing site

Static Astro site. All copy and structure live in content files; components only render them.

## Content model

- `src/content.config.ts` — zod schemas for three collections:
  - `site` ← `src/content/site.json`: `name`, `description`, `nav[]`, `footer`, `socials[]`.
  - `pages` ← `src/content/pages/*.json`: `title`, `description`, `sections[]` (ordered list of section ids).
  - `sections` ← `src/content/sections/*.mdx`: one file per section instance. Frontmatter is a discriminated union on `type`: `hero | features | pricing | testimonials | faq | cta | logos`. The MDX body is optional supporting copy.
- A section id is its file name without the extension (`sections/faq.mdx` → `faq`). A page renders the sections it lists, in order; a section that is not listed is not shown.

## File map

- `src/pages/index.astro` — renders `pages/home.json` and its sections.
- `src/layouts/Base.astro` — header (from `site.nav`), footer, global styles.
- `src/components/sections/*.astro` — one component per section `type`; `index.ts` maps `type` → component.
- `src/lib/href.ts` — base-aware link helper.
- `astro.config.mjs` — reads `PUBLIC_BASE` (default `/`) for `base` and the HMR socket path (needed for the live preview proxy). Keep both. Do not hard-code absolute paths; use `import.meta.env.BASE_URL` or `href()`.

## Conventions

- Content edits: edit the JSON/MDX under `src/content/`. Do not put copy in components.
- New section instance: add `src/content/sections/<id>.mdx` and add `<id>` to `sections` in `pages/home.json`.
- New section type: add a schema variant to `sectionSchema` in `src/content.config.ts`, add a component in `src/components/sections/`, and register it in `components/sections/index.ts`.
- New page: add `src/content/pages/<slug>.json` and a route in `src/pages/`.
- Keep the dev server (`npm run dev`) running as-is; it is managed externally. Edits hot-reload in the live preview — just save the files and stop. Do **not** run `npm run build` (or `astro build`) after ordinary content/component edits; a full build is only for deploy or when the user explicitly asks to verify the production build.

## Deployment

Push-and-live on Cloudflare Pages. `.github/workflows/deploy.yml` runs `npm ci && npm run build` and deploys `dist/` on every push to `main`, and deploys a preview of every pull request branch (published as a GitHub deployment + sticky PR comment, see `README.md`); `wrangler.toml` names the Pages project (`pages_build_output_dir = ./dist`). Needs the `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` repo secrets. Keep the site fully static (no SSR adapter) and keep the build output in `dist/`.
