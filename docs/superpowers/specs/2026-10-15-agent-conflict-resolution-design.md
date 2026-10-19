# Agent conflict resolution design

Status: **draft for review, 2026-10-15**. Builds on the implemented conflict sessions and visual
resolver (`2026-10-10-visual-conflict-resolution-design.md`), which become the advanced fallback.

## Goal

Writers never deal with conflicts. When syncing a draft with the live site collides with someone
else's edits, the project's coding agent combines both versions in the background. A dialog
explains what is happening in plain language and asks the writer a question only when the agent
cannot decide safely. If anything fails, the draft is left exactly as it was.

## Decisions

| Topic | Decision |
|---|---|
| Trigger | User-initiated only: the **Sync with live site** button (no background sync) |
| Where the agent runs | Headless, in a temporary git worktree; the user's terminal, checkout and preview are untouched |
| How the agent resolves | It edits a real `git merge` with conflict markers (approach A); not JSON file contents, not prompt replay |
| Who decides it is safe | The daemon verifies the result itself; the agent's claim is not trusted |
| Asking the user | Plain-language questions with 2-3 agent-prepared options plus "Something else…" |
| Failure | Draft untouched, "Nothing changed. Your draft is safe.", **Try again**, and an **Advanced** link to the existing visual resolver |
| Wording | Never "conflict", "merge", "branch" or "main"; say "the live site" and "your draft" |

## Writer flow

1. The writer clicks **Sync with live site**. With no collision, behaviour is unchanged (toast).
2. On a collision, a centred dialog opens: *"Someone else also updated this site. Combining your
   changes…"*, with a stage line such as "Combining *Pricing page*…" or "Checking the site still
   builds…".
3. **Resolved:** *"Done. Your changes and the latest site are combined."* plus the agent's
   one-line summary. The preview reloads.
4. **Needs input:** one question at a time with the agent's options and a free-text "Something
   else…". After the answers, the agent runs again and the daemon verifies again.
5. **Failed or stopped:** *"Nothing changed. Your draft is safe."* with one plain reason,
   **Try again**, **Close**, and a small **Advanced** link that opens `ConflictView`.

## Daemon

### `src/changes/auto_resolve.rs` (new)

An auto-resolve job belongs to one existing conflict session (which already records `branch`,
`tip`, `incoming` and `base`). At most one job runs per project; jobs live in memory and are lost
on daemon restart (the writer clicks Try again).

States: `preparing → resolving → (needs_input → resolving)* → verifying → done | failed`.

1. **Preparing.** `git worktree add --detach <tmp> <tip>` (same pattern as
   `conflict_preview.rs`), then `git merge --no-commit --no-ff <incoming>` inside it so conflicted
   files contain markers. Temporary directories live under the daemon state dir, never in the
   project.
2. **Resolving.** Run the project's provider headless with `cwd` = worktree, auto-approve on, and
   a fixed prompt (below). Each round has a timeout of 10 minutes. Headless argv per provider is
   built in `providers.rs` beside the interactive `LaunchSpec`:
   - Claude: `claude -p <prompt> --output-format stream-json --dangerously-skip-permissions`
   - Codex: `codex exec --json --dangerously-bypass-approvals-and-sandbox <prompt>`
   - Cursor: `cursor-agent -p --force --output-format stream-json <prompt>`
   - OpenCode: `opencode run <prompt>` with the per-project `OPENCODE_CONFIG_DIR`
   - Unknown ids: base argv followed by the prompt as the last argument.

   Exact flags are verified against the installed CLIs during planning, as `providers.rs`
   already records for the interactive launches. Raw output goes to a debug log under the state
   dir; it is never shown to the writer.
3. **Result file.** The agent must write `.orso/resolve-result.json` in the worktree
   (git-excluded):
   ```json
   {"status":"resolved","summary":"Kept your new headline and Ana's updated prices"}
   {"status":"needs_input","questions":[{"id":"q1","text":"Which headline should stay?","options":["Simple pricing","Plans for every team"]}]}
   ```
   Missing, unparsable, or schema-invalid output fails the job. `options` has 2-3 entries;
   `questions` has 1-5 entries.
4. **Needs input.** The job waits for answers, then reruns the agent in the same worktree with
   the original prompt plus `Answers from the writer:` and one line per question. After 3
   resolving rounds the job fails.
