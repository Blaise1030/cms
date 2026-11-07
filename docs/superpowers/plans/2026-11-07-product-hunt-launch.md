# Product Hunt launch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an unsigned macOS `0.1.0` DMG via GitHub Releases and wire the `astro-marketing` site so Product Hunt visitors can download, self-accept Gatekeeper, and land on the workers.dev origin.

**Architecture:** Flip the existing `release.yml` desktop job behind an explicit `DESKTOP_UNSIGNED` repo variable so builds skip Apple/Tauri signing secrets, notarization verification, and updater artifacts. Marketing stays in `/Users/blaise/orso-projects/astro-marketing`: real DMG URLs, Open Anyway copy on `/download/`, and publish-host copy that matches Cloudflare Workers (not GitHub Pages-only).

**Tech Stack:** GitHub Actions, Tauri 2 (`src-tauri`), Astro marketing site, GitHub Releases.

**Spec:** `docs/superpowers/specs/2026-11-07-product-hunt-launch-design.md`

## Global Constraints

- Platform: macOS only (`aarch64` + `x86_64` DMGs).
- Signing: unsigned / not notarized for this launch; users self-accept Gatekeeper.
- Domain: `https://astro-marketing.blaisetiong-dev.workers.dev` only — no invented brand hostname.
- Updater (`.app.tar.gz`, `.sig`, `latest.json`): defer when unsigned; DMGs are required.
- Do not require the eight Apple / Tauri secrets while `DESKTOP_UNSIGNED` is enabled.
- Prefer a clear flag (`vars.DESKTOP_UNSIGNED == 'true'`) over silently skipping verification.
- Marketing copy must not claim GitHub Pages as the only / primary publish path; describe GitHub workflow + Cloudflare Workers (or “your repo’s deploy workflow”).
- Two repos: app `Blaise1030/cms` (`/Users/blaise/Developer/cms`); site `astro-marketing` (`/Users/blaise/orso-projects/astro-marketing`).

## Review Focus

- `DESKTOP_UNSIGNED` left `true` after a future notarized release → still ships unsigned; flip variable off and restore secret gate before that cut. (Task 1 docs + workflow comments.)
- Publish runs `desktop-latest-json.sh` without `.sig` files → release fails; skip updater manifest when unsigned. (Task 1.)
- Download page links only one arch → Intel Mac users bounce; page must expose both DMGs. (Task 4.)
- Gatekeeper steps omitted or buried → first-run fail; Open Anyway must appear on `/download/` whenever `url` is set. (Task 4.)
- Site still says “GitHub Pages” as the default host story → false claim on PH day. (Task 5.)

## File map

| File | Role |
| --- | --- |
| `.github/workflows/release.yml` | Unsigned desktop path via `vars.DESKTOP_UNSIGNED` |
| `.github/scripts/desktop-latest-json.sh` | Unchanged; publish skips calling it when unsigned |
| `docs/desktop-release.md` | Document unsigned launch mode + Gatekeeper |
| `CHANGELOG.md` | `## v0.1.0` section for release notes |
| `astro-marketing/src/site.ts` | `download.url` / `download.urlIntel` + platform |
| `astro-marketing/src/pages/download.astro` | Dual DMG CTAs + Gatekeeper self-accept copy |
| `astro-marketing/src/lib/faqs.ts`, `…/HowItWorks.astro` (+ audit) | Publish-host copy fix |
| `astro-marketing/scripts/content-pages.test.mjs` | Assert download + Gatekeeper + no “coming soon” |

---

### Task 1: Unsigned desktop path in `release.yml`

**Repo:** `Blaise1030/cms`

**Files:**
- Modify: `.github/workflows/release.yml` (`desktop` + `publish` jobs)
- Modify: `docs/desktop-release.md` (unsigned launch section; note notarization deferred)

**Interfaces:**
- Consumes: GitHub Actions variable `DESKTOP_UNSIGNED` (`true` / unset)
- Produces: when `vars.DESKTOP_UNSIGNED == 'true'`, desktop job uploads only `orso-<version>-<arch>.dmg` for both matrix arches; publish creates the GitHub Release with those DMGs and **does not** write/upload `latest.json`

- [ ] **Step 1: Set the repo variable**

```bash
gh variable set DESKTOP_UNSIGNED --body "true" --repo Blaise1030/cms
gh variable list --repo Blaise1030/cms | grep DESKTOP_UNSIGNED
```

Expected: `DESKTOP_UNSIGNED` = `true`

- [ ] **Step 2: Gate the secrets step**

In `.github/workflows/release.yml` `desktop` job, wrap “Require signing and notarization secrets” so it runs only when `vars.DESKTOP_UNSIGNED != 'true'`. Add a step (or `if:`) that when unsigned prints a clear notice: `DESKTOP_UNSIGNED=true; shipping unsigned DMGs`.

