# Agent guide — Astro marketing site

Static Astro site. The landing page, navigation and footer are written directly in code; Blog and Help articles live in Markdown content collections.

## Content model

- The content models are the site's own: collection schemas, loaders and preview paths live in `src/content/collections.js` as `collections`, and are meant to be edited (add a collection, add or change a field). Each collection is loaded with `orsoGlob` from `./integrations/orso/index.js` so Orso can edit it, and each entry is `{ collection, previewPath? }`, where `previewPath` returns the page an entry is shown on. `astro.config.mjs` imports `collections` and passes them to `orso({ collections })`; `src/content.config.ts` re-exports `collections` from `virtual:orso/collections`. When you change a field, update the pages and `src/lib/` helpers that read it and the existing Markdown that must still validate. `yaml`, `js-yaml`, `jsonc-parser`, and `linkedom` are project dependencies; there is no `orso-astro` npm package. Collections:
  - `blog` ← `src/content/blog/en/*.md`: `title`, `description`, `pubDate` (`YYYY-MM-DD`), optional `coverImage` + `coverAlt`, `categories[]` (`blogCategories` ids), `tags[]`, `relatedPosts[]` (`blog` ids).
  - `blogCategories` ← `src/content/blogCategories/en/*.md`: `title`, optional `description`. Page: `/blog/categories/{id}/`.
  - `helpDocs` ← `src/content/helpDocs/en/*.md`: `title`, `description`, `section` (a `helpCategories` id), optional `category` (a `helpDocsCategories` id), optional `subcategory` (a `helpSubcategories` id), `order`.
  - `helpCategories` ← `src/content/helpCategories/en/*.md`: the Help sections — `title`, optional `description`, `order`. Page: `/help/section/{id}/`. A `section` value that names no file (free text) still gets a section of that name.
  - `helpSubcategories` ← `src/content/helpSubcategories/en/*.md`: groups inside a section — `title`, `category` (a `helpCategories` id), `order`. Shown on the section page; no page of its own.
  - `helpDocsCategories` ← `src/content/helpDocsCategories/en/*.md`: cross-cutting Help topics — `title`, optional `description`, `order`. Page: `/help/categories/{id}/`.
  - An entry's id is its file name without `.md`; reference fields and URLs use that id.

## File map

- `src/site.ts` — site name, tagline, `description` (the default meta description), `url` (placeholder live address; `PUBLIC_SITE_URL` overrides it at build), default `ogImage`, header `nav` and `cta`, footer text/links and socials.
- `src/pages/index.astro` — the landing page: renders the section components in order.
- `src/components/sections/*.astro` — one component per landing-page section (`Hero`, `Logos`, `Features`, `Testimonials`, `Pricing`, `Faq`, `Cta`). Each holds its own copy; its root `<section id>` is the anchor used by nav links (`#features`, `#pricing`, …).
- `src/pages/blog/` — `/blog/` (all posts), `/blog/{id}/` (post), `/blog/categories/` and `/blog/categories/{id}/`.
- `src/pages/help/` — `/help/` (search, sections, every article), `/help/{id}/` (article with "On this page" and previous/next), `/help/section/{id}/`, `/help/categories/` and `/help/categories/{id}/`.
- `src/lib/blog.ts`, `src/lib/help.ts` — load the collections and work out ordering, categories, sections and related posts; pages call `loadBlog()` / `loadHelp()` instead of `getCollection` directly.
- `src/components/BlogCard.astro`, `BlogFilters.astro`, `HelpSearch.astro` — post card, category filter bar, and the Help search (a plain client-side `<script>` over the articles on the page; no search service). `Artwork.astro` draws the token-coloured landscape used when a post has no cover image; `PageHeader.astro` is the title block of inner pages; `DocList.astro` lists help articles; `Wordmark.astro` is the logo (same mark as `public/favicon.svg`); `LegalPage.astro` frames `src/pages/privacy.astro` and `terms.astro` (placeholder text: replace before launch).
- `src/layouts/Base.astro` — `<head>` (title template `Page | Acme`, description defaulting to `site.description`, canonical, Open Graph/Twitter tags, `theme-color` read from the tokens, favicon, the Albert Sans `<Font>`, JSON-LD from the `jsonLd` prop, `<ClientRouter />`), header, footer and the scroll-reveal script; imports `src/styles/global.css`.
- `src/lib/seo.ts` — `absoluteUrl()` (turns a site path into a full URL on the live site) and JSON-LD builders (`organization`, `website`, `breadcrumbs`). `src/lib/pages.ts` — every page of the site, for the sitemap.
- `src/pages/robots.txt.ts`, `sitemap.xml.ts`, `llms.txt.ts`, `llms-full.txt.ts` — generated at build from the pages and collections; new `.astro` pages are picked up by the sitemap automatically. `public/` holds `favicon.svg`, `apple-touch-icon.png` and the default social card `og-default.png` (1200×630).
- `src/styles/global.css` — Tailwind CSS v4 entry (`@import "tailwindcss"` plus the typography plugin), the `@theme` design tokens, their dark-mode values, small shared utilities (`page`, `label`, `link`, `button`, `button-quiet`), the `prose` colour mapping and the motion rules. There is no `tailwind.config.js`.
- `astro.config.mjs` — reads `PUBLIC_SITE_URL` (default `site.url`) for `site`, and registers the self-hosted Albert Sans font. Keep both, and keep `vite.resolve.preserveSymlinks` (previews run with `node_modules` symlinked in). The site has no `base`: orso previews it at the root of its own address, as on the live site.
- `integrations/orso/` — generated Orso integration sources (no content models). Relative imports only. Do not edit; a scaffold overwrites this folder.

