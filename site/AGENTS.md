# Agent guide — Astro marketing site

Static Astro site. The landing page, navigation and footer are written directly in code; Blog and Help articles live in Markdown content collections.

## Content model

- Collection schemas and loaders live in `integrations/orso/marketing.js` as `marketingCollections` (generated from `packages/astro/src`; do not edit the copy). `astro.config.mjs` imports `orso` and `marketingCollections` from `./integrations/orso/` and passes them to `orso({ collections: marketingCollections })`; `src/content.config.ts` re-exports `collections` from `virtual:orso/collections`. `yaml`, `js-yaml`, `jsonc-parser`, and `linkedom` are project dependencies; there is no `orso-astro` npm package. Collections:
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
- `src/components/sections/*.astro` — one component per landing-page section (`Hero`, `Logos`, `Ops`, `Features`, `AgentBand`, `Proof`, `Faq`, `Cta`). Interactive UI uses shadcn React islands in `src/components/react/` over CLI primitives in `src/components/ui/`. Mock frames use `AppWindow.astro` (shadcn Breadcrumb); status pills use `StatusChip` → `StatusBadge`.
- `src/pages/blog/` — `/blog/` (all posts), `/blog/{id}/` (post), `/blog/categories/` and `/blog/categories/{id}/`.
- `src/pages/help/` — `/help/` (search, sections, every article), `/help/{id}/` (article with "On this page" and previous/next), `/help/section/{id}/`, `/help/categories/` and `/help/categories/{id}/`.
- `src/lib/blog.ts`, `src/lib/help.ts` — load the collections and work out ordering, categories, sections and related posts; pages call `loadBlog()` / `loadHelp()` instead of `getCollection` directly.
- `src/components/BlogCard.astro`, `BlogFeatured.astro`, `BlogFilters.astro`, `BlogSearch.astro`, `BlogToc.astro`, `HelpSearch.astro` — the featured post, grid card, category filter bar, client-side blog search, the post table of contents, and the Help search (a plain client-side `<script>` over the articles on the page; no search service). `Artwork.astro` draws the token-coloured proof-sheet placeholder used when a post has no cover image; `PageHeader.astro` is the title block of inner pages; `DocList.astro` lists help articles; `BearMark.astro` is the bear mark, used by `Wordmark.astro` (the logo) and the footer; `Footer.astro` is the closing night band; `LegalPage.astro` frames `src/pages/privacy.astro` and `terms.astro` (placeholder text: replace before launch).
- `src/layouts/Base.astro` — `<head>` (title template `Page | Project Orso`, description defaulting to `site.description`, canonical, Open Graph/Twitter tags, `theme-color` read from the tokens, favicon, the Geist `<Font>`, JSON-LD from the `jsonLd` prop, `<ClientRouter />`), header, footer and the scroll-reveal script; imports `src/styles/global.css`.
- `src/lib/seo.ts` — `absoluteUrl()` (base- and site-aware) and JSON-LD builders (`organization`, `website`, `breadcrumbs`). `src/lib/pages.ts` — every page of the site, for the sitemap.
- `src/pages/robots.txt.ts`, `sitemap.xml.ts`, `llms.txt.ts`, `llms-full.txt.ts` — generated at build from the pages and collections; new `.astro` pages are picked up by the sitemap automatically. `public/` holds `favicon.svg`, `apple-touch-icon.png` and the default social card `og-default.png` (1200×630).
- `src/styles/global.css` — Tailwind CSS v4 entry (`@import "tailwindcss"` plus the typography plugin), the `@theme` design tokens, their dark-mode values, small shared utilities (`page`, `label`, `link`, `button`, `button-quiet`), the `prose` colour mapping and the motion rules. There is no `tailwind.config.js`.
- `src/lib/href.ts` — base-aware link helper.
- `astro.config.mjs` — reads `PUBLIC_BASE` (default `/`) for `base` and the HMR socket path (needed for the live preview proxy), `PUBLIC_SITE_URL` (default `site.url`) for `site`, and registers the self-hosted Geist font. Keep all of them, and keep `vite.resolve.preserveSymlinks` (previews run with `node_modules` symlinked in). Do not hard-code absolute paths; use `import.meta.env.BASE_URL`, `href()` or `absoluteUrl()`.
- `integrations/orso/` — generated Orso integration sources. Relative imports only. Do not edit; a scaffold overwrites this folder.

