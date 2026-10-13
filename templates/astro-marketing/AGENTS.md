# Agent guide — Astro marketing site

Static Astro site. The landing page, navigation and footer are written directly in code; Blog and Help articles live in Markdown content collections.

## Content model

- `src/content.config.ts` wires the collections defined in `src/content-schemas.mjs`:
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

## Conventions

- Landing-page copy: edit the section component directly. Nav, footer and socials: edit `src/site.ts`.
- New landing-page section: add a component in `src/components/sections/` and render it in `src/pages/index.astro`.
- New page: add an `.astro` route in `src/pages/` wrapped in `Base`.
- Blog posts and help docs: add or edit Markdown under `src/content/blog/en/` and `src/content/helpDocs/en/`.
- Styling: use Tailwind utility classes in components (with `dark:` variants for dark mode). Wrap rendered Markdown bodies in `prose dark:prose-invert`. Avoid `<style>` blocks.
- `DESIGN.md` is the design source of truth. When you change styles, update both `DESIGN.md` and the `@theme` block in `src/styles/global.css` (create it if missing).
- Keep the dev server (`npm run dev`) running as-is; it is managed externally. Edits hot-reload in the live preview — just save the files and stop. Do **not** run `npm run build` (or `astro build`) after ordinary content/component edits; a full build is only for deploy or when the user explicitly asks to verify the production build.

## Deployment

Push-and-live on Cloudflare Pages. `.github/workflows/deploy.yml` runs `npm ci && npm run build` and deploys `dist/` on every push to `main`, and deploys a preview of every pull request branch (published as a GitHub deployment + sticky PR comment, see `README.md`); `wrangler.toml` names the Pages project (`pages_build_output_dir = ./dist`). Needs the `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` repo secrets. Keep the site fully static (no SSR adapter) and keep the build output in `dist/`.
