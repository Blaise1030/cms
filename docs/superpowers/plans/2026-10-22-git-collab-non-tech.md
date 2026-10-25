# Git Collab for Non-Tech Teammates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Internal writers collaborate via GitHub only — hard agent change-lock, plain switch/sync UX, agent-owned combine copy, light invite/open path, and in-app Publish with no greenlight gate.

**Architecture:** Keep local Orso + GitHub. Lock agent checkouts with a PATH `git` shim + `ORSO_ALLOW_CHECKOUT` (daemon switch sets it). Frontend confirms mid-chat draft switches and soft toasts for slow opens. Extend `GithubAuth` with PR merge; `POST …/publish` + a Publish control. SyncDialog renames Advanced. Invite is a checklist/link only (ACL stays on GitHub).

**Tech Stack:** Rust (axum, tokio), Vue 3 + Pinia + vitest, existing GitHub REST client in `src/github.rs`.

**Spec:** `docs/superpowers/specs/2026-10-22-git-collab-non-tech-design.md`

## Global Constraints

- Writer-facing copy never says branch, commit, merge, conflict, PR, checkout, clone, or main; say change/draft, live site, combine, publish, live.
- No collaboration server; GitHub is the only shared store.
- Anyone-publishes for this profile — no greenlight / CODEOWNERS gate in these tasks (see greenlight spec for the other profile).
- Only humans switch drafts via Change picker / `POST …/switch`; agents cannot.
- Sync collisions stay agent-first (`auto_resolve`); manual path is failure escape only.
- Exact copy from the spec where quoted below.

## Review Focus

- Agent runs `git checkout other-branch` → shim exits non-zero; HEAD unchanged. (Task 1)
- Daemon `switch` still works while an agent is running (sets `ORSO_ALLOW_CHECKOUT`). (Task 1)
- User switches drafts during an active chat stream → confirm before continue; no silent tool use on old draft. (Task 2)
- Lockfile-identical switch → no “Updating tools…” toast; stale deps → toast then existing reinstall. (Task 3)
- Publish when GitHub returns 405/409 → plain error, draft unchanged. (Task 5)
- Writer with no repo access on Home → “Ask an owner to add you to this site.” not a git error. (Task 7)

## File map

| Area | Files |
| --- | --- |
| Agent lock | Create `src/agent_git_shim.rs` (or `scripts/orso-git` installed beside hook exe); modify `src/chat/manager.rs`, `src/terminal/mod.rs`, `src/changes.rs` (`switch`), `src/providers.rs` (`GIT_GUIDE`) |
| Mid-chat switch | `web/src/modules/changes/store.ts`, `web/src/modules/chat/store.ts`, picker / small confirm dialog |
| Switch toasts | `src/devserver.rs` (event) and/or `web/src/modules/preview/store.ts` + `changes/store.ts` |
| Sync copy | `web/src/modules/changes/SyncDialog.vue` (+ tests) |
| Publish | `src/github.rs`, `src/http/changes.rs`, `web/src/modules/changes/*` (new Publish control near Share/Cleanup) |
| Onboarding | `web/src/modules/projects/RepoBrowser.vue` / `ProjectsGrid.vue`, light Invite affordance on project/settings |

---

### Task 1: Hard-block agent checkout

**Files:**
- Create: git shim binary or script installed next to the hook exe (same pattern as `Config::hook_exe_path`)
- Modify: `src/chat/manager.rs` (PATH + env when spawning chat agent)
- Modify: `src/terminal/mod.rs` (same for agent terminals)
- Modify: `src/changes.rs` `switch` — set `ORSO_ALLOW_CHECKOUT=1` on the git checkout invocation only
- Modify: `src/providers.rs` `GIT_GUIDE` — remove instructions to `git checkout` / start changes via git; tell agent to ask the user to create/switch a change in the app; stay on current change for follow-ups
- Test: `tests/` new or extend existing chat/terminal spawn tests; unit test shim argv parsing

