# Agent Conflict Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When **Sync with live site** collides with newer edits, the project's coding agent combines both versions headless in a temporary worktree, a dialog explains progress in plain language and asks the writer only when needed, and the draft is untouched on any failure.

**Architecture:** A new `src/changes/auto_resolve.rs` owns in-memory jobs keyed by conflict-session id. Each job checks out the session tip in a temp worktree, runs `git merge --no-commit` there, runs the agent headless (argv from a new `providers::headless_spec`), reads a JSON result file, verifies the tree (markers, symlinks, `npm run build`), then commits and advances the branch through helpers extracted from `ConflictSessions::finish_inner`. Progress goes over `/ws/events` as `changes.auto_resolve`; the Vue `SyncDialog` renders it.

**Tech Stack:** Rust (axum, tokio, serde_json), Vue 3 + Pinia + vitest/@vue/test-utils, Playwright.

**Spec:** `docs/superpowers/specs/2026-10-15-agent-conflict-resolution-design.md`

## Global Constraints

- Writer-facing copy never says "conflict", "merge", "branch" or "main"; say "the live site" and "your draft".
- Sync is user-initiated only (no background sync).
- One auto-resolve job per project at a time; jobs are in memory only.
- Agent round timeout 10 minutes; build timeout 5 minutes; at most 3 resolving rounds.
- `questions` has 1-5 entries; each `options` has 2-3 entries.
- The branch moves only if its tip still equals the session's recorded `tip`.
- Error sentences (exact):
  - `Unavailable` → "Your AI assistant isn't available."
  - `BuildFailed` → "The site didn't build after combining."
  - `AgentFailed` → "Your AI assistant couldn't finish combining the changes."
  - `TimedOut` → "This took too long."
  - `DraftMoved` → "Your draft changed while combining. Please sync again."
  - `Cancelled` → "You stopped combining."
- **Deviation from spec (decided during planning):** the result file is written to
  `<job dir>/result.json` outside the worktree (path passed in the prompt and in env
  `ORSO_RESOLVE_RESULT`), not `.orso/resolve-result.json` inside it, because projects
  have no git-excluded `.orso/` and the file must never enter the merge tree. The provider id
  is sent by the frontend on start (providers are chosen per browser, not stored per project).

## Review Focus

- The agent commits, switches branch, or aborts the merge in the worktree → job fails with `AgentFailed`; branch untouched. (Task 3 test `verify_rejects_agent_commit`.)
- The draft gets a new checkpoint while the agent works → `DraftMoved`; the new checkpoint survives. (Task 4 test `draft_moved_keeps_new_commit`.)
- The writer reloads the page mid-job → dialog reopens with the live state, not the old resolver. (Task 6 test `loadConflicts_opens_sync_dialog_for_existing_session`.)
- No provider selected in the browser (fresh install) → dialog shows "Your AI assistant isn't available." with Try again. (Task 6 test `startSync_without_provider_fails_plainly`.)
- The `node_modules` symlink created for the build must not enter the merge commit. (Task 4 test `merge_commit_excludes_node_modules_link`.)

---

### Task 1: Extract merge-commit and branch-advance helpers

**Files:**
- Modify: `src/changes/conflicts.rs` (add helpers near `checked`, ~L87)
- Modify: `src/changes/conflict_sessions.rs:184-292` (`finish_inner` calls the helpers)
- Test: `tests/changes_conflicts.rs`

**Interfaces:**
- Produces:
  - `pub(crate) async fn merge_commit(dir: &Path, tree: &str, tip: &str, incoming: &str, author: &Identity, message: &str) -> Result<String, ApiError>` — `commit-tree` with `-p tip -p incoming`, author/committer env from `author` (move the env-building code out of `finish_inner`).
  - `pub(crate) async fn advance_branch(dir: &Path, branch: &str, tip: &str, commit: &str) -> Result<(), ApiError>` — the existing tip CAS (`rev-parse --verify refs/heads/{branch}` must equal `tip`, else `ApiError::conflict("This change moved while resolving…")`), then the existing checked-out (`status` clean + `merge --ff-only`) or not-checked-out (`update-ref refs/heads/{branch} <commit> <tip>`) path. Callers hold `git::lock_for(dir)`.

