# Astro marketing site

Static Astro site styled with Tailwind CSS. The landing page is written in `src/components/sections/`
and `src/site.ts`; Blog and Help articles live in `src/content/` (see `AGENTS.md`).

```bash
npm ci
npm run dev     # local dev server
npm run build   # static build into ./dist
```

## Deploys (Cloudflare Pages)

`.github/workflows/deploy.yml` builds the site and deploys `dist/` to Cloudflare Pages:

- **Push to `main`** → production deploy.
- **Pull request** (opened, updated, reopened, marked ready) → preview deploy of the PR head
  commit (`wrangler pages deploy --branch=<head branch>`; the name is sanitised by
  `.github/scripts/pages-branch.sh` so it can't inject into the deploy command). The workflow then:
  - records a GitHub **deployment** for the PR head commit and sets its status to `success`
    with `environment_url` = the preview URL (or `failure` if the deploy failed);
  - posts one **sticky PR comment** with the preview link, updated on every push.

  The CMS workbench reads that deployment to show a **Preview** link beside the PR status
  ("Draft · Checks pending", "Open · Checks passing", …) in its preview browser bar.

### One-time setup

1. Create a Cloudflare Pages project (Direct Upload) and set `name` in `wrangler.toml` to it.
2. In the GitHub repo, **Settings → Secrets and variables → Actions**:
   - secret `CLOUDFLARE_API_TOKEN` — API token with the *Cloudflare Pages: Edit* permission;
   - secret `CLOUDFLARE_ACCOUNT_ID`;
   - variable `CLOUDFLARE_PAGES_PROJECT` (optional; defaults to `astro-marketing`).

The job asks for its own token permissions (`contents: read`, `deployments: write`,
`pull-requests: write`); nothing else needs enabling. Pull requests from forks get no secrets,
so they are skipped.
