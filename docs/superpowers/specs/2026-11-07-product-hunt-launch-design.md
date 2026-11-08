# Product Hunt launch — design

Date: 2026-11-07

Status: approved.

## Intent

Launch Orso on Product Hunt as the **first** public launch (not an internal soft run
afterward, and not Windows). Visitors land on the existing marketing site, download a
**macOS** app (unsigned / not notarized for this launch), follow short “open anyway”
instructions, and complete a short first-run path.

Success means:

1. A GitHub Release ships installable DMGs for `0.1.0` (or the first public version
   chosen at cut time). Notarization and Developer ID signing are **out of scope** for
   this launch; updater artifacts are optional if the unsigned path makes them awkward.
2. A clean macOS account can download the matching DMG, install, clear Gatekeeper via
   documented self-accept steps (right-click → Open, or System Settings → Privacy &
   Security → Open Anyway), open Orso, create/open a project, and use preview + chat.
3. The marketing site’s `/download/` serves a real installer URL (not “coming soon”)
   and surfaces the Gatekeeper self-accept steps next to the download.
4. Site copy matches what the product actually publishes (GitHub workflow / Cloudflare
   Workers starter — not a false “GitHub Pages only” claim).
5. The Product Hunt listing links a **placeholder production origin** for now
   (`https://astro-marketing.blaisetiong-dev.workers.dev` until a real domain is bought).
   Do not invent a brand hostname in site or listing copy until one is purchased.

## Decisions

| Area | Decision |
| --- | --- |
| Launch order | Product Hunt first; Windows later. |
| Signing / notarization | **Not ready.** Ship unsigned (or ad-hoc) DMGs; users self-accept Gatekeeper. Notarized Developer ID builds are a post-launch follow-up. |
| Approach | Cut unsigned app release → prove install + Gatekeeper steps → wire site download + copy + open-anyway instructions → PH assets on placeholder origin. |
| Marketing site | Existing project at `/Users/blaise/orso-projects/astro-marketing` (not a new landing). |
| Download hosting | GitHub Release assets; site `download.url` points at the installer. |
| Platform | macOS only on the listing and download page. |
| Custom domain | **Placeholder.** No domain purchased yet. PH + site use `https://astro-marketing.blaisetiong-dev.workers.dev` until a hostname is chosen and attached. |
| First version | Target `0.1.0` via existing release flow adapted for unsigned desktop builds (root `package.json` is currently `0.0.0`; `CHANGELOG.md` must exist). |

## Two surfaces

| Surface | Path / repo | Role on PH day |
| --- | --- | --- |
| App | `Blaise1030/cms` (`/Users/blaise/Developer/cms`) | Unsigned DMG (both arches); notarization deferred |
| Site | Orso project `astro-marketing` (`/Users/blaise/orso-projects/astro-marketing`) | Pitch + `/download/` + Gatekeeper help + PH destination URL |

## Gate sequence

Do not advance the PH listing link until gates 1–3 are green. Gate 4 is placeholder-only
for this launch. Gate 5 can overlap once 1–3 are done.

### 1. Release plumbing (unsigned)

- **Do not** require the eight Apple / Tauri notarization secrets for this launch cut.
- Adapt `release.yml` desktop job (today: “unsigned or un-notarized builds are never
  shipped”) so a deliberate **unsigned launch** path can produce and upload DMGs.
  Prefer a clear flag/condition over silently skipping verification.
- Add `CHANGELOG.md` with a `## v0.1.0` (or chosen) section that `release.yml` can extract.
- Open/merge the release PR so `package.json` bumps from `0.0.0` and `release.yml` runs.
- Confirm the GitHub Release contains at least: `orso-*-aarch64.dmg`, `orso-*-x86_64.dmg`.
  Updater `.app.tar.gz` / `.sig` / `latest.json` are nice-to-have if easy without signing;
  otherwise defer in-app updates until the notarized follow-up.

### 2. Prove install (self-accept Gatekeeper)

Minimum for PH:

- Clean macOS user: download the matching DMG, install to Applications.
- Reproduce the Gatekeeper block once; clear it with the documented self-accept path;
  confirm the same steps work for a second person / fresh account.
- Create or open a project; confirm preview and chat.

Deferred (post-launch / notarized milestone):

- Local `src-tauri/scripts/update-e2e.sh`.
- Signed N→N+1 updater path from `docs/desktop-release.md`.

### 3. Wire the marketing site

In `astro-marketing`:

- Set `site.download.url` in `src/site.ts` to the public DMG URL (primary: Apple Silicon
  build) or to an interim page that links **both** arch DMGs. Keep `platform: "macOS"`.
- On `/download/`, add concise Gatekeeper self-accept instructions (macOS only): what
  users see, and how to Open Anyway. Do not hide this — unsigned launch depends on it.
- Replace “GitHub Pages” claims in site copy (`site.ts`, Hero, Features, HowItWorks,
  Stack, FAQs, page meta) with the real model: publish/push through GitHub; the starter
  template deploys via **Cloudflare Workers** (or “your repo’s deploy workflow”).
- Deploy so `/download/` no longer shows “coming soon”.

Header CTA already points at `/download/`; no new IA required.

### 4. Domain (placeholder)

- **No purchase yet.** Keep production origin as
  `https://astro-marketing.blaisetiong-dev.workers.dev`.
- Product Hunt “Get it” / website field = that workers.dev origin for this launch.
- Site `SITE` / `PUBLIC_SITE_URL` stay on workers.dev; no fake brand domain in copy.
- When a real domain is bought later: attach Worker custom domain, set
  `PUBLIC_SITE_URL`, redeploy, update PH listing URL.

### 5. Product Hunt assets

- Tagline aligned with DESIGN.md: free studio for your site, agent you already pay for.
- Explicit **macOS only**.
- Note that first open may require allowing an unidentified developer (honest expectation).
- Gallery: app screenshots + site hero.
- Link: placeholder origin from gate 4.

## Non-goals

- Windows / Linux desktop packaging.
- Developer ID signing, notarization, and stapling for this launch.
- Guaranteed in-app auto-update on the unsigned build.
- npm publish of `orso-astro` / changing the integrations copy model for launch.
- Migrating already-scaffolded user projects.
- Full documentation site rewrite beyond launch-critical copy on `astro-marketing`.
- Buying or attaching a custom domain before PH day.

## Risks

| Risk | Mitigation |
| --- | --- |
| Gatekeeper friction → bounce | Clear download-page + PH first-comment steps; test on a clean Mac account. |
| Release job still requires Apple secrets | Gate 1: explicit unsigned path in `release.yml` before bumping version. |
| PH crowd distrusts unsigned apps | State macOS + self-accept up front; plan notarized follow-up release. |
| Domain still unbought | Ship on workers.dev; treat custom domain as post-launch. |
| Copy still says GitHub Pages | Gate 3 is blocking; audit strings before deploy. |
| Only one arch linked | Document both DMGs or offer a chooser; primary CTA = aarch64. |

## Implementation outline

1. Unsigned desktop release path + `CHANGELOG.md` + release PR for `0.1.0`.
2. Clean-account install proof including Gatekeeper self-accept.
3. Marketing: download URL + open-anyway copy + publish-copy fix + deploy on workers.dev.
4. Domain remains placeholder; revisit when a hostname is purchased.
5. PH listing draft and schedule (macOS only, unsigned disclaimer).
