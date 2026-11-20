# Review Inbox Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Content leads review teammates' labelled changes inside orso (rendered preview + what changed) and give a greenlight or send them back, without touching their own checkout.

**Architecture:** The daemon lists labelled PRs from GitHub, fetches a PR head into `refs/orso/review/<n>` (objects only), diffs it against its merge-base with `main`, and builds a static preview in a temporary worktree that is deleted as soon as the build ends. Decisions are GitHub reviews pinned to the viewed head SHA. The web app gets a Review tab (`/p/:id/review/:number?`).

**Tech Stack:** Rust (axum, tokio, reqwest), git CLI, Vue 3 + Pinia + Nuxt UI, Vitest, Playwright e2e with `e2e/fixtures/githubStub.ts`.

**Spec:** `docs/superpowers/specs/2026-11-19-review-inbox-design.md`

**Prerequisite:** the `prLabels` work (`settings::pr_labels`, labelled `create_draft_pr`) is committed and merged to `main` before Task 1. Work on a branch from `main`, one PR per slice.

## Global Constraints

- On-screen words: **change, greenlight, send back, Sent back, Greenlit**. Never "PR", "pull request", "approve" in UI copy.
- Inbox = open, non-draft PRs with any label from `settings::pr_labels(&project.dir)`, author ≠ signed-in login. Label and login comparisons are case-insensitive.
- `myState` values: `"new" | "sent_back" | "greenlit"`, computed from the signed-in user's latest decisive review **whose `commit_id` equals the current head SHA**.
- Disabled reasons, verbatim: `"This is your change"`, `"You need edit access to this site's repository to greenlight"`.
- Error copy, verbatim: head moved → `"This change was updated since you opened it"` (409, reason `head_moved`); PR not open → `"This change is no longer waiting"` (404); send back with blank comment → 400.
- Empty inbox copy: `"Nothing waiting for your greenlight."`
- Worktrees exist only during a build: removed after success, failure and timeout. Build timeout 90 s.
- Review preview cache: key `(project, PR number)` holding one head SHA; max 3 per project (least recently viewed evicted); max age 24 h; dropped when the PR is no longer open.
- Preview URL: `/preview/{project}/reviews/{number}/{sha}/…`, mirroring the existing `/preview/{project}/conflicts/{session}/{source}/…` routing in `src/preview.rs` (the spec's `/review-preview/…` path is replaced by this, so the preview proxy's base handling is shared).
- New UI uses the repo's shadcn-vue / Nuxt UI primitives (skill `using-shadcn-components`).

## Review Focus

1. **More than 100 open PRs.** The list must paginate like `repos()` does (`PAGE_SIZE`/`MAX_PAGES`); a labelled PR on page 2 still appears. Test in Task 1.
2. **A PR from a fork or a deleted account.** Fetch by `refs/pull/<n>/head`, never by branch name; a `user: null` author shows as `"ghost"` and never matches the reviewer. Tests in Tasks 1 and 2.
3. **Label or login casing differs** (`Orso` vs `orso`, `Ada` vs `ada`). Still listed / still recognised as own. Test in Task 1.
4. **Double-click "Prepare preview" or two tabs open on one change.** One build per `(project, number, sha)`; the second call joins it. Test in Task 11.
5. **Change touches binary files (images).** "What changed" still lists the file (git prints `Binary files … differ`), no crash in `parseUnifiedDiff`. Test in Task 2 (daemon) and Task 4 (web).

---

## Slice 1 — Inbox (PR 1)

### Task 1: GitHub inbox and per-PR review state

**Files:**
- Modify: `src/github.rs` (`ApiPull`, `ApiReview`, new types and methods, tests)