## Conventions

- Landing-page copy: edit the section component directly. Nav, footer and socials: edit `src/site.ts`.
- New landing-page section: add a component in `src/components/sections/` and render it in `src/pages/index.astro`.
- New page: add an `.astro` route in `src/pages/` wrapped in `Base`.
- Blog posts and help docs: add or edit Markdown under `src/content/blog/en/` and `src/content/helpDocs/en/`. Categories, sections and topics are Markdown files too (frontmatter only), in their own folders under `src/content/`.
- New content type or field: edit `src/content/collections.js` (a new collection gets its own folder under `src/content/`, an `orsoGlob` loader and, if it has a page, a `previewPath`).
- Links: write plain root-relative paths (`/help/getting-started/`, `/#pricing`) in components, Markdown, scripts and CSS. They work the same in the live preview and on the live site. Use `absoluteUrl()` only where a full URL is needed (canonical links, social cards, feeds).
- Styling: use Tailwind utility classes in components. Wrap rendered Markdown bodies in `prose` (its colours already follow the tokens in both modes; no `prose-invert`). Avoid `<style>` blocks.
- Design tokens, never hard-coded values: every color, font, radius, shadow, spacing step and motion duration is a CSS variable in the `@theme` block of `src/styles/global.css`, and components use the matching utilities (`bg-surface`, `text-ink`, `text-muted`, `border-line`, `bg-accent`, `font-display`, `rounded-lg`, `shadow-soft`, `py-section`, `duration-gentle`, `ease-gentle`) or `var(--…)`. Do not write hex/rgb/hsl values, Tailwind palette colors (`neutral-600`, `white`, `blue-500`), arbitrary values (`text-[#333]`, `p-[13px]`) or inline `style` colors in components. Dark mode is handled by redefining the variables, not by `dark:` color variants. Need a value that doesn't exist? Add a token to `@theme` (and its dark value), document it in `DESIGN.md`, then use it. `npm test` runs `scripts/design-tokens.test.mjs`, which fails on colour literals, palette or arbitrary colour classes, `dark:` colour variants and inline style colours anywhere in `src/` outside the token definitions.
- Motion: keep it quiet. Inner-page titles use `rise`/`rise-2`/`rise-3`; add `data-reveal` to a landing-page section to fade it in on scroll. Page changes cross-fade through `<ClientRouter />`, so a `<script>` that touches the page must run on `document.addEventListener("astro:page-load", …)`, not at top level. All motion stops under `prefers-reduced-motion`.
- SEO: pass `title` and `description` to `Base` on every page (titles get ` | Acme` appended; keep them under 60 characters and descriptions under 160). Pass `jsonLd` for structured data that matches the visible content (pages default to a `WebPage`). External links use `rel="noopener noreferrer"`.
- `DESIGN.md` is the design source of truth. When you change styles, update both `DESIGN.md` and the `@theme` block in `src/styles/global.css`.
- Keep the dev server (`npm run dev`) running as-is; it is managed externally. Edits hot-reload in the live preview — just save the files and stop. Do **not** run `npm run build` (or `astro build`) after ordinary content/component edits; a full build is only for deploy or when the user explicitly asks to verify the production build.

## Skills

Project skills live in `.claude/skills/` (Claude Code) and `.agents/skills/` (Codex and others); `skills-lock.json` pins their sources, and `npx skills update` refreshes them. Load the matching skill before the work:

- Landing-page layout, visual design or a new section: `frontend-design`, then `cro` for structure and conversion.
- Writing new copy (headlines, sections, blog posts, help docs): `copywriting`. Tightening existing copy: `copy-editing`.
- SEO, metadata, structured data: `seo`. Accessibility: `accessibility`. Speed: `performance` and `core-web-vitals`.
- Astro routing, content collections, islands: `astro`.

## Checking your work

In a orso agent session, check the live preview after editing with `"$ORSO_HOOK_EXE" ctl preview-status` (it prints JSON; `devserver` is `starting`, `ready`, or `crashed`). Edits often hot-reload; do not build. If it is `crashed`, run `"$ORSO_HOOK_EXE" ctl restart-preview` once, then tell the user in plain words what happened.

After adding or renaming content (or when the Content sidebar looks stale), run `"$ORSO_HOOK_EXE" ctl refresh-content` so the app reloads **collections** and entry lists, then `"$ORSO_HOOK_EXE" ctl refresh-preview` so the preview page remounts. Cursor has no turn-end hook — call both before saying you are done; other agents also get this refresh when the turn ends.

## Deployment

Static `dist/` only (no SSR adapter). Default CI is Cloudflare Workers via `.github/workflows/deploy.yml` + `wrangler.toml` (secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`) — see `README.md`.

**Any host is fine** if the deploy workflow records GitHub Deployments: on success, set status `success` with an `http(s)` `environment_url` (PR head SHA → Preview link; `main` tip → Open live site). Without that URL, orso has nothing to link. When editing or replacing the workflow, always keep those deployment statuses.

`npm run build` (and thus CI deploy) runs Orso preview checks on the built HTML via `integrations/orso/`: **error**-severity findings fail the build; warnings are logged only. `linkedom` is a production dependency for those checks.