**Interfaces:**
- Produces: shim behavior — if argv is a HEAD-moving checkout/switch (`checkout <branch>`, `checkout -b`, `switch`, etc.) and `ORSO_ALLOW_CHECKOUT` is unset/≠`1`, print a one-line stderr for logs (`orso: switching drafts is only allowed from the app`) and exit `1`; otherwise `exec` real `git` (resolve via `ORSO_REAL_GIT` or next PATH entry).
- Produces: spawn helpers prepend shim dir to `PATH` and set `ORSO_REAL_GIT` to the system git found at spawn time.
- Consumes: existing `changes::switch` git checkout path.

- [ ] **Step 1: Write failing tests** — shim rejects `git checkout content/other` without allow env; allows `git status` / `git add` / `git commit`; allows `git checkout content/other` with `ORSO_ALLOW_CHECKOUT=1`. Guide test: `agent_guide()` / `GIT_GUIDE` must not contain the substring ``git checkout -b``.
- [ ] **Step 2:** Run tests → FAIL.
- [ ] **Step 3:** Implement shim, PATH wiring, `switch` allow env, rewrite `GIT_GUIDE` per spec (propose new change in plain words; user creates it in the app).
- [ ] **Step 4:** Run tests → PASS.
- [ ] **Step 5:** Commit `feat(agent): block git checkout unless app switches drafts`

---

### Task 2: Confirm when user switches drafts mid-chat

**Files:**
- Modify: `web/src/modules/changes/store.ts` (`switchTo`)
- Modify: `web/src/modules/chat/store.ts` (expose `isRunning` / active stream flag if missing)
- Modify: `ChangesBranchPicker.vue` and/or a small confirm dialog component
- Test: `web/src/modules/changes/*.test.ts`

**Interfaces:**
- Consumes: chat “run active” boolean.
- Produces: before calling switch API, if a chat run is active, show confirm: exact copy `You switched drafts — this chat was on [old]. Continue on [new]?` with Continue / Cancel. Cancel aborts switch. Continue cancels/clears the active run then switches.

- [ ] **Step 1: Failing test** — `switchTo` with chat running does not hit the switch API until confirmed; cancel leaves `active` unchanged.
- [ ] **Step 2:** Run vitest → FAIL.
- [ ] **Step 3:** Implement confirm + clear-run-then-switch.
- [ ] **Step 4:** vitest → PASS.
- [ ] **Step 5:** Commit `fix(changes): confirm draft switch during active chat`

---

### Task 3: Soft toasts when opening a draft needs tools

**Files:**
- Modify: `src/devserver.rs` — emit a WS/app event when entering install-on-`DepsChanged` (reuse existing event bus pattern)
- Modify: `web/src/modules/changes/store.ts` `switchTo` — toast `Opening this draft…` at start of switch
- Modify: preview/changes listener — on deps-install event toast `Updating tools for this draft…`
- Test: frontend store test + optional Rust event unit test

**Interfaces:**
- Produces: event name e.g. `preview.deps_installing` (match existing `preview.*` naming).
- No new user-facing words beyond the two toast strings above.

- [ ] **Step 1: Failing tests** for toast triggers.
- [ ] **Step 2:** Implement minimal emit + listeners (daemon already reinstalls when `deps_stale`).
- [ ] **Step 3:** Tests PASS.
- [ ] **Step 4:** Commit `feat(preview): plain toasts when a draft needs tool updates`

---

### Task 4: SyncDialog escape copy — “I’ll choose myself”

**Files:**
- Modify: `web/src/modules/changes/SyncDialog.vue` (failed view button label)
- Modify: any test asserting `"Advanced"`

**Interfaces:**
- Button label becomes `I'll choose myself` (keep `data-testid="sync-advanced"` for stability).

- [ ] **Step 1: Failing test** asserting the new label on the failed view.
- [ ] **Step 2:** Change label; update tests.
- [ ] **Step 3:** Commit `copy(sync): rename Advanced to I'll choose myself`

---

### Task 5: In-app Publish (merge PR) — backend

**Files:**
- Modify: `src/github.rs` — add `merge_pr`
- Modify: `src/http/changes.rs` — `POST /projects/{id}/changes/{branch}/publish` (route name match existing share/ready style)
- Test: `src/github.rs` mock tests + `tests/ctl.rs` or changes HTTP tests