- [ ] **Step 3: Build without signing env when unsigned**

Keep `pnpm --prefix src-tauri tauri build --ci --target ${{ matrix.target }}`. When unsigned, **do not** pass `APPLE_*` or `TAURI_SIGNING_*` env vars. When signed (variable unset/false), keep today’s env block.

- [ ] **Step 4: Skip notarization verify when unsigned**

“Verify signature and notarization” runs only when `vars.DESKTOP_UNSIGNED != 'true'`.

- [ ] **Step 5: Package DMG-only when unsigned**

“Package” always copies the single `.dmg` to `desktop/orso-${version}-${arch}.dmg`. Copy `.app.tar.gz` + `.sig` only when **not** unsigned. Do not fail the job if updater files are absent in unsigned mode.

- [ ] **Step 6: Publish without updater manifest when unsigned**

In `publish`, run `.github/scripts/desktop-latest-json.sh` only when `vars.DESKTOP_UNSIGNED != 'true'`. `gh release create` still uploads whatever is in `dist/` (DMGs required).

- [ ] **Step 7: Document in `docs/desktop-release.md`**

Add an “Unsigned launch (Product Hunt)” subsection: set `DESKTOP_UNSIGNED=true`, expected assets (DMGs only), Gatekeeper self-accept, and that flipping the variable off restores the notarized path. Soften any absolute “never distribute unsigned” line so it allows this explicit mode.

- [ ] **Step 8: Commit**

```bash
git add .github/workflows/release.yml docs/desktop-release.md
git commit -m "$(cat <<'EOF'
ci(desktop): allow explicit unsigned DMG releases for PH launch

EOF
)"
```

---

### Task 2: `CHANGELOG.md` for `v0.1.0`

**Repo:** `Blaise1030/cms`

**Files:**
- Create: `CHANGELOG.md`

**Interfaces:**
- Produces: a top section exactly headed `## v0.1.0` that `release-pr.yml` / `release.yml` can `awk`-extract

- [ ] **Step 1: Create `CHANGELOG.md`**

Include `## v0.1.0` with short user-facing bullets: first public macOS desktop build, unsigned (Gatekeeper Open Anyway), Astro marketing studio basics (edit / preview / chat). No fake domain claims.

- [ ] **Step 2: Verify extract**

```bash
awk -v h="## v0.1.0" '$0 == h {on=1; next} on && /^## / {exit} on' CHANGELOG.md | head
```

Expected: non-empty notes body

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "$(cat <<'EOF'
docs: add CHANGELOG for v0.1.0 Product Hunt cut

EOF
)"
```

---

### Task 3: Cut `0.1.0` and confirm Release assets

**Repo:** `Blaise1030/cms`

**Files:**
- None in-repo beyond what `release-pr.yml` bumps (`package.json`, `Cargo.toml`, lockfiles as today)

**Interfaces:**
- Consumes: Tasks 1–2 on `main`; `DESKTOP_UNSIGNED=true`
- Produces: GitHub Release `v0.1.0` with `orso-0.1.0-aarch64.dmg` and `orso-0.1.0-x86_64.dmg`

- [ ] **Step 1: Open the release PR**

```bash
gh workflow run release-pr.yml -f version=0.1.0 --repo Blaise1030/cms
```

- [ ] **Step 2: Review and merge** the generated `chore(release): v0.1.0` PR after CI on the PR is green.

- [ ] **Step 3: Wait for `release.yml` on `main`**

Confirm `desktop` jobs succeed without Apple secrets and `publish` creates the release.

- [ ] **Step 4: Verify assets**

```bash
gh release view v0.1.0 --repo Blaise1030/cms
```

Expected: both DMGs present; `latest.json` absent (unsigned). Record the two asset URLs for Task 4.

---

### Task 4: Marketing download page — DMGs + Gatekeeper

**Repo:** `astro-marketing` (`/Users/blaise/orso-projects/astro-marketing`)

**Files:**
- Modify: `src/site.ts` (`download` shape)
- Modify: `src/pages/download.astro`
- Modify: `scripts/content-pages.test.mjs`

**Interfaces:**
- Consumes: Task 3 DMG URLs  
  `https://github.com/Blaise1030/cms/releases/download/v0.1.0/orso-0.1.0-aarch64.dmg`  
  `https://github.com/Blaise1030/cms/releases/download/v0.1.0/orso-0.1.0-x86_64.dmg`
- Produces: `site.download = { url, urlIntel, platform: "macOS" }` where `url` is Apple Silicon and `urlIntel` is x86_64; download page shows both buttons when set, never “coming soon”

- [ ] **Step 1: Extend `site.download` and set URLs** in `src/site.ts`

