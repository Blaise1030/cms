# Phase 2 — Changes, preview links, review (spec)

Source: roadmap `docs/superpowers/plans/2026-10-02-multi-writer-roadmap.md` § Phase 2.
Status: **approved 2026-10-04**. All *(proposed)* decisions below were accepted as written.
Tracker: GitHub issue #21 on `Blaise1030/cms` (milestone #1).

## Phase 0 findings

- **Tracker:** GitHub Issues, `Blaise1030/cms`, issue #21 as parent (one issue per phase).
- **CI:** this repo (`Blaise1030/cms`) has **no workflows** under `.github/workflows`. Nothing runs
  on pull requests. Checks today are local only: `just test` = web build + `cargo test` +
  `pnpm -C web test` (vitest) + template build. → Q2.
- **Template CI:** `templates/astro-marketing/.github/workflows/deploy.yml` deploys `main` to
  Cloudflare Pages on push; no `pull_request` trigger.
- **E2E:** none. Rust integration tests in `tests/*.rs`, vitest unit/component tests in `web/src`.
  → plan needs a **Slice 0** (e2e harness). → Q3.
- **Conventions:** CLAUDE.md only covers context-mode routing. Exemplars: `src/git.rs`,
  `src/github.rs`, `src/http/*.rs` (Axum handlers + `ApiError` codes), `web/src/modules/<x>/`
  (Pinia `store.ts` + components + colocated `*.test.ts`), commit style `feat(web): …` / `fix: …`.
- **Working tree:** branch `docs/readme-dev-mode` has uncommitted element-picker work. Phase 2
  branches should start from `main`; that work needs merging or stashing first. → Q10.

## User flows (actor does X → sees Y)

1. **Start a change.** Writer clicks "New change", types "New post: Launch week" → app creates
   and checks out `content/launch-week` from latest `main`; the header shows the active change.
2. **List / switch changes.** Writer opens the Changes list → sees their open change branches
   (title, last edited, PR state if any); clicking one switches the working tree and preview to it.
3. **Edit inside a change.** Agent edits and checkpoints commit to the active change branch, not
   `main`. *(proposed)* Editing while on `main` prompts "Start a change first".
4. **Share preview.** Writer clicks "Share preview" → branch is pushed, a **draft PR** is opened
   (or updated if one exists); the app shows the PR link.
5. **Preview deploy.** PR opened/updated → the template workflow builds and deploys with
   `--branch=<head>` and publishes the preview URL `<branch>.<project>.pages.dev` as a GitHub
   deployment (and a PR comment).
6. **Status in the browser bar.** Writer sees PR state (draft / open / approved / merged / closed),
   checks (pending / passing / failing) and the preview URL (click to open) for the active change,
   refreshed with the user's token.
7. **Ready for review.** *(proposed)* Writer clicks "Ready for review" → draft PR becomes ready.
   Merging happens on GitHub (link out).
8. **Get latest.** Writer clicks "Get latest" → `main` is pulled; open change branches are updated
   from `main` *(proposed: merge, not rebase, so no force-push)*; clean updates just succeed.
9. **Conflict.** Two people edited the same file → Get latest shows a conflict view per file with
   **mine / theirs / both** choices; resolving all files completes the update; abandoning restores
   the branch to its pre-update state.
10. **Merged change.** PR merged on GitHub → the change shows as merged; *(proposed)* "Clean up"
    deletes the local branch and returns to `main`.
11. **Branch protection.** Admin reads docs (and *(proposed)* sees a one-time hint in the app when
    `main` is unprotected) recommending "require review before merging".

## Out of scope

Content editor / frontmatter forms (Phase 3), localisation (Phase 4), packaging/writer mode
(Phase 5), merging PRs from inside the app, PR review comments in the app, setting branch
protection via API, non-GitHub remotes, projects without a GitHub remote (Phase 2 UI hidden).

## Resolved decisions (from roadmap)

- GitHub is store, identity and review surface; writers never touch Cloudflare.
- Preview + prod deploys run in GitHub Actions with admin-set repo secrets.
- One branch per change, `content/<slug>`.

## Decisions confirmed at approval

- Slice 0 adds a PR workflow (cargo test, web build + vitest, e2e) and a Playwright e2e harness:
  real daemon + web app, a local bare repo as the remote, a stubbed GitHub API.
- Projects with a GitHub remote can't be edited on `main`; the first edit prompts "Start a change".
- Branches are `content/<slug>`, with a numeric suffix if the name is taken.
- Get latest merges `main` into change branches (no rebase, no force-push).
- Conflict resolution: per file, mine / theirs / both.
- Preview URL is read from GitHub deployments and also posted as a PR comment; the template
  workflow change ships in this phase.
- In-app review actions: "Ready for review" and "Clean up" after merge. Merging happens on GitHub.
  Branch protection: docs plus a one-time hint.
- Open: uncommitted element-picker work on `docs/readme-dev-mode` must land or be stashed before
  slices branch from `main`.