- [ ] **Step 1: Write the failing test** `advance_branch_refuses_moved_tip` in `tests/changes_conflicts.rs`: use `fixture(...)`, record tip, commit another file on the branch, call `advance_branch(dir, branch, old_tip, some_commit)`; assert `Err` with status 409 and that `refs/heads/<branch>` still equals the new commit. (Expose the helpers for integration tests the same way the crate already exposes `conflicts::*` used by `tests/`; if it does not, make them `pub` and add a `#[doc(hidden)]`.)
- [ ] **Step 2:** `cargo test --test changes_conflicts advance_branch_refuses_moved_tip` → FAIL (not defined).
- [ ] **Step 3:** Implement both helpers by moving code out of `finish_inner`; `finish_inner` calls them with message `"Get latest from main"`.
- [ ] **Step 4:** `cargo test --test changes_conflicts` → all PASS (existing finish tests prove no behaviour change).
- [ ] **Step 5:** `git commit -am "refactor(changes): share merge commit and branch advance"`

---

### Task 2: Headless provider launch

**Files:**
- Modify: `src/providers.rs` (add after `launch_spec`, ~L300)
- Test: inline `mod tests` in `src/providers.rs`

**Interfaces:**
- Consumes: `Provider`, `LaunchSpec`, `GIT_GUIDE`, opencode dir logic (`state_dir.join(project_id).join("opencode")`, `write_opencode_config`).
- Produces: `pub fn headless_spec(provider: &Provider, prompt: &str, state_dir: &Path, project_id: &str) -> LaunchSpec` (always auto-approve; no hooks, no session flags).

- [ ] **Step 1: Verify real CLI flags** for every installed CLI, in a scratch git repo, each with a "write `{\"ok\":true}` to $ORSO_RESOLVE_RESULT" prompt:
  - `claude -p "<prompt>" --output-format stream-json --verbose --dangerously-skip-permissions`
  - `codex exec --json --dangerously-bypass-approvals-and-sandbox "<prompt>"`
  - `cursor-agent -p --force --output-format stream-json "<prompt>"`
  - `opencode run "<prompt>"` (with `OPENCODE_CONFIG_DIR` set as for terminals)

  Record the verified versions and any flag corrections in the module doc comment, next to the existing "Hook mechanics (verified …)" notes. Use the corrected flags below.
- [ ] **Step 2: Write failing tests** `headless_claude`, `headless_codex`, `headless_cursor`, `headless_opencode_sets_config_dir`, `headless_unknown_appends_prompt`. Assert the exact argv, e.g. for a base `["claude"]`:
  ```rust
  assert_eq!(spec.argv, ["claude","-p",PROMPT,"--output-format","stream-json","--verbose","--dangerously-skip-permissions","--append-system-prompt",GIT_GUIDE]);
  ```
  Codex: `["codex","exec","--json","--dangerously-bypass-approvals-and-sandbox","-c","developer_instructions=<json GIT_GUIDE>",PROMPT]`. Cursor: `["cursor-agent","-p","--force","--output-format","stream-json",PROMPT]`. OpenCode: `["opencode","run",PROMPT]` with env `OPENCODE_CONFIG_DIR`. Unknown `["sh","fake.sh","resolved"]` → base plus `PROMPT`.
- [ ] **Step 3:** `cargo test --lib providers::tests::headless` → FAIL.
- [ ] **Step 4:** Implement `headless_spec`.
- [ ] **Step 5:** `cargo test --lib providers` → PASS.
- [ ] **Step 6:** `git commit -am "feat(providers): headless launch spec for agent sync"`

---

### Task 3: Result parsing, prompt, and verification

**Files:**
- Create: `src/changes/auto_resolve.rs` (register `pub mod auto_resolve;` in `src/changes.rs` beside `conflict_sessions`)
- Test: inline `mod tests` in `src/changes/auto_resolve.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct Question { pub id: String, pub text: String, pub options: Vec<String> }
  #[derive(Debug, PartialEq)]
  pub enum ResolveResult { Resolved { summary: String }, NeedsInput { questions: Vec<Question> } }
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub enum Failure { Unavailable, BuildFailed, AgentFailed, TimedOut, DraftMoved, Cancelled }
  impl Failure { pub fn message(self) -> &'static str }   // exact sentences from Global Constraints
  pub const MAX_ROUNDS: u32 = 3;
  pub fn parse_result(bytes: &[u8]) -> Option<ResolveResult>;
  pub fn prompt(result_path: &Path, answers: &[(Question, String)]) -> String;
  pub async fn stage_and_verify(worktree: &Path, tip: &str, incoming: &str) -> Result<(), Failure>;
  ```