```ts
download: {
  url: "https://github.com/Blaise1030/cms/releases/download/v0.1.0/orso-0.1.0-aarch64.dmg",
  urlIntel: "https://github.com/Blaise1030/cms/releases/download/v0.1.0/orso-0.1.0-x86_64.dmg",
  platform: "macOS",
},
```

- [ ] **Step 2: Update `download.astro`**

Primary button: `Download for macOS (Apple Silicon)` → `url`. Secondary text link or button: `Download for Intel Mac` → `urlIntel`. Below CTAs, add a short “First open on macOS” note covering right-click → Open and System Settings → Privacy & Security → Open Anyway. Keep `platform: "macOS"` messaging. No invented custom domain.

- [ ] **Step 3: Failing-then-passing test** in `scripts/content-pages.test.mjs`

Add assertions against built `dist/download/index.html` (use existing root build from `npm test` / prebuilt dist pattern in that file):

- matches Apple Silicon DMG href
- matches Intel DMG href
- matches /Open Anyway/i (or the exact phrase you put in the page)
- does **not** match /coming soon/i

Run: `npm test` in `astro-marketing`  
Expected: FAIL before Step 2 content exists; PASS after.

- [ ] **Step 4: Commit** (in the marketing repo)

```bash
git add src/site.ts src/pages/download.astro scripts/content-pages.test.mjs
git commit -m "$(cat <<'EOF'
feat(download): wire v0.1.0 DMGs and Gatekeeper open-anyway steps

EOF
)"
```

---

### Task 5: Fix publish-host copy (not GitHub Pages-only)

**Repo:** `astro-marketing`

**Files:**
- Modify: `src/lib/faqs.ts` (publish FAQ answer)
- Modify: `src/components/sections/HowItWorks.astro` (publish step body)
- Audit/fix other **primary marketing** strings under `src/components` / `src/site.ts` that present GitHub Pages as the default. Blog/help historical posts may stay; launch-critical surfaces (Hero if needed, HowItWorks, FAQs, Stack) must match the starter’s Cloudflare Workers deploy story.

**Interfaces:**
- Produces: FAQ/HowItWorks (and any other audited primary surfaces) say publish/push via GitHub and deploy with **Cloudflare Workers** or “your repo’s deploy workflow” — not “GitHub Pages” as the sole host

- [ ] **Step 1: Rewrite the blocking strings**

Exact intent (wording may vary slightly): Orso publishes through your GitHub workflow; the starter deploys on Cloudflare Workers (or whatever host your repo already uses).

- [ ] **Step 2: Grep gate**

```bash
rg -n "GitHub Pages" src/components src/lib/faqs.ts src/site.ts
```

Expected: no remaining hits that claim Pages as the product default (Components/FAQ/site only).

- [ ] **Step 3: `npm test`**

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/lib/faqs.ts src/components/sections/HowItWorks.astro
# plus any other audited files
git commit -m "$(cat <<'EOF'
fix(copy): describe Cloudflare Workers publish path for launch

EOF
)"
```

---

### Task 6: Deploy marketing + manual install proof

**Repos:** `astro-marketing` deploy; manual macOS check against Release + site

**Files:** none required beyond deploying current marketing `main`

- [ ] **Step 1: Deploy** the marketing site so production workers.dev serves the new `/download/`.

- [ ] **Step 2: Smoke the live page**

Open `https://astro-marketing.blaisetiong-dev.workers.dev/download/` — both DMG links work; Open Anyway copy visible; no “coming soon”.

- [ ] **Step 3: Clean-account install proof**

On a macOS account that has never opened Orso: download matching arch DMG → install → hit Gatekeeper → clear via documented steps → create/open project → preview + chat work.

- [ ] **Step 4: Record result** in the PR/issue or a short note under the release (pass/fail). Do not schedule PH until this passes.

---

### Task 7: Product Hunt listing draft (assets only)

**Repo:** none (Product Hunt UI + screenshots)

**Interfaces:**
- Consumes: live workers.dev origin; macOS-only + unsigned disclaimer from the spec

- [ ] **Step 1: Draft listing** — tagline aligned with free studio / agent you already pay for; explicit macOS only; first-open may need Open Anyway; website = `https://astro-marketing.blaisetiong-dev.workers.dev`.

- [ ] **Step 2: Gallery** — app screenshots + site hero; no Windows claims.

- [ ] **Step 3: Hold publish** until Tasks 3–6 are green.

---

## Self-review notes

- Spec gates 1–3 → Tasks 1–6; gate 4 placeholder → no domain task; gate 5 → Task 7.
- Notarization / updater / custom domain explicitly deferred — not in task list as deliverables.
- `DESKTOP_UNSIGNED` flip-off is documented for the later notarized follow-up, not implemented here.