**Interfaces:**
- Produces: `GithubAuth::merge_pr(&self, repo: &str, head_branch: &str) -> Result<PublishResult, ApiError>` — find open PR for head (reuse `pr_ref` / `find_pr`), `PUT …/pulls/{number}/merge` with merge_method suitable for the repo (prefer `squash` only if already used elsewhere; otherwise `merge` — match whatever the codebase/docs already recommend for content PRs). Map 403/405/409 to plain `ApiError` messages: permission → ask an owner; not mergeable → sync/get latest first; otherwise “Couldn't publish…”.
- Produces: HTTP handler that requires GitHub-backed project + active `content/*` change, ensures PR exists (share/create draft if missing, same as ready path patterns), calls `merge_pr`, returns updated PR/change status. Does **not** check CODEOWNERS/approvals (anyone-publishes).
- After successful merge: same local cleanup hooks as post-merge today if any; otherwise leave cleanup to existing Clean up button (document which).

- [ ] **Step 1: Failing mock test** `merge_pr_merges_open_pull` + `merge_pr_maps_not_mergeable`.
- [ ] **Step 2:** Implement client + route.
- [ ] **Step 3:** `cargo test` for new tests → PASS.
- [ ] **Step 4:** Commit `feat(changes): publish merges the change PR on GitHub`

---

### Task 6: Publish button — frontend

**Files:**
- Create or modify: control beside `SharePreviewButton.vue` / `PrMenu.vue` (e.g. `PublishButton.vue`)
- Modify: `web/src/modules/changes/store.ts` — `publish()` API call
- Modify: shell that hosts Share/Cleanup (`ProjectPage.vue` / `Workspace.vue` — match Share placement)
- Test: component/store vitest

**Interfaces:**
- Produces: **Publish** button on the active change when GitHub-backed and not already merged; on success toast/banner `It's live.` and refresh change list / offer Clean up if not auto-cleaned.
- Failure: surface API plain message; no github.com deep link required.

- [ ] **Step 1: Failing test** — click Publish calls publish endpoint; success shows live copy.
- [ ] **Step 2:** Implement.
- [ ] **Step 3:** Tests PASS.
- [ ] **Step 4:** Commit `feat(changes): Publish control for anyone-publishes teams`

---

### Task 7: First-open + invite (friction A)

**Files:**
- Modify: `web/src/modules/projects/RepoBrowser.vue` / `ProjectsGrid.vue` / `store.ts` `cloneRepo` — toast `Opening [site]…` using project name
- Modify: after open, existing require-change path — ensure edit steers to Create a change (fix copy if it still says branch/main)
- Add: light **Invite** entry (project settings or home overflow) — short checklist: (1) add teammate on GitHub, (2) they sign in with GitHub, (3) they open this site — plus button/link to GitHub collaborators URL for the repo when known
- Access errors: map to `Ask an owner to add you to this site.`
- Test: vitest for toast + invite link construction

**Interfaces:**
- Invite does not call a new backend ACL API; GitHub remains source of truth.

- [ ] **Step 1: Failing tests** for opening toast and invite collaborators URL shape (`https://github.com/{owner}/{repo}/settings/access`).
- [ ] **Step 2:** Implement.
- [ ] **Step 3:** Tests PASS.
- [ ] **Step 4:** Commit `feat(projects): opening toast and invite checklist for teammates`

---

### Task 8: Mark spec approved + smoke acceptance

**Files:**
- Modify: `docs/superpowers/specs/2026-10-22-git-collab-non-tech-design.md` status line → approved
- Manual smoke (checklist in PR / commit message): two accounts, different posts, publish each; force a sync collision and confirm agent dialog; attempt `git checkout` inside chat and confirm rejection.

- [ ] **Step 1:** Update spec status.
- [ ] **Step 2:** Run targeted `cargo test` + `npm test` suites touched by Tasks 1–7.
- [ ] **Step 3:** Commit `docs: approve git collab non-tech spec`

---

## Spec coverage check

| Spec section | Task |
| --- | --- |
| Agent must not switch changes | 1, 2 |
| Switching changes + soft toasts | 3 (+ existing deps_stale) |
| Sync collisions agent-owned + I’ll choose myself | 4 (+ existing auto_resolve) |
| Onboarding | 7 |
| Publish without GitHub | 5, 6 |
| Error copy | 2, 5, 7 |
| Out of scope (co-edit, server, greenlight) | intentionally omitted |
`}