- `parse_result` accepts `{"status":"resolved","summary":<non-empty string>}` or `{"status":"needs_input","questions":[…]}` with 1-5 questions, unique non-empty ids, non-empty text, 2-3 options; anything else → `None`.
- `prompt` contains, in order: the fixed instruction from the spec ("Fixed prompt (summary)"), the absolute result path and both JSON shapes, and — only when `answers` is non-empty — a line `Answers from the writer:` followed by `- <question text>: <answer>` per answer.
- `stage_and_verify`, in this order:
  1. `HEAD` must equal `tip` and `MERGE_HEAD` must equal `incoming`, else `AgentFailed`.
  2. Run `git add -A -- . ':!node_modules'`.
  3. `git grep --cached -l -E '^(<<<<<<< |=======$|>>>>>>> )'` must find nothing, else `AgentFailed`.
  4. `git diff --cached --raw <tip>` must show no new mode `120000` entries, else `AgentFailed`.

  Use `conflicts::git`/`text` with the worktree as `dir`.

- [ ] **Step 1: Write failing tests:**
  - `parse_resolved`, `parse_needs_input`, `parse_rejects_missing_summary`, `parse_rejects_zero_or_six_questions`, `parse_rejects_one_or_four_options`, `parse_rejects_garbage`;
  - `prompt_without_answers_has_no_answer_block`, `prompt_lists_answers`;
  - `failure_messages_exact` (asserts all six sentences);
  - `verify_rejects_markers`, `verify_rejects_agent_commit`, `verify_rejects_new_symlink`, `verify_accepts_clean_merge`.

  Each verify test builds a temp repo with a conflicting merge (`git merge --no-commit` leaving markers), then edits it into the target state.
- [ ] **Step 2:** `cargo test --lib changes::auto_resolve` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test --lib changes::auto_resolve` → PASS.
- [ ] **Step 5:** `git commit -am "feat(changes): auto-resolve result parsing and verification"`

---

### Task 4: Auto-resolve job runner

**Files:**
- Modify: `src/changes/auto_resolve.rs`
- Modify: `src/conflict_preview.rs:268` — extract `pub(crate) fn link_node_modules(repo: &Path, dir: &Path) -> std::io::Result<()>` and reuse it
- Modify: `src/state.rs` — add `pub auto_resolve: Arc<AutoResolveJobs>` (built with `Timeouts::default()` in every constructor)
- Create: `tests/fixtures/fake_resolver.sh`
- Test: `tests/auto_resolve.rs`

**Interfaces:**
- Consumes: Task 1 helpers, Task 2 `headless_spec`, Task 3 items, `ConflictSessions::{get, abandon}`, `ConflictPreviewManager::remove_session`, `EventBus::emit`, `git::lock_for`.
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Serialize)] #[serde(rename_all = "snake_case")]
  pub enum JobState { Preparing, Resolving, NeedsInput, Verifying, Done, Failed }
  #[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
  pub struct JobStatus { pub session_id: String, pub state: JobState, pub stage: String,
      pub questions: Vec<Question>, pub summary: Option<String>, pub error: Option<String> }
  pub struct Timeouts { pub agent: Duration, pub build: Duration }   // Default: 600 s / 300 s
  pub struct JobContext { pub project_id: String, pub project_dir: PathBuf, pub session_id: String,
      pub provider: Provider, pub author: Identity, pub state_dir: PathBuf, pub events: EventBus,
      pub sessions: Arc<ConflictSessions>, pub previews: Arc<ConflictPreviewManager> }
  impl AutoResolveJobs {
      pub fn new(timeouts: Timeouts) -> Self;
      pub async fn start(&self, ctx: JobContext) -> Result<(), ApiError>;      // 409 if this project has a non-terminal job
      pub async fn status(&self, session_id: &str) -> Option<JobStatus>;
      pub async fn answer(&self, session_id: &str, answers: BTreeMap<String, String>) -> Result<(), ApiError>; // 409 unless NeedsInput; badreq if any question id unanswered
      pub async fn cancel(&self, session_id: &str) -> Result<(), ApiError>;
      pub async fn is_running(&self, session_id: &str) -> bool;                 // non-terminal state
  }
  ```