## Conventions

- Landing-page copy: edit the section component directly. Nav, footer and socials: edit `src/site.ts`.
- New landing-page section: add a component in `src/components/sections/` and render it in `src/pages/index.astro`.
- New page: add an `.astro` route in `src/pages/` wrapped in `Base`.
- Blog posts and help docs: add or edit Markdown under `src/content/blog/en/` and `src/content/helpDocs/en/`. Categories, sections and topics are Markdown files too (frontmatter only), in their own folders under `src/content/`.
- Links inside Markdown bodies are not base-aware: link to other articles by name, or use full URLs. A root-relative link (`/help/...`) skips `PUBLIC_BASE`, so it breaks in the live preview and in any build with a base path.
- Styling: use Tailwind utility classes in components. Wrap rendered Markdown bodies in `prose` (its colours already follow the tokens in both modes; no `prose-invert`). Avoid `<style>` blocks.
- Design tokens, never hard-coded values: every color, font, radius, shadow, spacing step and motion duration is a CSS variable in the `@theme` block of `src/styles/global.css`, and components use the matching utilities (`bg-surface`, `text-ink`, `text-muted`, `border-line`, `bg-night`, `text-status-live`, `font-display`, `rounded-lg`, `shadow-soft`, `py-section`, `duration-gentle`, `ease-gentle`) or `var(--…)`. Type uses only the six role utilities (`text-label`, `text-small`, `text-body`, `text-heading`, `text-title`, `text-display`); numeric size classes such as `text-sm` no longer exist. Do not write hex/rgb/hsl values, Tailwind palette colors (`neutral-600`, `white`, `blue-500`), arbitrary values (`text-[#333]`, `p-[13px]`) or inline `style` colors in components. Dark mode is handled by redefining the variables, not by `dark:` color variants. Need a value that doesn't exist? Add a token to `@theme` (and its dark value), document it in `DESIGN.md`, then use it. Status colours (`status-draft`, `status-review`, `status-live`, `status-danger`) are semantic; use them only for state, never decoration. `npm test` runs `scripts/design-tokens.test.mjs`, which fails on colour literals, palette or arbitrary colour classes, `dark:` colour variants and inline style colours anywhere in `src/` outside the token definitions.
- Motion: one orchestrated hero sequence (`.word`) and interaction motion only. Do not add scroll-triggered section reveals. Inner-page titles use `rise`/`rise-2`/`rise-3`; tab switching and other scripts must run on `document.addEventListener("astro:page-load", …)`, not at top level, because page changes cross-fade through `<ClientRouter />`. All motion stops under `prefers-reduced-motion`.
- SEO: pass `title` and `description` to `Base` on every page (titles get ` | Project Orso` appended; keep them under 60 characters and descriptions under 160). Pass `jsonLd` for structured data that matches the visible content (pages default to a `WebPage`). External links use `rel="noopener noreferrer"`.
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

Static `dist/` only (no SSR adapter). Default CI is Cloudflare Workers via the repo-root `.github/workflows/deploy-site.yml` in orso's repo + `wrangler.toml` (secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`) — see `README.md`.

**Any host is fine** if the deploy workflow records GitHub Deployments: on success, set status `success` with an `http(s)` `environment_url` (PR head SHA → Preview link; `main` tip → Open live site). Without that URL, orso has nothing to link. When editing or replacing the workflow, always keep those deployment statuses.

`npm run build` (and thus CI deploy) runs Orso preview checks on the built HTML via `integrations/orso/`: **error**-severity findings fail the build; warnings are logged only. `linkedom` is a production dependency for those checks.
