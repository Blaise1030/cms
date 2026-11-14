# Astro marketing site

Static Astro site styled with Tailwind CSS. The landing page is written in `src/components/sections/`
and `src/site.ts`; Blog and Help articles live in `src/content/` (see `AGENTS.md`).

```bash
npm ci
npm run dev     # local dev server
npm run build   # static build into ./dist
```

## Deploys

The site is a static `dist/` build. The starter ships Cloudflare Workers CI; any other static
host works the same for orso as long as your workflow records GitHub Deployments (see below).

### What orso needs (any host)

orso’s **Preview** (on a change) and **Open live site** (on `main`) links come only from
GitHub Deployments — not from the host brand. For each successful deploy, create a GitHub
Deployment and set its latest status to `success` with an `http(s)` `environment_url`:

- **PR / change preview** → deployment for the PR **head SHA**, `environment_url` = preview URL
- **Production / live site** → deployment for the current **`main` tip**, `environment_url` = live URL

Without that `environment_url`, those buttons stay empty. Local in-app preview does not use
Deployments.

### Default: Cloudflare Workers

`.github/workflows/deploy.yml` builds the site and deploys `dist/` to Cloudflare Workers as static
assets (no Worker script; `wrangler.toml` points `[assets]` at `./dist`):

- **Push to `main`** → production deploy (`wrangler deploy`), served at
  `https://<worker>.<subdomain>.workers.dev`. The workflow records a GitHub **deployment**
  for the push commit (`environment: production`) and sets its status to `success` with
  `environment_url` = the live site URL (from wrangler output, or repo variable
  `PUBLIC_SITE_URL` when set).
- **Pull request** (opened, updated, reopened, marked ready) → preview of the PR head commit
  (`wrangler versions upload --preview-alias=<alias>`). The alias comes from the head branch
  (`content/launch-week` → `content-launch-week`), made safe and short enough by
  `.github/scripts/preview-alias.sh`, so every branch keeps one stable preview URL:
  `https://<alias>-<worker>.<subdomain>.workers.dev`. Previews never touch production. The
  workflow then:
  - records a GitHub **deployment** for the PR head commit and sets its status to `success`
    with `environment_url` = the preview URL (or `failure` if the upload failed);
  - posts one **sticky PR comment** with the preview link, updated on every push.

### One-time setup (Cloudflare)

1. Set `name` in `wrangler.toml` to your Worker's name (the first deploy creates it). Keep
   `workers_dev` and `preview_urls` on: preview URLs live on `workers.dev`.
2. In the GitHub repo, **Settings → Secrets and variables → Actions**:
   - secret `CLOUDFLARE_API_TOKEN` — API token with the *Workers Scripts: Edit* permission;
   - secret `CLOUDFLARE_ACCOUNT_ID`;
   - variable `CLOUDFLARE_WORKER_NAME` (optional; defaults to `astro-marketing`);
   - variable `PUBLIC_SITE_URL` (optional; canonical live origin, e.g. a custom domain — used
     for the production GitHub deployment URL when wrangler’s output is ambiguous).

The job asks for its own token permissions (`contents: read`, `deployments: write`,
`pull-requests: write`); nothing else needs enabling. Pull requests from forks get no secrets,
so they are skipped.

### Other hosts

Replace or adapt `deploy.yml` (and drop `wrangler.toml` if unused). Keep the site fully static
(no SSR adapter), build into `dist/`, and keep the Deployments contract above so Preview and
Open live site still work.