**Interfaces:**
- Produces:
  - `pub enum MyReviewState { New, SentBack, Greenlit }`, `#[serde(rename_all = "snake_case")]`.
  - `pub struct ReviewItem { pub number: u64, pub title: String, pub author: String, pub url: String, pub updated_at: String, pub head_sha: String, pub checks: ChecksState, pub my_state: MyReviewState }`, `Serialize` camelCase (`headSha` **is** serialized).
  - `pub struct ReviewDetail { #[serde(flatten)] pub item: ReviewItem, pub description: Option<String>, pub open: bool, pub draft: bool }`.
  - `fn my_review_state(reviews: &[ApiReview], me: &str, head_sha: &str) -> MyReviewState` (pure).
  - `pub async fn review_inbox(&self, repo: &str, labels: &[String], me: &str) -> Result<Vec<ReviewItem>, ApiError>`.
  - `pub async fn review_detail(&self, repo: &str, number: u64, me: &str) -> Result<ReviewDetail, ApiError>`.
  - `pub async fn can_push(&self, repo: &str) -> Result<bool, ApiError>` (`GET /repos/{repo}`, `permissions.push`, false when absent).
  - `pub async fn signed_in_login(&self) -> Result<String, ApiError>` (from `status().user`, else `not_signed_in()`).

- [ ] **Step 1: Write failing tests** (in `github::tests`, using `mock_api` + `write_session` like `create_draft_pr_sends_title_body_and_draft`)
  - `my_review_state_uses_latest_review_on_current_head`: reviews by `ada` = [APPROVED on `aaa`, CHANGES_REQUESTED on `bbb`]; head `bbb` → `SentBack`; head `ccc` → `New`; reviews by others only → `New`; `Ada` vs `ada` → matches; a COMMENTED review after APPROVED on the same head → still `Greenlit`.
  - `review_inbox_filters_label_draft_and_author`: stub `GET /repos/acme/site/pulls` returns 4 PRs: labelled `Orso` by `bob` (kept), labelled `orso` draft (dropped), unlabelled (dropped), labelled by `ADA` (dropped for `me = "ada"`); a PR with `"user": null` and the label → kept with `author == "ghost"`. Labels passed: `["orso"]`.
  - `review_inbox_paginates`: page 1 has 100 unlabelled PRs and a `Link: <…page=2>; rel="next"` header, page 2 has one labelled PR → it is returned.
  - `can_push_reads_permissions`: `{"permissions":{"push":true}}` → true; no `permissions` → false.
- [ ] **Step 2: Run** `cargo test --lib github::tests::review_ github::tests::my_review_state github::tests::can_push` → FAIL (not defined).
- [ ] **Step 3: Implement.** Add to `ApiPull`: `#[serde(default)] labels: Vec<ApiLabel{name}>`, `user: Option<ApiUser>`, `updated_at: String`, `body: Option<String>`. Add to `ApiReview`: `#[serde(default)] commit_id: String`, `#[serde(default)] body: Option<String>`. List with `GET /repos/{repo}/pulls?state=open&per_page=100` through the same pagination helper `repos()` uses. Per kept PR: `GET …/pulls/{n}/reviews` and the existing check-run aggregation `pr_status` uses for `checks`. `review_detail`: `GET /repos/{repo}/pulls/{n}` (`open` = state `open` and not merged).
- [ ] **Step 4: Run** the same tests → PASS; `cargo clippy --all-targets` clean.
- [ ] **Step 5: Commit** `feat(github): list labelled changes awaiting the signed-in reviewer`

### Task 2: Fetch a change's head and diff it against main