5. **Verifying.** All must pass:
   - no conflict markers (`<<<<<<<`, `=======`, `>>>>>>>` at line start) in tracked files;
   - `git diff --name-only` stays inside the worktree and touches no `.git`/`.orso` paths
     beyond the result file;
   - `npm run build` exits 0 in the worktree (dependencies reused the same way conflict previews
     reuse them; 5-minute timeout).
6. **Done.** Write the tree, create a two-parent commit (`tip`, `incoming`) authored by the
   signed-in writer's identity, and `update-ref` the branch only if it still equals `tip` (same
   compare-and-swap as `ConflictSessions::finish`). Remove the worktree and the conflict session.
   The project watcher and dev server reload the preview as for any branch update.
7. **Failed / cancelled.** Kill the agent process group, remove the worktree, leave the branch
   untouched, and keep the conflict session so **Advanced** still works.

Fixed prompt (summary): you are combining a writer's draft with newer changes to the live site;
resolve every conflict marker; keep both sides' intent; never drop content silently; do not
commit, push or switch branches; if two sides made genuinely different choices about the same
content, do not guess — write `needs_input` with plain-language questions a non-technical writer
can answer; otherwise write `resolved` with a one-sentence plain-language summary.

### Progress events

The daemon emits `changes.auto_resolve` events over `/ws/events`:
`{ sessionId, state, stage, questions?, summary?, error? }`. `stage` is a fixed plain-language
string built by the daemon from the conflict file labels ("Combining *Pricing page*…",
"Checking the site still builds…"). `error` is one plain sentence from a fixed set:

- "Your AI assistant isn't available." (provider missing or not signed in)
- "The site didn't build after combining."
- "Your AI assistant couldn't finish combining the changes."
- "This took too long."
- "Your draft changed while combining. Please sync again." (tip moved)

### API

Next to the existing `/api/changes/conflicts/{id}/*` routes:

- `POST /api/changes/conflicts/{id}/auto` → `{ jobId }` (409 if a job is already running)
- `GET /api/changes/conflicts/{id}/auto` → current state, stage, questions, summary or error
- `POST /api/changes/conflicts/{id}/auto/answers` `{ answers: { "<qid>": "<text>" } }`
- `POST /api/changes/conflicts/{id}/auto/cancel`

While a job runs, `choice`/`finish`/`abandon` on that session return 409, and checkpoints and
branch switches for that branch are serialised as they are for manual resolution.

## Frontend

### `web/src/modules/changes/SyncDialog.vue` (new)

Uses the existing `dialog` component, centred and not dismissable by clicking outside while
working. Escape or **Stop** asks "Stop combining? Nothing will change." State comes from
`changes.auto_resolve` events, with `GET …/auto` on (re)open.

| State | Content |
|---|---|
| Working | Spinner, headline, current stage line, **Stop** |
| Needs input | `questionnaire` component: one question per step, the agent's options as choices, "Something else…" input, **Continue** |
| Done | Done message, summary, **Close** |
| Failed | "Nothing changed. Your draft is safe.", the error sentence, **Try again**, **Close**, small **Advanced** link |

### Changes

- `GetLatestButton.vue` and the palette entry (`usePaletteGroups.ts`) are relabelled **Sync with
  live site** and call `store.getLatest()` (`POST /api/changes/update`) again instead of sending
  an agent request. When the response contains `conflictSessions`, the store opens `SyncDialog`
  for the current branch's session and starts the auto job.
- Remove the sync request from `agentRequests.ts`.
- `ConflictView` is unchanged and opens only from **Advanced**; Workspace no longer auto-mounts it
  for sessions that have an auto job.

## Testing

- **Rust unit:** result-file parsing (valid, invalid, missing, out-of-range counts); state machine
  including the 3-round cap and cancel; verification (rejects leftover markers, paths outside the
  worktree, failing build); finish refuses when the tip moved; headless argv per provider.
- **Rust integration:** a fake agent script configured through `agents.json` that writes
  `resolved`, `needs_input` then `resolved`, or garbage; asserts the branch only moves on success.
- **Web unit:** `SyncDialog.test.ts` covers every state, answer submission, Stop, Try again and
  Advanced; update `GetLatestButton.test.ts` and `store.test.ts`.
- **E2E:** `e2e/specs/sync-auto-resolve.spec.ts` with the fake agent: create a collision, answer
  one question, see "Done" and the merged content in the preview.

## Out of scope

Background or automatic syncing, field-level content merge drivers, presence indicators, and
replaying earlier agent requests on top of the live site.
