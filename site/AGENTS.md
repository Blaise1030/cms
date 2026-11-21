# Agent guide — Astro marketing site

Static Astro site. The landing page, navigation and footer are written directly in code; Blog and Help articles live in Markdown content collections.

## Content model

- Collection schemas and loaders live in `integrations/orso/marketing.js` as `marketingCollections` (generated from `packages/astro/src`; do not edit the copy). `astro.config.mjs` imports `orso` and `marketingCollections` from `./integrations/orso/` and passes `blog` and `blogCategories` to `orso({ collections })`, alongside the hand-rolled help centre (`integrations/help.js`) and legal pages (`integrations/legal.js`); `src/content.config.ts` re-exports `collections` from `virtual:orso/collections`. `yaml`, `js-yaml`, `jsonc-parser`, and `linkedom` are project dependencies; there is no `orso-astro` npm package. Collections:
  - `blog` ← `src/content/blog/en/*.md`: `title`, `description`, `pubDate` (`YYYY-MM-DD`), optional `coverImage` + `coverAlt`, `categories[]` (`blogCategories` ids), `tags[]`, `relatedPosts[]` (`blog` ids).
  - `blogCategories` ← `src/content/blogCategories/en/*.md`: `title`, optional `description`. Page: `/blog/categories/{id}/`.
  - `helpDocs` ← `src/content/helpDocs/en/*.md` (defined in `integrations/help.js`): `title`, `description`, `category` (a `helpCategories` id), `order`.
  - `helpCategories` ← `src/content/helpCategories/en/*.md` (defined in `integrations/help.js`): `title`, optional `description`, `order`. Page: `/help/category/{id}/`. A `category` value that names no file (free text) still gets a category of that name.
  - An entry's id is its file name without `.md`; reference fields and URLs use that id.

## File map

- `src/site.ts` — site name, `url` (placeholder live address; `PUBLIC_SITE_URL` overrides it at build), default `ogImage`, header `nav` and `cta`, footer links and socials. Links name their label by `key`; the labels, tagline and description are in `src/i18n/messages/en.json`.
- `src/i18n/` — translations. `locales.ts` lists the languages (`en` is the source with no URL prefix; `zh` Simplified Chinese and `ms` Malay are served under `/zh/` and `/ms/`). Every visitor-facing string in the templates is in `messages/en.json` and read with `const { t, path } = getI18n(Astro.url)`; `path()` turns a site path into a link in the current language. `zh.json` and `ms.json`, and the Markdown under `src/content/<collection>/zh|ms/`, are written by Crowdin (`crowdin.yml`, `.github/workflows/crowdin.yml`): never edit them by hand. Missing keys and untranslated entries fall back to English. Pages live in `src/pages/[...locale]/` so each one is built once per language; legal pages, `llms.txt`, robots and the sitemap stay at the root.
- `src/pages/index.astro` — the landing page: renders the section components in order.
- `src/components/sections/*.astro` — one component per landing-page section (`Hero`, `Logos`, `Ops`, `Features`, `AgentBand`, `Proof`, `Faq`, `Cta`). UI comes from Nuxt UI (Vue): import components directly (`import UButton from "@nuxt/ui/components/Button.vue"`) and use them in `.astro` files, where they render to static HTML, or inside the Vue islands in `src/components/vue/` (hydrated with `client:*`). Do not hand-build buttons, badges, menus, dialogs, tabs, accordions or carousels. Icons are Hugeicons by full literal name (`i-hugeicons-…`, or `Icon.astro` by role) so Nuxt UI bundles them at build time. Nuxt UI's colours follow the tokens through the `--ui-*` bridge at the end of `src/styles/global.css`; its options (no router, no colour-mode switch, type roles for class merging) are in `astro.config.mjs`.
- `src/pages/blog/` — `/blog/` (all posts), `/blog/{id}/` (post), `/blog/categories/` and `/blog/categories/{id}/`.
- `src/pages/help/` — `/help/` (search, categories, every article), `/help/{id}/` (article with "On this page" and previous/next) and `/help/category/{id}/`.
- `src/lib/blog.ts`, `src/lib/help.ts` — load the collections and work out ordering, categories, sections and related posts; pages call `loadBlog()` / `loadHelp()` instead of `getCollection` directly.
- `src/components/BlogCard.astro`, `BlogFeatured.astro`, `BlogFilters.astro`, `BlogToc.astro`, `PagefindSearch.astro` — the featured post, grid card, category filter bar, the post table of contents, and the Blog/Help search (a button, or ⌘K / Ctrl+K, opening Nuxt UI's `UCommandPalette` in a `UModal`, in `src/components/vue/SiteSearch.vue`, querying Pagefind's JS API over the per-section indexes that `integrations/pagefind.js` writes to `dist/pagefind-blog/` and `dist/pagefind-help/` after `astro build`; under `astro dev` it says search works on the published site). `Artwork.astro` draws the token-coloured proof-sheet placeholder used when a post has no cover image; `PageHeader.astro` is the title block of inner pages; `DocList.astro` lists help articles; `BearMark.astro` is the bear mark, used by `Wordmark.astro` (the logo) and the footer; `Footer.astro` is the closing night band; `LegalPage.astro` frames `src/pages/privacy.astro` and `terms.astro` (placeholder text: replace before launch).
- `src/layouts/Base.astro` — `<head>` (title template `Page | Project Orso`, description defaulting to `site.description`, canonical, Open Graph/Twitter tags, `theme-color` read from the tokens, favicon, the Geist `<Font>`, JSON-LD from the `jsonLd` prop, `<ClientRouter />`), header, footer and the scroll-reveal script; imports `src/styles/global.css`.
- `src/lib/seo.ts` — `absoluteUrl()` (base- and site-aware) and JSON-LD builders (`organization`, `website`, `breadcrumbs`). `src/lib/pages.ts` — every page of the site, for the sitemap.
- `src/pages/robots.txt.ts`, `sitemap.xml.ts`, `llms.txt.ts`, `llms-full.txt.ts` — generated at build from the pages and collections; new `.astro` pages are picked up by the sitemap automatically. `public/` holds `favicon.svg`, `apple-touch-icon.png` and the default social card `og-default.png` (1200×630).
- `src/styles/global.css` — Tailwind CSS v4 entry (`@import "tailwindcss"` plus the typography plugin), the `@theme` design tokens, their dark-mode values, small shared utilities (`page`, `label`, `link`, `button`, `button-quiet`), the `prose` colour mapping and the motion rules. There is no `tailwind.config.js`.
- `src/lib/href.ts` — base-aware link helper.
- `astro.config.mjs` — reads `PUBLIC_BASE` (default `/`) for `base` and the HMR socket path (needed for the live preview proxy), `PUBLIC_SITE_URL` (default `site.url`) for `site`, and registers the self-hosted Geist font. Keep all of them, and keep `vite.resolve.preserveSymlinks` (previews run with `node_modules` symlinked in). Do not hard-code absolute paths; use `import.meta.env.BASE_URL`, `href()` or `absoluteUrl()`.
- `integrations/orso/` — generated Orso integration sources. Relative imports only. Do not edit; a scaffold overwrites this folder.

