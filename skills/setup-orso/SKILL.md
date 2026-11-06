---
name: setup-orso
description: >-
  Use when preparing an existing Astro site for Orso (not created from the Orso
  template), when Orso preview fails under /preview/{id}/, when Content is empty
  or read-only after opening a GitHub repo, or when asked to add orso-astro,
  orso(), orsoGlob, PUBLIC_BASE, or orso.json.
---

# Setup Orso on an existing Astro site

Make a non-template Astro repo work in Orso: live preview under `/preview/{id}/` and editable Content collections. For day-to-day editing inside Orso after this, use `working-in-orso`.

**Requires Astro `>=7.3.0 <8`.** Non-Astro sites are out of scope.

## Checklist

Do only what is missing. Prefer a draft change / branch; do not push to `main` unless asked.

1. **Locate the Astro root** (directory with `astro.config.*` + `package.json`). If it is not the repo root, write `orso.json` at the **repo** root:

```json
{
  "root": "apps/web",
  "installDir": "apps/web",
  "install": "npm install --no-audit --no-fund --no-progress",
  "dev": "npm run dev -- --port {port} --host {host} --ignore-lock"
}
```

Omit `orso.json` when defaults (`.` / npm) are fine. Empty strings are invalid — omit keys instead.

2. **Vendor `orso-astro`** into `<astro-root>/packages/astro` (not published to npm). From a temp dir:

```bash
git clone --depth 1 --filter=blob:none --sparse https://github.com/Blaise1030/cms.git /tmp/orso-src
git -C /tmp/orso-src sparse-checkout set packages/astro
cp -R /tmp/orso-src/packages/astro <astro-root>/packages/astro
```

In `<astro-root>/package.json` set `"orso-astro": "file:packages/astro"`, then install with the project’s package manager.

3. **Wire preview + integration** in `astro.config.*`:

```js
import { orso } from "orso-astro";

const base = process.env.PUBLIC_BASE ?? "/";
const hmrPort = Number(process.env.ORSO_DEV_PORT || process.env.ORSO_PROXY_PORT || 0);

export default defineConfig({
  base,
  integrations: [/* existing */, orso()],
  vite: {
    // keep existing plugins
    server: {
      hmr: {
        path: `${base}vite-hmr`.replace(/\/{2,}/g, "/"),
        ...(hmrPort > 0 ? { clientPort: hmrPort } : {}),
      },
    },
  },
});
```

Merge with existing `vite` / `integrations` — do not drop them. Orso sets `PUBLIC_BASE=/preview/{id}/` when it runs `dev`.

4. **Content loaders** in `src/content.config.ts` (or equivalent):

- `import { orsoGlob } from "orso-astro"`
- Replace `glob({...})` from `astro/loaders` with `orsoGlob({...})` (same options)
- Leave non-`glob` loaders alone (they stay listed but read-only in Orso)

5. **Base-aware links:** under a non-`/` base, hard-coded `/...` hrefs break. Prefer `import.meta.env.BASE_URL` (or a small `href()` helper) for in-site links.

6. **Verify:** reinstall if needed; in an Orso session run `"$ORSO_HOOK_EXE" ctl restart-preview` then `preview-status`. Confirm `.astro/orso-content.json` appears after `astro sync` / `astro dev`, and Content shows collections as editable when loaders use `orsoGlob`.

## Do not

- Add Cloudflare deploy workflows, `AGENTS.md`, or template sample content unless asked
- Rewrite the site to match the marketing template
- Commit secrets or force-push

When you *are* asked to add or fix deploy workflows: record GitHub **Deployments** for both production (`main`, with live `environment_url`) and PR previews (with preview `environment_url`). orso’s **Open live site** / preview links read those statuses — see `templates/astro-marketing/.github/workflows/deploy.yml` and `working-in-orso`.

## Humans: install this skill

```bash
npx skills add Blaise1030/cms --skill setup-orso
```
