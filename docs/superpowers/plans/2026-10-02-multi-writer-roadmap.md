# Multi-writer orso — Roadmap

Each phase below gets its own implementation plan (`docs/superpowers/plans/`) when it starts,
and each ships working software on its own.

## Decisions and assumptions

- **Everyone runs the daemon locally.** Blog writers, help-doc authors, translators and
  developers all download `orso`. There is no hosted CMS (Keystatic/Tina were rejected).
- **Everyone has a GitHub account.** GitHub is the shared store, the identity, and the review
  surface (pull requests).
- **Cloudflare access is limited.** Writers never touch Cloudflare. Preview and production
  deploys run in GitHub Actions using repo secrets an admin sets once
  (`CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`). The template's
  `.github/workflows/deploy.yml` already deploys `main`.
- Content lives in the site repo as Markdown/MDX in Astro content collections, with images in
  `src/assets/`. Locales live in per-locale folders (`src/content/blog/en/…`, `…/fr/…`).

## Phase 1 — GitHub sign-in, open a repo, real authors

Plan: `2026-10-02-phase1-github-signin.md`.

- Sign in with GitHub from the app (OAuth device flow); the session survives restarts.
- "Open from GitHub": clone any repo the user can access as a project.
- Commits (checkpoints and the agent's own) are authored by the signed-in person instead of
  the hardcoded `orso` identity.

## Phase 2 — Changes, preview links, review

- One branch per change ("New post: Launch week" → `content/launch-week`); a list of open changes.
- **Share preview**: push the branch and open/update a draft PR.
- Template workflow: also run on `pull_request`, deploy with `--branch=<head>` and post the
  Cloudflare Pages preview URL (`<branch>.<project>.pages.dev`) as a PR comment / GitHub deployment.
- The app reads PR status, checks and the preview URL with the user's token and shows them in
  the browser bar.
- **Get latest**: pull `main`, update open branches; conflict view (mine / theirs / both) when
  two people edited the same file.
- Recommend branch protection on `main` (merging needs review).

## Phase 3 — Content editor

- Content view: collections (Blog, Help docs) and their entries; new, rename, delete.
- Frontmatter form generated from the Astro collection schema.
- Tiptap editor with the official markdown extension (round-trips `.md`), toolbar and slash
  commands, plus a raw-markdown toggle.
- Images: paste, drop or upload → saved to `src/assets/<collection>/<slug>/`, inserted as a
  relative link, alt text required.

## Phase 4 — Localisation

- Astro `i18n` config (`defaultLocale`, `locales`, `fallback`), per-locale content folders,
  `src/i18n/<locale>.json` + `t()` for UI strings, `hreflang` and an i18n sitemap.
- Locale switcher in the Content view and the preview.
- Translation status per entry: translated / outdated (source changed since, from git
  history) / missing.
- Side-by-side editor (source read-only, translation editable).
- "Translate with agent": drafts the target file preserving markdown, links and frontmatter
  keys; marked `translation: draft` until a reviewer sets `reviewed`.
- If the locale count grows, a Crowdin/Lokalise GitHub integration can own translations; the
  status view keeps working because it only reads files.

## Phase 5 — Packaging and writer mode

- Release binaries (GitHub Releases / Homebrew tap); `orso` opens the browser already
  authenticated (no token copy-paste).
- Startup checks for Node and git with a friendly install screen.
- `gh auth token` as a sign-in shortcut when the GitHub CLI is installed.
- Writer mode: Content, Preview, History only; the agent limited to writing help
  (drafting, translating, proofreading), no shell.
- Windows support (the dev-server supervisor is Unix-only today).