## Conventions

- Landing-page and template copy: edit `src/i18n/messages/en.json` (the component shows which key with `t("…")`); add a key there for any new visitor-facing text instead of writing it in the component. Nav, footer and socials links: edit `src/site.ts`.
- New landing-page section: add a component in `src/components/sections/` and render it in `src/pages/index.astro`.
- New page: add an `.astro` route in `src/pages/` wrapped in `Base`.
- Blog posts and help docs: add or edit Markdown under `src/content/blog/en/` and `src/content/helpDocs/en/`. Blog and help categories are Markdown files too (frontmatter only), in their own folders under `src/content/`.
- Links inside Markdown bodies: write them root-relative, the way the page appears on the site (`[Install orso](/help/install-orso/)`, `/blog/{id}/`). `integrations/base-links.js` (a Sätteri hast plugin wired in `astro.config.mjs`) adds `PUBLIC_BASE` to every root-relative `href`/`src` at build time, so they work in the live preview too. Don't use `../` paths for links between pages.
- Styling: use Tailwind utility classes in components. Wrap rendered Markdown bodies in `prose` (its colours already follow the tokens in both modes; no `prose-invert`). Avoid `<style>` blocks.
- Design tokens, never hard-coded values: every color, font, radius, shadow, spacing step and motion duration is a CSS variable in the `@theme` block of `src/styles/global.css`, and components use the matching utilities (`bg-surface`, `text-ink`, `text-muted`, `border-line`, `bg-night`, `text-status-live`, `font-display`, `rounded-lg`, `shadow-soft`, `py-section`, `duration-gentle`, `ease-gentle`) or `var(--…)`. Type uses only the six role utilities (`text-label`, `text-small`, `text-body`, `text-heading`, `text-title`, `text-display`); never numeric size classes such as `text-sm` (Tailwind's defaults stay defined only because Nuxt UI uses them internally; pass a role class through a component's `class`/`ui` to resize it). Do not write hex/rgb/hsl values, Tailwind palette colors (`neutral-600`, `white`, `blue-500`), arbitrary values (`text-[#333]`, `p-[13px]`) or inline `style` colors in components. Dark mode is handled by redefining the variables, not by `dark:` color variants. Need a value that doesn't exist? Add a token to `@theme` (and its dark value), document it in `DESIGN.md`, then use it. Status colours (`status-draft`, `status-review`, `status-live`, `status-danger`) are semantic; use them only for state, never decoration. `npm test` runs `scripts/design-tokens.test.mjs`, which fails on colour literals, palette or arbitrary colour classes, `dark:` colour variants and inline style colours anywhere in `src/` outside the token definitions.
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
