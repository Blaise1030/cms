# Phase 2 — Changes, preview links, review — Slice Plan

- Spec: docs/superpowers/specs/2026-10-04-phase2-changes-preview-review-design.md
- Feature branch: `features/phase2-changes-preview-review` (spec + plan live here; slice PRs chain into it; final PR → `main`)
- Base SHA: 5d3c1dee14aab5a2975f89ea2820c801f2ccc225 (`main` the feature branch was cut from)
- Tracker: GitHub Issues, `Blaise1030/cms` — parent: #21 (milestone #1)
- CI checks on PR: `.github/workflows/ci.yml` (added in Slice 0) → jobs `rust` (cargo test), `web` (pnpm build + vitest), `e2e` (Playwright)
- E2E: Playwright, specs in `e2e/specs/`, run with `just e2e`, conventions doc at `e2e/README.md`
- Conventions source: CLAUDE.md; exemplars `src/git.rs`, `src/github.rs`, `src/http/*.rs` (Axum + `ApiError` codes), `web/src/modules/<x>/` (Pinia `store.ts` + components + colocated `*.test.ts`); commits `feat(web): …` / `fix: …`

**Pre-req (spec open item):** resolved — element-picker work merged to `main` in #25.

Branching: slice N lives on `features/phase2-changes-preview-review/slice-N-<name>`, based on its unmerged dependency's slice branch, else the feature branch.

New code homes (shared by slices; created by whichever slice first needs them):
- Rust: `src/changes.rs` (branch ops: create/list/switch/merge-main/cleanup), `src/http/changes.rs` (routes under `/api/changes`), PR/deployment calls in `src/github.rs`.
- Web: `web/src/modules/changes/` (`store.ts`, `ChangesMenu.vue`, `ConflictView.vue`, tests), status pill in `web/src/modules/preview/PreviewPanel.vue`.

## Slice 0: CI runs a smoke e2e on every PR

- Issue: #26 (https://github.com/Blaise1030/cms/issues/26)
- Depends on: none
- Flow (REQUIRED): Developer opens a PR against any base branch (`main`, feature or slice branch — no `branches:` filter) → GitHub Actions runs cargo test, web build + vitest, and a Playwright smoke spec, all green.
- E2E spec (REQUIRED): `e2e/specs/smoke.spec.ts` — fixture boots daemon (`cargo run -- serve`, temp state dir, fixed token) serving the built web app, with a project cloned from a local **bare repo** remote and `api_url` pointed at a **stub GitHub server**; test opens the project page and sees the file tree + preview panel.
- Unit tests (REQUIRED): stub GitHub server responds to `/user` and device-flow endpoints with a signed-in identity; fixture helper creates bare remote with a seeded `main`.
- Layers touched: `.github/workflows/ci.yml`; `e2e/playwright.config.ts`, `e2e/fixtures/{daemon,bareRemote,githubStub}.ts`, `e2e/README.md` (conventions: fixtures, seeding/cleanup, `data-testid` selector rule, run command); `justfile` (`e2e` recipe, `test` unchanged); config hook if needed so the clone/git URL base can target the bare repo (`src/config.rs`).
- Out of scope for this slice: any Phase 2 feature.
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 1: Start a change (walking skeleton)

- Issue: #27 (https://github.com/Blaise1030/cms/issues/27)
- Depends on: 0
- Flow (REQUIRED): Writer clicks "New change", types "New post: Launch week" → `content/launch-week` is created from latest `main` and checked out; header shows "Launch week"; the next agent checkpoint commits to that branch.
- E2E spec (REQUIRED): `e2e/specs/changes-start.spec.ts` — open project → New change → enter title → header shows change; write a file via the agent/checkpoint path → bare remote untouched, local `content/launch-week` has the commit, `main` does not. Second change with the same title → `content/launch-week-2`.
- Unit tests (REQUIRED): slugify (punctuation, unicode, empty → error, prefix stripping); suffix on collision; create fetches `main` first; `GET /api/changes/active` returns branch + title; store sets active change.
- Layers touched: `src/changes.rs`, `src/http/changes.rs`, `src/http/mod.rs` routes (`POST /api/changes`, `GET /api/changes/active`); title stored in branch description (`git config branch.<b>.description`); `web/src/modules/changes/{store.ts,NewChangeDialog.vue}`; header in `web/src/app/EditorMenubar.vue`.
- Out of scope for this slice: list/switch, edit-on-main gate, push/PR.
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 2: List and switch changes

