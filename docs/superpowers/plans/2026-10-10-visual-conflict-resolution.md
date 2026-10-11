# Visual Conflict Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a writer resolve Get latest conflicts with per-file choices and a rendered comparison of their version, latest main, and the proposed result.

**Architecture:** Hold conflicting merges as immutable, short-lived sessions; derive candidate trees without changing the active checkout. Serve isolated previews of each commit and candidate result, then atomically advance the branch only when its recorded tip still matches. The conflict screen always retains a text comparison fallback.

**Tech Stack:** Rust/Axum/Tokio, Git 2.38+, Vue 3/Pinia, Astro/Vite preview, Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-10-10-visual-conflict-resolution-design.md`

## Global Constraints

- Keep Get latest's existing clean-merge behavior and its Git 2.38+ minimum.
- Do not move a conflicting branch or its working tree until Finish; Abandon leaves both unchanged.
- A choice covers one conflicted file. Keep clean merged edits and combine only conflict regions for **Keep both**.
- A confirmed invalid result disables Finish; preview infrastructure failure still permits text-based resolution with a clear warning.
- Resolve only the recorded incoming main commit; reject Finish when the branch tip changes.
- Use temporary paths outside projects and remove preview processes/worktrees on Finish, Abandon, expiry, and daemon shutdown.

## Review Focus

1. A branch changes after the writer opens a conflict: Finish must reject without overwriting it (Task 1 test).
2. One file has multiple conflict regions plus clean edits: Keep both must preserve the clean edits and merge each region once (Task 1 test).
3. A binary conflict: Keep both must be unavailable; choosing either side must preserve exact bytes (Task 1 test).
4. A stylesheet or shared component affects several routes: the UI must show the whole-page comparison and let the writer change routes (Task 4 test).
5. A preview server fails to start: the text comparison and choices must remain usable, with an explicit warning (Task 3 test).

---

## File map

- `src/changes/conflicts.rs`: conflict discovery, candidate trees, choices, Finish and Abandon. Keep the existing branch operations in `src/changes.rs`.
- `src/changes/conflict_sessions.rs`: in-memory session registry, expiry, and serialization for each project/branch.
- `src/http/changes.rs`, `src/http/mod.rs`, `src/state.rs`: conflict API and shared registry.
- `src/conflict_preview.rs`, `src/preview.rs`: temporary worktrees, snapshot dev servers, and scoped proxy routes.
- `web/src/modules/changes/{store.ts,ConflictView.vue,ConflictFile.vue}`: conflict navigation, choices, result state, and fallback.
- `web/src/modules/preview/conflictHighlight.ts`: stable section matching and overlay in snapshot frames.
- `web/src/core/{api.ts,types.ts}`: typed client and session/result contracts.
- `tests/changes_conflicts.rs`, `tests/conflict_preview.rs`, colocated web tests, `e2e/specs/conflicts.spec.ts`: behavior at each boundary.

### Task 1: Immutable conflict sessions and candidate merge trees

**Files:** Create `src/changes/conflicts.rs`, `src/changes/conflict_sessions.rs`, `tests/changes_conflicts.rs`; modify `src/changes.rs`, `src/state.rs`, `src/lib.rs`.

**Interfaces:** `ConflictSession { id, project_id, branch, tip, incoming, base, files, expires_at }`; `ConflictFile { path, binary, ours_oid, theirs_oid, base_oid }`; `Choice = Yours | Latest | Both`; `ConflictSessions::start(dir, project_id, branch, incoming) -> Result<ConflictSession, ApiError>`; `choose(id, path, choice) -> Result<Candidate, ApiError>`; `finish(id, author) -> Result<String, ApiError>`; `abandon(id) -> Result<(), ApiError>`. `Candidate` carries a tree OID and validation state for Task 3. Session IDs are opaque and bound to the project. Extend `Update` with `conflict_sessions: HashMap<branch, session_id>` while preserving its existing `conflicts: Vec<Change>` for the current UI.

- [ ] **Step 1: Write failing Rust integration tests.** Cover exact source/base blobs, multiple conflict regions plus clean changes, binary yours/latest and rejected both, unchanged branch/checkout before Finish and after Abandon, two-parent merge with user identity, stale tip rejection, and two concurrent sessions for different branches.
- [ ] **Step 2: Run** `cargo test --test changes_conflicts` **and confirm the new tests fail.**
- [ ] **Step 3: Implement the conflict model and session registry.** Preserve `merge-tree --write-tree`'s conflicted result/stages; construct candidate trees with a temporary `GIT_INDEX_FILE`, never the project's index. For Both, combine each conflict region in the stated yours-then-latest order. Lock ref updates against checkpoints and other branch operations. Expire idle sessions and remove their temporary data.
- [ ] **Step 4: Run** `cargo test --test changes_conflicts` **and confirm all pass.**
- [ ] **Step 5: Commit** only Task 1 files with `feat(changes): prepare safe conflict resolutions`.

### Task 2: Conflict API and usable text resolution screen

**Files:** Modify `src/http/changes.rs`, `src/http/mod.rs`, `web/src/core/api.ts`, `web/src/core/types.ts`, `web/src/modules/changes/store.ts`, `web/src/modules/changes/GetLatestButton.vue`; create `web/src/modules/changes/ConflictView.vue`, `ConflictFile.vue`, their tests, and `e2e/specs/conflicts.spec.ts`.

**Interfaces:** `GET /api/changes/conflicts?project=` lists active sessions; `GET /api/changes/conflicts/{id}?project=` returns files and choices; `POST /api/changes/conflicts/{id}/choice` accepts `{project,path,choice}` and returns candidate state; `/finish` and `/abandon` accept `{project}`. Add `ConflictSession`, `ConflictFile`, `ConflictCandidate`, and `ChangesUpdate.conflictSessions: Record<string,string>` to web types. The store exposes `loadConflicts`, `chooseConflict`, `finishConflict`, `abandonConflict`; Get latest opens a session ID from `conflictSessions`, and reload recovers it through the list endpoint.

- [ ] **Step 1: Write failing API, component, and e2e tests.** Use a bare remote with the same-line edit on a content branch and main. Assert file list, text difference, choices, Finish disabled until every file is chosen, successful merge after a choice, Abandon keeping the original SHA, invalid path/session rejection, and session recovery after reloading the page.
- [ ] **Step 2: Run** `cargo test --test changes_conflicts`, `pnpm -C web test`, and `just e2e` **for the new conflict spec; confirm the new cases fail.**
- [ ] **Step 3: Implement typed routes and the text-first screen.** Reuse `checkpoints/DiffView.vue` for source comparison. Show readable file labels, exact paths, explicit choice order, a confirmable Abandon, and errors inline; keep the existing Get latest success path unchanged.
- [ ] **Step 4: Re-run the Task 2 tests and confirm they pass.**
- [ ] **Step 5: Commit** only Task 2 files with `feat(web): resolve change conflicts by file`.

### Task 3: Isolated source and result previews

**Files:** Create `src/conflict_preview.rs`, `tests/conflict_preview.rs`; modify `src/preview.rs`, `src/http/mod.rs`, `src/state.rs`, `src/changes/conflicts.rs`, `web/src/core/{api.ts,types.ts}`, `web/src/modules/changes/{ConflictView.vue,ConflictFile.vue}` and their tests.

**Interfaces:** `ConflictPreviewManager::prepare(project_dir, session_id, source: Yours | Latest | Result, revision) -> Result<PreviewState, ApiError>`; `PreviewState = Ready { url } | Unavailable { reason } | Invalid { errors }`. Proxy URLs live under `/preview/{project}/conflicts/{session}/{source}/...`, scoped to that session and authenticated like the existing preview. `GET /api/changes/conflicts/{id}/previews?project=` returns the three states and rebuilds Result when the candidate tree changes.

- [ ] **Step 1: Write failing tests.** Verify three independent revisions render distinct text without changing the project checkout; asset requests route to the matching server; unrelated projects/sessions cannot read previews; repeated choices replace only the Result preview; abandoned/expired sessions stop processes and remove worktrees. A server startup failure returns `Unavailable` and leaves text choices usable; a confirmed syntax/build error returns `Invalid` and blocks Finish.
- [ ] **Step 2: Run** `cargo test --test conflict_preview` **and the new web tests; confirm failure.**
- [ ] **Step 3: Implement preview lifecycle and proxy isolation.** Create detached temporary worktrees, start bounded dev servers using the project's launch convention, and isolate root-absolute Vite assets: the current single `wb_vite` cookie cannot identify three simultaneous frames. Prefer snapshot-scoped URL rewriting or request context over a shared cookie; disable HMR if snapshot routing cannot keep it scoped. Clean up on every session exit path.
- [ ] **Step 4: Render Your version, Latest from main, and Result at equal widths.** Provide desktop/mobile toggle, same route, loading/error states, and a visible text fallback. Require all file choices for Finish; block on confirmed invalid Result only.
- [ ] **Step 5: Run** `cargo test --test conflict_preview`, `pnpm -C web test`, and the conflict e2e spec **and confirm pass.**
- [ ] **Step 6: Commit** only Task 3 files with `feat(preview): compare isolated conflict versions`.

### Task 4: Section highlighting and full-page fallback

**Files:** Create `web/src/modules/preview/conflictHighlight.ts` and its tests; modify `web/src/modules/changes/{ConflictView.vue,ConflictFile.vue}` and `e2e/specs/conflicts.spec.ts`; update `templates/astro-marketing/src/components/sections/` only if stable section IDs are not already rendered.

**Interfaces:** `findConflictTarget(doc: Document, filePath: string, sectionId?: string): Element | null`; `attachConflictHighlight(win: Window, target: Element | null): () => void`. A null target means full-page comparison with no misleading outline. Route selection remains available for shared styles/components.

- [ ] **Step 1: Write failing unit and e2e tests.** A changed Astro section gets the same stable target in both frames; a moved, deleted, or ambiguous section yields no outline; CSS/shared-component changes compare the full page and allow route selection; desktop/mobile widths stay synchronized.
- [ ] **Step 2: Run** `pnpm -C web test` **and the conflict e2e spec; confirm the new cases fail.**
- [ ] **Step 3: Implement stable-ID matching and overlays.** Use Astro source annotations only when they identify a unique section. Never infer identity solely from DOM position. Reuse the element picker's non-interactive overlay behavior where possible.
- [ ] **Step 4: Run** `pnpm -C web test`, `cargo test`, `just e2e`, and `pnpm -C web build` **and confirm pass.**
- [ ] **Step 5: Commit** only Task 4 files with `feat(preview): highlight changed sections`.