**Files:**
- Create: `src/review.rs` (module doc: reviewing teammates' changes without touching the checkout); register in `src/lib.rs`
- Modify: `src/git.rs` (add `range_diff`)

**Interfaces:**
- Produces:
  - `pub async fn fetch_head(dir: &Path, number: u64, env: &[(String, String)]) -> Result<String, ApiError>`: one `git fetch origin +refs/pull/{n}/head:refs/orso/review/{n} +refs/heads/main:refs/remotes/origin/main`; returns the full head SHA. Takes the project git lock (`crate::git::lock_for`). Fetch failure → `ApiError::enoent("This change is no longer waiting")`.
  - `pub async fn diff(dir: &Path, head_sha: &str) -> Result<String, ApiError>`: merge-base of `origin/main` and `head_sha`, then `git::range_diff`, scoped with `settings::site_prefix(dir)`.
  - `pub async fn prune_refs(dir: &Path, open: &[u64])`: deletes `refs/orso/review/<n>` for `n` not in `open`; errors ignored.
  - `git::range_diff(dir: &Path, base: &str, head: &str, scope: &str) -> Result<String, ApiError>`: `git diff --no-color --no-ext-diff --no-textconv base head [-- scope]`.

- [ ] **Step 1: Write failing tests** in `src/review.rs` (temp repo + bare `origin`, following the helpers in `src/changes.rs` tests): push a commit to `refs/pull/7/head` on the bare remote that edits `site/a.md` and adds `site/logo.png` (binary).
  - `fetch_head_stores_ref_without_touching_checkout`: returns the pushed SHA; `refs/orso/review/7` exists; `git status --porcelain` and current branch unchanged; no new `refs/heads/*`.
  - `diff_is_head_against_merge_base_scoped_to_site_root`: with `orso.json` `{"root":"site"}` and a later unrelated commit on `main`, the diff contains `site/a.md` and `Binary files` for `site/logo.png`, and not the unrelated main commit.
  - `fetch_head_of_missing_pr_is_no_longer_waiting`: number 99 → error message `"This change is no longer waiting"`.
  - `prune_refs_drops_closed_reviews`: refs for 7 and 8, `open = [8]` → only 8 remains.
- [ ] **Step 2: Run** `cargo test --lib review::` → FAIL.
- [ ] **Step 3: Implement** the four functions above.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat: fetch and diff a teammate's change without a checkout`

### Task 3: Review endpoints (list, detail, diff)

**Files:**
- Create: `src/http/reviews.rs`
- Modify: `src/http/mod.rs` (routes), `src/static.rs` (`SPA_ROUTE_PREFIXES` if per-tab prefixes are listed there)

**Interfaces:**
- Consumes: Task 1 (`review_inbox`, `review_detail`, `can_push`, `signed_in_login`), Task 2 (`fetch_head`, `diff`, `prune_refs`), `settings::pr_labels`, `state.github.git_env()`.
- Produces (JSON camelCase):
  - `GET /api/projects/{id}/reviews` → `{ reviews: ReviewItem[] }`; also calls `review::prune_refs` with the open numbers.
  - `GET /api/projects/{id}/reviews/{n}` → `ReviewDetail` + `{ canDecide: bool, reason: string | null }`. `reason` = `"This is your change"` when author matches the login (case-insensitive), else `"You need edit access to this site's repository to greenlight"` when `!can_push`, else null. Not open or draft → 404 `"This change is no longer waiting"`.
  - `GET /api/projects/{id}/reviews/{n}/diff` → `text/plain` unified diff (fetches the head first).
  - No GitHub remote → 400 `"this project has no GitHub remote — open a repository first"` (same text as share).

- [ ] **Step 1: Write failing tests** in `src/http/reviews.rs` (follow the handler tests in `src/http/changes.rs` that build an `AppState` with a mocked GitHub): list returns only the labelled PR by another author; detail for own PR → `canDecide=false`, own-change reason; without push → edit-access reason; closed PR → 404 with the exact message; diff route returns `text/plain` containing the changed path.
- [ ] **Step 2: Run** `cargo test --lib http::reviews` → FAIL.
- [ ] **Step 3: Implement** handlers and register routes next to `/api/projects/{id}/deploy`.
- [ ] **Step 4: Run** → PASS; full `cargo test` green.
- [ ] **Step 5: Commit** `feat: review inbox endpoints`

### Task 4: Review tab, inbox and change view (no decisions yet)

**Files:**
- Create: `web/src/modules/review/store.ts`, `ReviewInbox.vue`, `ReviewChange.vue`, `ReviewPanel.vue` (routes inbox vs change by `number` prop), tests `ReviewInbox.test.ts`, `ReviewChange.test.ts`
- Modify: `web/src/core/types.ts`, `web/src/core/api.ts`, `web/src/router.ts` (`ProjectTab` gains `"review"`; child route `review/:number?` named `project-review`), `web/src/app/ProjectRail.vue` (tab `{ id: "review", label: "Review", icon: "i-hugeicons-task-done-01" }` with count badge), `web/src/lib/tabOrder.ts` (include the new tab), `web/src/app/palette/usePaletteGroups.ts` if it enumerates tabs

**Interfaces:**
- Consumes: Task 3 JSON.
- Produces:
  - Types `MyReviewState`, `ReviewItem`, `ReviewDetail` (`+ canDecide`, `reason`).
  - `api.reviews(project) → Promise<ReviewItem[]>`, `api.review(project, n) → Promise<ReviewDetail>`, `api.reviewDiff(project, n) → Promise<string>`.
  - `useReviewStore()` with `items`, `waitingCount` (count of `myState === "new"`), `load(projectId)`, `open(projectId, n)`, `current`, `diffText`, `error`.

- [ ] **Step 1: Write failing tests.**
  - Inbox: renders title, author, relative updated time, checks icon, and a badge `New` / `You sent back` / `You greenlit`; empty list shows `"Nothing waiting for your greenlight."`; clicking a row routes to `project-review` with `number`.
  - Rail: Review tab shows `waitingCount` badge only when > 0 (`ProjectRail.test.ts`).
  - Change view: header shows title, author, description, `View live site` link (from `api.liveUrl`/existing live URL source); "What changed" renders `ChangedFilesTree` + `DiffView` from `parseUnifiedDiff(diffText)`; a diff with only `Binary files a/site/logo.png and b/site/logo.png differ` still lists `site/logo.png`.
  - 404 from `api.review` shows `"This change is no longer waiting"` with a back-to-inbox action.
- [ ] **Step 2: Run** `pnpm -C web exec vitest run src/modules/review src/app/ProjectRail.test.ts` → FAIL.
- [ ] **Step 3: Implement.** Refresh the inbox on tab focus and every 60 s while the tab is visible. Copy per Global Constraints.
- [ ] **Step 4: Run** the same → PASS; `pnpm -C web exec vue-tsc -b` clean.
- [ ] **Step 5: Commit** `feat(web): Review tab with inbox and what changed`

---

## Slice 2 — Decisions (PR 2)

### Task 5: Submit greenlight / send back; Sent back state

**Files:**
- Modify: `src/github.rs`, `src/http/reviews.rs`, `src/http/mod.rs`

**Interfaces:**
- Produces:
  - `pub enum Decision { Greenlight, SendBack }` (`Deserialize`, snake_case: `"greenlight" | "send_back"`).
  - `pub async fn submit_review(&self, repo: &str, number: u64, head_sha: &str, decision: Decision, comment: Option<&str>) -> Result<(), ApiError>`: re-reads the PR head; different → `ApiError::conflict("This change was updated since you opened it").with_reason("head_moved")` with **no** POST; else `POST /repos/{repo}/pulls/{n}/reviews` `{ event: "APPROVE" | "REQUEST_CHANGES", body, commit_id: head_sha }`.
  - `PrState::ChangesRequested`, serialized `"changes_requested"` (explicit `#[serde(rename)]`; the enum is `lowercase`). `pr_state` returns it when any reviewer's latest decisive review is CHANGES_REQUESTED (before the approved check).
  - `PrStatus.sent_back: Option<SentBack>`, `pub struct SentBack { pub reviewer: String, pub comment: String }` from the latest CHANGES_REQUESTED review.
  - `POST /api/projects/{id}/reviews/{n}/decision` `{ decision, comment?, headSha }` → `204`; blank/whitespace `comment` with `send_back` → 400; own change / no push → 403 with the Global Constraints reason.

- [ ] **Step 1: Write failing tests:** `submit_review_pins_commit_id` (stub captures body: event, body, `commit_id`); `submit_review_refuses_moved_head` (stub head ≠ given → error reason `head_moved`, POST counter 0); `pr_state_changes_requested_beats_approval`; `pr_status_carries_send_back_comment`; handler tests for 400 blank comment and 403 own change.
- [ ] **Step 2: Run** `cargo test --lib` filtered to these → FAIL.
- [ ] **Step 3: Implement.** Grep every `match` on `PrState` (`rg "PrState::" src web/src`) and treat `ChangesRequested` like `Open` for publish gating.
- [ ] **Step 4: Run** full `cargo test` → PASS.
- [ ] **Step 5: Commit** `feat: greenlight or send back a change as a GitHub review`

### Task 6: Decision UI and the creator's Sent back view

**Files:**
- Create: `web/src/modules/review/SendBackDialog.vue`, `SendBackDialog.test.ts`
- Modify: `ReviewChange.vue` (+ test), `store.ts`, `core/api.ts`, `core/types.ts` (`PrState` gains `"changes_requested"`, `PrStatus.sentBack?: { reviewer: string; comment: string } | null`), `web/src/modules/changes/PrMenu.vue` (+ test)

**Interfaces:**
- Consumes: Task 5 route.
- Produces: `api.decideReview(project, n, { decision, comment?, headSha })`; store `decide(decision, comment?)`.

- [ ] **Step 1: Write failing tests.**
  - `Give greenlight` and `Send back` buttons; when `canDecide` is false both are disabled and the `reason` text is shown.
  - Send back dialog: submit disabled until the comment has non-whitespace text.
  - On a 409 `head_moved`, show `"This change was updated since you opened it"` and reload detail + diff; nothing else changes.
  - After success, the row's state updates to `You greenlit` / `You sent back` without a full reload.
  - `PrMenu`: state `changes_requested` shows **Sent back**, the reviewer, and the comment verbatim; `approved` shows **Greenlit**.
- [ ] **Step 2: Run** `pnpm -C web exec vitest run src/modules/review src/modules/changes` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS; `pnpm -C web exec vue-tsc -b` clean.
- [ ] **Step 5: Commit** `feat(web): give a greenlight or send a change back`

### Task 7: End-to-end review specs

**Files:**
- Create: `e2e/specs/review-inbox.spec.ts`
- Modify: `e2e/fixtures/githubStub.ts` (open PR list with `labels`, `user`, `updated_at`, `head.sha`; `GET /repos/{o}/{r}` with `permissions`; capture `POST …/pulls/{n}/reviews` as `reviewPosts()`), `e2e/fixtures/daemon.ts` (helper to push a commit to `refs/pull/<n>/head` on the bare remote), `e2e/README.md` (fixture table row)

- [ ] **Step 1: Write the specs.**
  - *Greenlight:* a labelled PR #3 by `bob` exists → Review tab badge `1` → open it → "What changed" lists the edited file → `Give greenlight` → `reviewPosts()[0]` has `event: "APPROVE"` and `commit_id` = pushed SHA → reviewer's current branch and `git status` are unchanged.
  - *Send back:* `Send back` with comment `"Please fix the title"` → post has `REQUEST_CHANGES` and that body; with the stub now serving that review on the creator's PR status, the creator's change menu shows **Sent back** and `Please fix the title`.
- [ ] **Step 2: Run** `pnpm -C web build && cargo build --release && pnpm -C e2e exec playwright test specs/review-inbox.spec.ts` (`just e2e` for the full suite) → PASS.
- [ ] **Step 3: Commit** `test(e2e): review inbox greenlight and send back`

---

## Slice 3 — Preview (PR 3)

### Task 8: Shared snapshot builder that never keeps a worktree

**Files:**
- Create: `src/snapshot_build.rs` (register in `src/lib.rs`)
- Modify: `src/conflict_preview.rs` (use it; drop its own `build_snapshot`/worktree removal; serve from the output dir)

**Interfaces:**
- Produces:
  - `pub enum BuildError { Unavailable(String), Invalid(String) }` (moved from `conflict_preview`, same classification markers).
  - `pub async fn build(repo: &Path, revision: &str, base_url: &str, out: &Path, timeout: Duration) -> Result<(), BuildError>`:
    1. `settings::load(repo)` (defaults on error) → `root`, `install_dir`.
    2. `git worktree add --detach <tmp> <revision>` where `<tmp>` is a fresh dir under `out`'s parent.
    3. Symlink `repo/<install_dir>/node_modules` → `<tmp>/<install_dir>/node_modules` (generalizes `link_node_modules`).
    4. `npm run build` in `<tmp>/<root>` with `PUBLIC_BASE=base_url`, process group, `timeout`.
    5. Check `dist/index.html`, rename `<tmp>/<root>/dist` → `out`.
    6. **Always** `git worktree remove --force <tmp>` and delete `<tmp>` (a drop guard covers early returns and timeout).
  - `pub async fn sweep(repo: &Path, base: &Path, keep: &[PathBuf])`: removes dirs under `base` not in `keep`, then `git worktree prune`.

- [ ] **Step 1: Write failing tests** (fixture repo with a `package.json` whose `build` script writes `dist/index.html`; variants that `exit 1` and `sleep 5`):
  - `build_removes_worktree_after_success` — `out/index.html` exists; `git worktree list` shows only the main worktree; `<tmp>` gone.
  - `build_removes_worktree_after_failure` — returns `Err`; worktree list clean.
  - `build_removes_worktree_after_timeout` — timeout 200 ms; `Err(Unavailable("Preview build timed out"))`; worktree list clean; process killed.
  - `build_runs_in_orso_root` — `orso.json` `{"root":"site","installDir":"site"}`, package under `site/` → succeeds.
  - `sweep_removes_orphans_and_prunes` — leftover dir and a stale worktree entry are gone; `keep` dir stays.
  - Existing `conflict_preview` tests still pass unchanged.
- [ ] **Step 2: Run** `cargo test --lib snapshot_build conflict_preview` → FAIL.
- [ ] **Step 3: Implement** and switch `conflict_preview` to `build` + output dirs.
- [ ] **Step 4: Run** full `cargo test` → PASS.
- [ ] **Step 5: Commit** `refactor: build previews in a throwaway worktree, honouring orso.json root`

### Task 9: Review preview cache with limits

**Files:**
- Create: `src/review_preview.rs`
- Modify: `src/state.rs` (`review_previews: Arc<ReviewPreviews>`), `src/main.rs` / wherever `conflict_previews.shutdown()` and startup run (call `sweep` + `shutdown`)

**Interfaces:**
- Consumes: Task 8 `build`, `sweep`, `BuildError`.
- Produces:
  - `pub enum ReviewPreview { Building, Ready { url: String }, Failed { message: String } }` (serde tag `state`, values `building|ready|failed`).
  - `ReviewPreviews::new(base: PathBuf, clock: fn() -> SystemTime)`.
  - `prepare(&self, project: &str, repo: &Path, number: u64, sha: &str) -> ReviewPreview` — joins an in-flight build for the same key+sha; a new sha replaces (and deletes) the old entry; then enforces limits.
  - `status(&self, project, number, sha) -> Option<ReviewPreview>`; `serve(&self, project, number, sha, rest) -> Result<Response, ApiError>` (reuse `conflict_preview`'s safe-route file serving; marks last viewed).
  - `prune(&self, project, open: &[u64])`; `shutdown(&self)`; limits `MAX_PER_PROJECT = 3`, `MAX_AGE = 24 h`.

- [ ] **Step 1: Write failing tests** (inject a fake builder via a `cfg(test)` hook or a trait so tests don't run npm; injected clock):
  - `prepare_twice_builds_once` (Review Focus 4).
  - `new_head_replaces_old_build` — old output dir deleted.
  - `fourth_build_evicts_least_recently_viewed`.
  - `builds_older_than_a_day_are_dropped`.
  - `prune_drops_closed_prs`.
  - `failed_build_reports_message_and_can_retry`.
- [ ] **Step 2: Run** `cargo test --lib review_preview` → FAIL.
- [ ] **Step 3: Implement**; startup runs `sweep(base, keep = [])` per project dir; inbox load (Task 3 handler) also calls `prune`.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat: cache review previews with size and age limits`

### Task 10: Preview routes and the Preview pane

**Files:**
- Modify: `src/http/reviews.rs`, `src/http/mod.rs`, `src/preview.rs` (`review_path` next to `conflict_path`), `web/src/modules/review/ReviewChange.vue` (+ test), `store.ts`, `core/api.ts`, `e2e/specs/review-inbox.spec.ts`

**Interfaces:**
- Produces:
  - `POST /api/projects/{id}/reviews/{n}/preview` `{ headSha }` → `ReviewPreview`; `GET …/preview?headSha=` → `ReviewPreview`.
  - Files served at `/preview/{project}/reviews/{n}/{sha}/…` (base URL passed to the build).
  - Web: Preview pane shows `"Preparing the preview…"` while building (poll every 2 s), the iframe when ready, and the build's last output plus **Try again** when failed; diff and decision buttons keep working in all three states.

- [ ] **Step 1: Write failing tests:** `review_path` parsing (valid, missing sha, bad number); handler returns `building` then `ready`; web states for building / ready / failed + Try again calls `prepare` again; e2e greenlight spec additionally waits for the iframe to show the fixture page's heading.
- [ ] **Step 2: Run** Rust, web and e2e tests → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test`, `pnpm -C web test`, `pnpm -C web exec vue-tsc -b`, and the Task 7 single-spec e2e command → PASS.
- [ ] **Step 5: Commit** `feat: rendered preview of a change under review`