- Issue: #28 (https://github.com/Blaise1030/cms/issues/28)
- Depends on: 1
- Flow (REQUIRED): Writer opens the Changes menu → sees open change branches (title, last edited) → clicks one → working tree, file tree and preview switch to it.
- E2E spec (REQUIRED): `e2e/specs/changes-switch.spec.ts` — create two changes with different file edits → open menu, both listed newest first → switch to the first → header + file content reflect it.
- Unit tests (REQUIRED): list = local `content/*` branches not merged into `main`, sorted by last commit; switch refuses with a clear error on uncommitted changes (checkpoint first); store refreshes tree/preview on switch.
- Layers touched: `src/changes.rs` (list, switch), `GET /api/changes`, `POST /api/changes/{branch}/switch`; `web/src/modules/changes/ChangesMenu.vue`; files/preview stores reload on switch event.
- Out of scope for this slice: PR state in the list (added by Slice 5).
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 3: Editing on main prompts "Start a change"

- Issue: #29 (https://github.com/Blaise1030/cms/issues/29)
- Depends on: 1
- Flow (REQUIRED): Writer on `main` of a GitHub-remote project sends a chat prompt → sees "Start a change first" with a title field; submitting creates the change and sends the prompt on it. Local-only projects edit `main` as before.
- E2E spec (REQUIRED): `e2e/specs/changes-main-guard.spec.ts` — GitHub project on `main` → send prompt → guard dialog → enter title → prompt runs, commit on new branch, `main` untouched. Local-only project → prompt runs with no dialog.
- Unit tests (REQUIRED): server rejects agent prompt/checkpoint on `main` for GitHub projects (`ApiError` code `change_required`); allowed for non-GitHub projects; ChatPanel shows guard on that code and retries.
- Layers touched: `src/http/agent.rs`, `src/http/checkpoints.rs` guard; `web/src/modules/chat/ChatPanel.vue`, reuse `NewChangeDialog.vue`.
- Out of scope for this slice: restore on main (unchanged).
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 4: Share preview (push + draft PR)

- Issue: #30 (https://github.com/Blaise1030/cms/issues/30)
- Depends on: 1
- Flow (REQUIRED): Writer clicks "Share preview" → branch is pushed, a draft PR titled after the change is opened (or updated on later shares) → app shows the PR link. No push access / expired auth → clear error.
- E2E spec (REQUIRED): `e2e/specs/share-preview.spec.ts` — start change, commit → Share preview → bare remote has the branch, stub received `POST /repos/{o}/{r}/pulls` with `draft: true` + title → link shown; commit again → Share → remote advanced, no second PR created. Stub returns 403 → "You don't have push access" message; 401 → sign-in prompt.
- Unit tests (REQUIRED): find-existing-PR by head before create; PR title/body from change title; 401/403/422 → distinct `ApiError` codes; store state transitions.
- Layers touched: `src/github.rs` (`find_pr`, `create_draft_pr`), `src/changes.rs` (push via `git_env`), `POST /api/changes/{branch}/share`; `web/src/modules/changes/` Share button + PR link in `EditorMenubar.vue`.
- Out of scope for this slice: PR/check status display, preview URL.
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 5: PR status, checks and preview URL in the browser bar (+ template preview deploys)

- Issue: #31 (https://github.com/Blaise1030/cms/issues/31)
- Depends on: 4
- Flow (REQUIRED): Writer with a shared change sees in the preview browser bar: PR state (draft/open/approved/merged/closed), checks (pending/passing/failing) and the preview URL once deployed (opens in new tab). The template's workflow deploys PR previews and publishes that URL.
- E2E spec (REQUIRED): `e2e/specs/pr-status.spec.ts` — stub serves PR (draft), check-runs (pending), no deployment → bar shows "Draft · Checks pending", no link; stub flips to open + success + deployment status with `environment_url` → after refresh bar shows "Open · Checks passing" and link with correct `href`/`target=_blank`; approved review → "Approved". Changes menu shows PR state per change.
- Unit tests (REQUIRED): state mapping (draft, open, approved via reviews, merged, closed); check aggregation (any failing → failing, any pending → pending); preview URL from latest successful deployment for head SHA; polling only while a PR exists. Template: `actionlint` on `deploy.yml` in CI.
- Layers touched: `src/github.rs` (PR, reviews, check-runs, deployments), `GET /api/changes/{branch}/status`; `web/src/modules/preview/PreviewPanel.vue` status pill; `templates/astro-marketing/.github/workflows/deploy.yml` (`pull_request` trigger, `--branch=<head_ref>`, GitHub deployment + status with `environment_url`, sticky PR comment, `pull-requests: write`), setup docs in the workflow header + template README.
- Out of scope for this slice: Ready for review, merged clean-up.
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 6: Ready for review