- Every state change emits `events.emit("changes.auto_resolve", json!({ "projectId": …, ..JobStatus }))`.
- Stage strings (exact):
  - Preparing: "Getting ready…"
  - Resolving: "Combining " followed by the conflict file labels joined with ", " and then "…" (use each `ConflictFile`'s label, falling back to its path)
  - Verifying: "Checking the site still builds…"
  - NeedsInput: "Waiting for your answer"
  - Done: "Done"
  - Failed: ""
- Runner (spawned tokio task per job; job dir `std::env::temp_dir()/wb-sync-<session id>`):
  1. **Preparing.** `git worktree add --detach --quiet <job>/site <tip>` in `project_dir`, then `git merge --no-commit --no-ff <incoming>` inside the worktree. Exit 0 or 1 are both fine; anything else is `AgentFailed`.
  2. **Resolving.** Delete `result.json`, build `headless_spec(provider, prompt(...), state_dir, project_id)`, and spawn it:
     - `cwd` is the worktree; env is the spec env plus `ORSO_RESOLVE_RESULT`;
     - use `process_group(0)` and `kill_on_drop`; stdout/stderr go to `<job>/agent.log`;
     - wait up to `timeouts.agent`.

     Map outcomes: a spawn error of kind `NotFound` → `Unavailable`; timeout → `TimedOut`; missing or invalid result → `AgentFailed` (whatever the exit code); `NeedsInput` → wait for `answer`, then re-run, up to `MAX_ROUNDS` resolving rounds in total, after which it's `AgentFailed`.
  3. **Verifying.** Run `stage_and_verify`. Then `link_node_modules(project_dir, worktree)` and `npm run build` in the worktree (process group, `timeouts.build`): a non-zero exit is `BuildFailed` and a timeout is `TimedOut`.
  4. **Done.** `git write-tree` in the worktree, then `merge_commit(project_dir, tree, tip, incoming, author, "Sync with live site")`. Under `git::lock_for(project_dir)`, call `advance_branch`; its 409 maps to `DraftMoved`. Then `sessions.abandon(session_id)` and `previews.remove_session(session_id)`.
  5. **Always** run `git worktree remove --force <job>/site` and `remove_dir_all(<job>)`. Keep the conflict session on failure.
  6. `cancel` kills the process group and ends the job as `Failed` with `Cancelled`.
- `tests/fixtures/fake_resolver.sh <mode> … <prompt>` (prompt is the last arg; result path from `$ORSO_RESOLVE_RESULT`):
  - `resolved`: `git checkout --ours -- $(git diff --name-only --diff-filter=U)`, then write `{"status":"resolved","summary":"Kept your version"}`.
  - `needs-input`: if the prompt lacks `Answers from the writer:`, write one question `{"id":"q1","text":"Which headline should stay?","options":["Mine","Theirs"]}`. Otherwise write the text after `Which headline should stay?: ` as the whole content of each unmerged file, then write `resolved`.
  - `garbage`: write `not json`.
  - `commit`: resolve like `resolved`, then `git commit -qam x`.
  - `sleep`: `sleep 30`.

- [ ] **Step 1: Write failing tests** in `tests/auto_resolve.rs`. Build `AutoResolveJobs::new(Timeouts { agent: 5s, build: 30s })` and a real conflict session via `ConflictSessions::start` on a `fixture(...)` repo whose `package.json` has `"build": "true"`. Poll `status` until it reaches a terminal state.
  - `resolved_advances_branch`: Done; the branch tip has two parents `[tip, incoming]`; the session is gone.
  - `needs_input_then_answer`: NeedsInput with q1, then `answer({"q1":"Theirs"})`, then Done; the branch file content is `"Theirs\n"`.
  - `garbage_fails_agent`: Failed with error "Your AI assistant couldn't finish combining the changes."; the branch is unchanged; the session still exists.
  - `missing_cli_is_unavailable`: provider argv `["/nonexistent/cli"]` → "Your AI assistant isn't available."
  - `build_failure`: `"build": "false"` → "The site didn't build after combining."
  - `agent_timeout`: `sleep` mode with agent 1 s → "This took too long."
  - `cancel_stops`: `sleep` mode, then `cancel`, then "You stopped combining."; no `wb-sync-<id>` dir is left.
  - `draft_moved_keeps_new_commit`: `needs-input` mode. While it's in NeedsInput, commit a new file on the branch, then answer → "Your draft changed while combining. Please sync again."; the branch tip is the new commit.
  - `merge_commit_excludes_node_modules_link`: create `node_modules/` in the project; after Done, `git ls-tree -r <tip>` has no `node_modules`.
  - `second_job_same_project_409`.
- [ ] **Step 2:** `cargo test --test auto_resolve` → FAIL.
- [ ] **Step 3:** Implement the runner, the state field, `link_node_modules` extraction and the fixture script (`chmod +x`).
- [ ] **Step 4:** `cargo test --test auto_resolve && cargo test --test conflict_preview` → PASS.
- [ ] **Step 5:** `git add -A src tests && git commit -m "feat(changes): headless agent auto-resolve jobs"`

---

### Task 5: HTTP routes

**Files:**
- Modify: `src/http/changes.rs` (handlers beside `finish_conflict` L482), `src/http/mod.rs:101-119` (routes)
- Test: `tests/auto_resolve.rs` (HTTP section, reuse `call` helper pattern from `tests/changes_conflicts.rs:320`)

**Interfaces:**
- Consumes: `state.auto_resolve`, `project_session` (L346), `project_dir`, `state.github.identity()`, `providers::load`, state dir = `config.projects_dir.join(projects::STATE_DIR)`.
- Produces routes:
  - `POST /api/changes/conflicts/{id}/auto` body `{project, provider}` → `{started: true}`; unknown provider → 400; job already running → 409.
  - `GET /api/changes/conflicts/{id}/auto?project=` → the `JobStatus` JSON, or `{"sessionId": id, "state": "idle", "stage": "", "questions": [], "summary": null, "error": null}` when there is no job.
  - `POST /api/changes/conflicts/{id}/auto/answers` body `{project, answers: {qid: text}}` → `{ok: true}`.
  - `POST /api/changes/conflicts/{id}/auto/cancel` body `{project}` → `{ok: true}`.
  - `choose_conflict`, `finish_conflict` and `abandon_conflict` return `ApiError::conflict("Combining is in progress.")` while `is_running(id)`.

- [ ] **Step 1: Write failing tests:**
  - `http_auto_resolved_flow`: run update with a conflict, `POST …/auto`, poll `GET …/auto` until `done`.
  - `http_auto_unknown_provider_400`.
  - `http_get_auto_idle`.
  - `http_finish_blocked_while_running` (`sleep` mode → finish returns 409).

  Build `agents.json` with `{"fake-resolver":["sh","<fixture>","resolved"]}` and pass it as the router's `agents_config`.
- [ ] **Step 2:** `cargo test --test auto_resolve http_` → FAIL.
- [ ] **Step 3:** Implement the handlers and routes.
- [ ] **Step 4:** `cargo test` → PASS.
- [ ] **Step 5:** `git commit -am "feat(http): auto-resolve endpoints"`

---

### Task 6: Frontend API, event and store

**Files:**
- Modify: `web/src/core/types.ts` (~L110 and the `DaemonEvent` union ~L276), `web/src/core/api.ts` (~L276), `web/src/core/socket.ts:19` (`SHAPES`)
- Modify: `web/src/modules/changes/store.ts` (`applyUpdate` L193, `loadConflicts` L226)
- Modify: `web/src/app/ProjectPage.vue` (~L67, where `changes.updated` is subscribed)
- Test: `web/src/modules/changes/store.test.ts`

**Interfaces:**
- Produces (TS):
  ```ts
  export interface AutoResolveQuestion { id: string; text: string; options: string[] }
  export type AutoResolveState = "idle" | "preparing" | "resolving" | "needs_input" | "verifying" | "done" | "failed";
  export interface AutoResolveStatus { sessionId: string; state: AutoResolveState; stage: string;
    questions: AutoResolveQuestion[]; summary: string | null; error: string | null }
  // DaemonEvent: | ({ type: "changes.auto_resolve"; projectId: string } & AutoResolveStatus)
  // SHAPES: "changes.auto_resolve": { projectId: "string", sessionId: "string", state: "string" }
  api.startAutoResolve(project: string, sessionId: string, provider: string): Promise<{ started: true }>
  api.autoResolveStatus(project: string, sessionId: string): Promise<AutoResolveStatus>
  api.answerAutoResolve(project: string, sessionId: string, answers: Record<string, string>): Promise<{ ok: true }>
  api.cancelAutoResolve(project: string, sessionId: string): Promise<{ ok: true }>
  // changes store:
  syncSessionId: Ref<string | null>; syncStatus: Ref<AutoResolveStatus | null>
  startSync(id: string, sessionId: string): Promise<void>
  answerSync(id: string, answers: Record<string, string>): Promise<void>
  stopSync(id: string): Promise<void>
  retrySync(id: string): Promise<void>      // = startSync(id, syncSessionId)
  closeSync(): void                          // clears syncSessionId/syncStatus
  openAdvanced(id: string): Promise<void>    // openConflict(id, syncSessionId) then closeSync()
  onAutoResolve(id: string, event: AutoResolveStatus): Promise<void>
  ```
- `startSync` sets `syncSessionId` and `syncStatus = {state: "preparing", …}`. It reads the provider from `useTerminalStore().provider`. If that's null, or the start call fails, it sets `syncStatus` to `failed` with error "Your AI assistant isn't available." (or the API error message).
- `applyUpdate` calls `startSync(id, sessionId)` for the first conflicting branch, instead of `openConflict`.
- `loadConflicts` no longer auto-opens `ConflictView`. If sessions exist and `selectedConflictId` is null, it sets `syncSessionId` to the first one and loads `syncStatus` from `autoResolveStatus`.
- `onAutoResolve` ignores events whose `sessionId` isn't `syncSessionId`, and otherwise stores the status. On `done` it runs `refreshTreeAndPreview()`, `load(id)`, `list(id)` and `conflictSessions` reload.
- `ProjectPage.vue` subscribes with `daemon.on("changes.auto_resolve", e => changes.onAutoResolve(projectId, e))`.

- [ ] **Step 1: Write failing tests** in `store.test.ts`:
  - `applyUpdate_with_conflict_starts_sync`: `startAutoResolve` is called with the session id and provider `"fake"`; `selectedConflictId` stays null.
  - `startSync_without_provider_fails_plainly`.
  - `onAutoResolve_done_refreshes`.
  - `onAutoResolve_ignores_other_session`.
  - `loadConflicts_opens_sync_dialog_for_existing_session`.
  - `openAdvanced_opens_conflict_view`.
- [ ] **Step 2:** `pnpm -C web test -- store.test` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `pnpm -C web test` and `pnpm -C web exec vue-tsc --noEmit` (or the repo's typecheck script) → PASS.
- [ ] **Step 5:** `git commit -am "feat(web): auto-resolve api, event and store"`

---

### Task 7: `SyncDialog.vue`

**Files:**
- Create: `web/src/modules/changes/SyncDialog.vue`, `web/src/modules/changes/SyncDialog.test.ts`
- Modify: `web/src/app/Workspace.vue:30` — mount `<SyncDialog v-if="changes.syncSessionId" :project-id>` next to the existing `ConflictView`

**Interfaces:**
- Consumes: the Task 6 store API; `@/components/ui/dialog`, `@/components/ui/questionnaire`, `@/components/ui/spinner`, `@/components/ui/button`.
- Props: `{ projectId: string }`. Clicking outside doesn't close it. Escape or **Stop** while working opens an inline confirm "Stop combining? Nothing will change." with **Stop** / **Keep going**; confirming calls `stopSync`.
- Copy (exact):

| Store state | Content | `data-testid` |
|---|---|---|
| preparing / resolving / verifying | Spinner; heading "Someone else also updated this site."; line "Combining your changes…"; `stage`; **Stop** | `sync-working` |
| needs_input | Questionnaire, one step per question; each option is a choice plus a "Something else…" input; submit label **Continue** → `answerSync` | `sync-question` |
| done | "Done. Your changes and the latest site are combined."; `summary`; **Close** | `sync-done` |
| failed / idle | "Nothing changed. Your draft is safe."; `error` (if any); **Try again** → `retrySync`; **Close**; link-style button **Advanced** → `openAdvanced` | `sync-failed` |

- [ ] **Step 1: Write failing tests** in `SyncDialog.test.ts` (pattern from `GetLatestButton.test.ts` with `createFakeDaemon`):
  - `shows_stage_while_resolving`;
  - `stop_asks_then_cancels`;
  - `question_choice_submits_answer` (choosing "Theirs" calls `answerAutoResolve` with `{q1:"Theirs"}`);
  - `something_else_submits_free_text`;
  - `done_shows_summary`;
  - `failed_shows_reason_and_retry`;
  - `advanced_opens_conflict_view`;
  - `copy_never_mentions_git_words`: render every state and assert the text doesn't match `/conflict|merge|branch|\bmain\b/i`.
- [ ] **Step 2:** `pnpm -C web test -- SyncDialog` → FAIL.
- [ ] **Step 3:** Implement, and mount it in `Workspace.vue`.
- [ ] **Step 4:** `pnpm -C web test` → PASS.
- [ ] **Step 5:** `git add web/src && git commit -m "feat(web): sync dialog for agent auto-resolve"`

---

### Task 8: Sync button, palette and wording

**Files:**
- Modify: `web/src/modules/changes/GetLatestButton.vue`, `GetLatestButton.test.ts`
- Modify: `web/src/modules/changes/agentRequests.ts` (remove `askAgentToGetLatest` and its message at L12)
- Modify: `web/src/modules/changes/getLatest.ts` (`report` toasts, L31-61)
- Modify: `web/src/app/palette/usePaletteGroups.ts:91`

**Interfaces:**
- Consumes: `getLatest(projectId)` from `getLatest.ts` (calls `store.getLatest` and `report`).
- Button: `aria-label` and `title` are "Sync with live site"; click → `getLatest(projectId)`; keep `data-testid="get-latest"` and the disabled conditions. The palette entry label is "Sync with live site" and runs the same call.
- `report` toast copy:
  - "Synced with main" → "Up to date with the live site"
  - "Syncing with main" → "Syncing with the live site"
  - "Changes updated, but main was left as is" → "Your draft is updated; the live site was left as is."
  - No toast when the dialog takes over (a conflict session was returned).

- [ ] **Step 1: Update tests:**
  - `GetLatestButton.test.ts`: click → `updateChanges` called; no agent outbox send.
  - Add `report_copy_is_plain`: no string matches `/main|merge|conflict|branch/i` for the done/syncing/diverged cases.
- [ ] **Step 2:** `pnpm -C web test -- GetLatestButton getLatest` → FAIL.
- [ ] **Step 3:** Implement and delete `askAgentToGetLatest` (fix any imports, e.g. palette).
- [ ] **Step 4:** `pnpm -C web test && pnpm -C web build` → PASS.
- [ ] **Step 5:** `git commit -am "feat(web): sync with live site button calls the daemon"`

---

### Task 9: End-to-end

**Files:**
- Modify: `e2e/fixtures/daemon.ts:197` (`startorso`: add `"fake-resolver": ["sh", <repo>/tests/fixtures/fake_resolver.sh, "needs-input"]` to `agents.json`)
- Create: `e2e/specs/sync-auto-resolve.spec.ts`
- Modify: `e2e/specs/get-latest.spec.ts` and `e2e/specs/conflicts.spec.ts` if they relied on the button sending an agent request or on `ConflictView` auto-opening. `conflicts.spec.ts` now reaches `ConflictView` via the dialog's **Advanced** link, using provider `garbage` or a missing provider.

**Interfaces:**
- Consumes: the `orso.ts` fixture (`origin, token, projectId, projectDir, bareRemote`), `pushConflictingMain` and the static build setup from `conflicts.spec.ts`.

- [ ] **Step 1: Write the spec** `sync-auto-resolve.spec.ts`:
  - `page.addInitScript(() => localStorage.setItem("orso.terminal.provider", "fake-resolver"))`;
  - create a change through the UI, edit and commit the conflicting file, then `pushConflictingMain`;
  - click `get-latest`; expect `sync-question` with "Which headline should stay?"; choose "Theirs"; **Continue**;
  - expect `sync-done`; **Close**;
  - assert `git show <branch>:<file>` in `projectDir` equals `"Theirs\n"`;
  - assert the page shows no text matching `/conflict|merge/i`.
- [ ] **Step 2:** `just e2e` → the new spec FAILs before Tasks 1-8 are merged. On this branch it should PASS; fix any older specs as described above.
- [ ] **Step 3:** `just test && just e2e` → all PASS.
- [ ] **Step 4:** `git add e2e && git commit -m "test(e2e): agent auto-resolve sync"`