- Issue: #32 (https://github.com/Blaise1030/cms/issues/32)
- Depends on: 5
- Flow (REQUIRED): Writer with a draft PR clicks "Ready for review" → PR becomes ready; bar shows "Open"; a link opens the PR on GitHub for merging.
- E2E spec (REQUIRED): `e2e/specs/ready-for-review.spec.ts` — shared draft change → Ready for review → stub received the GraphQL `markPullRequestReadyForReview` mutation → bar shows "Open", button gone, "Open on GitHub" link present.
- Unit tests (REQUIRED): mutation uses PR node id; button only for draft PRs; error surfaced on 403.
- Layers touched: `src/github.rs` (GraphQL call), `POST /api/changes/{branch}/ready`; PreviewPanel/EditorMenubar button.
- Out of scope for this slice: merging in-app (out of scope for phase).
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 7: Merged change shows as merged; Clean up

- Issue: #33 (https://github.com/Blaise1030/cms/issues/33)
- Depends on: 5
- Flow (REQUIRED): PR merged on GitHub → writer sees the change as "Merged" → clicks "Clean up" → local branch deleted, app returns to updated `main`, change gone from the list.
- E2E spec (REQUIRED): `e2e/specs/merged-cleanup.spec.ts` — shared change; merge it into the bare remote's `main` and flip stub PR to merged → bar shows "Merged" + Clean up → click → header shows `main`, local branch gone, `main` contains the change's file, menu no longer lists it.
- Unit tests (REQUIRED): cleanup refuses unmerged branches; fast-forwards `main` before deleting; deletes only local branch.
- Layers touched: `src/changes.rs` (cleanup), `POST /api/changes/{branch}/cleanup`; changes store/menu, PreviewPanel pill.
- Out of scope for this slice: deleting remote branches.
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 8: Get latest (clean update)

- Issue: #34 (https://github.com/Blaise1030/cms/issues/34)
- Depends on: 2
- Flow (REQUIRED): Writer clicks "Get latest" → `main` is pulled and each open change branch has `main` merged in → "Up to date" and the preview reflects new `main` content.
- E2E spec (REQUIRED): `e2e/specs/get-latest.spec.ts` — two local changes; push a new non-conflicting commit to bare remote `main` → Get latest → success toast; both branches contain the new file; active preview shows it; no force-push (remote branch history preserved).
- Unit tests (REQUIRED): fast-forward `main`; merge `main` into each `content/*` branch (merge commit authored by the user's identity); already-up-to-date no-op; uncommitted changes → refuse with message.
- Layers touched: `src/changes.rs` (`get_latest`), `POST /api/changes/update`; Get latest button in `ChangesMenu.vue`/`EditorMenubar.vue`.
- Out of scope for this slice: conflicts (reported as "conflicts in N changes" and left untouched until Slice 9).
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 9: Resolve conflicts (mine / theirs / both)

- Issue: #35 (https://github.com/Blaise1030/cms/issues/35)
- Depends on: 8
- Flow (REQUIRED): Get latest hits a conflict → writer sees a conflict view per file with mine / theirs / both → resolving all files completes the update with a commit authored by the user; "Abandon" restores the branch to its pre-update state.
- E2E spec (REQUIRED): `e2e/specs/conflicts.spec.ts` — change edits `about.md`; remote `main` edits same lines → Get latest → conflict view lists `about.md` → choose "both" → commit on branch contains both versions, author = signed-in user. Second run → Abandon → branch SHA equals pre-update SHA, working tree clean.
- Unit tests (REQUIRED): conflict detection lists files with ours/theirs blobs; mine/theirs/both produce expected content; finish commits merge with user identity; abandon = `merge --abort`; ConflictView renders choices and disables Finish until all resolved.
- Layers touched: `src/changes.rs` (conflict state, resolve, finish, abort), `GET/POST /api/changes/conflicts…`; `web/src/modules/changes/ConflictView.vue` (reuse `checkpoints/DiffView.vue` rendering).
- Out of scope for this slice: per-hunk resolution.
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 10: Branch protection hint

- Issue: #36 (https://github.com/Blaise1030/cms/issues/36)
- Depends on: 4
- Flow (REQUIRED): Admin shares a preview on a repo whose `main` is unprotected → sees a one-time hint "Protect main so merges need review" linking to docs and the repo's branch settings; dismissing hides it for that project.
- E2E spec (REQUIRED): `e2e/specs/branch-protection-hint.spec.ts` — stub returns 404 for `/branches/main/protection` → hint shown after share → dismiss → reload, hint gone; stub returns protection → no hint.
- Unit tests (REQUIRED): 404 → unprotected, 200 → protected, 403 (no admin) → don't show; dismissal persisted per project.
- Layers touched: `src/github.rs` (protection check), field on share/status response; hint component in `web/src/modules/changes/`; docs `docs/branch-protection.md` + README link.
- Out of scope for this slice: setting protection via API.
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Dependency graph

```
0 ─ 1 ┬ 2 ─ 8 ─ 9
      ├ 3
      └ 4 ┬ 5 ┬ 6
          │   └ 7
          └ 10
```
Parallel waves: {0} → {1} → {2, 3, 4} → {5, 8, 10} → {6, 7, 9}.
