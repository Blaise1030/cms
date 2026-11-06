# Sync as a sidebar chat (spec)

Status: **draft for review**, 2026-10-28. Decisions settled in conversation with
the product owner.

Builds on:

- `2026-10-15-agent-conflict-resolution-design.md` (agent combines sync collisions)
- `2026-10-22-git-collab-non-tech-design.md` (vocabulary, plain-language copy)
- `2026-10-10-visual-conflict-resolution-design.md` (manual chooser, the escape hatch)
- `2026-10-17-headless-chat-panel-design.md` (ACP chats, sidebar, transcript)

## Problem

Combining a draft with the live site is a fire-and-forget job behind a modal.
The writer sees a spinner, then either "Done" or a generic failure, and has no
way to influence how the assistant interprets their intent. The assistant, in
turn, runs headless: its only output is a `result.json` on disk, and its only way
to ask a question is a canned 2–3-option questionnaire. When it fails, the
modal offers "Try again" and nothing else — the same prompt runs again with no
new information.

The agent sidebar already has everything needed for a better conversation:
streaming prose, free-form replies, history, restart and reconnect. None of it is
used for combining.

## Goal

Combining happens in a normal sidebar chat, opened as soon as a collision is
detected. The writer watches the assistant work and can interject in their own
words. The draft and preview stay untouched until verification passes, and the
assistant's working tree is isolated and throwaway.

Success: a writer who has never used git can collide with a teammate, answer the
assistant in plain language, and end up either combined or safely unchanged —
never seeing the words branch, merge, conflict, commit, or a file path.

## Decisions

| # | Decision |
| --- | --- |
| 1 | The sync surface is the agent sidebar. `SyncDialog` is deleted. |
| 2 | Each collision opens a **new** sync chat. It is read-only once it ends. |
| 3 | Landing is automatic, gated on verification at the end of every agent turn. |
| 4 | A sync always reaches an explicit terminal phase: `combined`, `ended`, or `expired`. |
| 5 | A sync chat is a chat with a profile (`kind = sync`) sharing the existing chat manager. |
| 6 | The merge worktree is durable (`{state_dir}/sync/{chatId}/site`) so a restart can resume. |
| 7 | Git vocabulary and tool output are filtered from sync transcripts. |
| 8 | The questionnaire, `result.json`, rounds, and the auto-resolve job are retired. |

This spec supersedes the sync-collision sections of `2026-10-15` and `2026-10-22`
(specifically their SyncDialog, needs-input questionnaire, and failure-dialog
behavior). Its vocabulary and error copy are adopted unchanged. The manual visual
chooser remains the only alternate path.

## Vocabulary and plain language

Settled words only: **project / site, change / draft, get latest, combine,
publish, live**. Never in writer-facing text: branch, commit, merge, conflict,
PR, checkout, clone, worktree, repo, origin, main, markers, npm.

Copy is canonical:

| Moment | Text |
| --- | --- |
| Opening message | "You and a teammate both changed {pages}. I'll keep both changes and check the site still works." |
| Stages | "Reading your pages…", "Comparing the two versions…", "Checking the site still works…" |
| Combined | "Done. Your changes and the latest site are combined." |
| Decision needed | "{n} pages still need a decision." |
| One file left | "One file still needs another look." |
| Unexpected failure | "I couldn't finish combining. Nothing changed." |
| Build failed once | "The site didn't work after those changes. I'll try once more." |
| Build failed twice | "I couldn't get your site working again. Nothing changed." |
| Draft moved | "Your draft changed while combining. Get latest again." |
| Writer ended | "You ended this combine. Nothing changed." |
| Expired | "This combine ran out of time. Nothing changed." |
| Assistant missing | "Your assistant isn't available." |
| Turn too long | "That took too long. Tell me to continue when you're ready." |
| Recovery failed | "I lost track of this combine. Nothing changed." |
| Actions | "Pick the changes myself", "Combine again", "End sync" |

The assistant's instructions (not the transcript) also require: talk about pages
and paragraphs, quote the two versions verbatim when asking, offer choices the
way a person would, invite "I don't understand" and rephrase, never commit, never
abort, never switch drafts, never touch pages that didn't collide, and ask any
last question before finishing the last page rather than after.

## Flow

1. Writer clicks **Get latest**. A collision is detected as today.
2. The daemon creates the worktree at the draft tip and merges incoming.
3. It creates a sync chat titled "Combining {pages}" and starts an ACP session
   with that worktree as cwd. The sidebar selects it, opening the panel if needed.
4. One prompt is sent, containing the writer-facing intro and the agent
   instructions. The transcript records only the intro.
5. The writer watches, and may type at any time while the assistant works.
6. Every completed turn is verified (§ Verification). Passing checks build, then
   land. Failing checks post one plain line and the conversation continues.
7. The sync ends (§ Ending) and the chat becomes read-only.

The draft checkout and preview are never touched: all agent work happens in the
worktree. If the writer saves further edits to the draft while the sync runs, the
land is refused (`DraftMoved`) rather than overwriting them.

## Phases

```
working ──turn end──▶ checking ──pass──▶ build ──pass──▶ combined
   ▲                     │                  │
   └─────fail────────────┘                  └──fail──▶ working (one repair)
   │
   └── writer ends / TTL / hard failure ──▶ ended | expired
```

`checking` is transient and covers both verification and the build (it is the
only phase in which the composer is locked). Terminal phases are `combined`,
`ended`, `expired`.

## Data model

`SessionRecord` (`src/chat/history.rs`) grows optional fields; existing records
deserialize unchanged:

```rust
pub kind: RecordKind,          // #[serde(default)] Chat | Sync
pub cwd: Option<PathBuf>,      // sync: the merge worktree
pub sync: Option<SyncState>,   // sync only
```

```rust
pub struct SyncState {
    pub conflict_session_id: String,
    pub branch: String,
    pub tip: String,
    pub incoming: String,
    pub files: Vec<String>,
    pub phase: SyncPhase,      // working | checking | combined | ended | expired
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub repair_attempted: bool,
}
```

The branch, tip, incoming, and file list are copied from the conflict session at
start, so a sync can verify and land after a restart even though conflict
sessions themselves are in-memory and do not survive one.

On disk:

- Worktree: `{state_dir}/sync/{chatId}/site` — durable across daemon restarts.
- Build/turn logs: `{state_dir}/sync-logs/{chatId}.log` (unchanged naming,
  chat id instead of conflict-session id).

Provider and model come from the project's configured assistant (the same pick
the chat panel uses). There is no picker; if none is configured, no sync chat is
created and the writer sees "Your assistant isn't available."

## Verification

Runs on every turn end where `kind = sync` and `stop_reason = end_turn`, in the
worktree:

1. **Draft moved** — the branch tip no longer equals the session tip. End.
2. **Agent moved git** — worktree `HEAD` no longer equals the tip, or
   `MERGE_HEAD` no longer equals incoming (commit, abort, switch). One note, stay
   `working`.
3. **Leftovers** — conflict markers remain in a conflicted page, or a new symlink
   was introduced that wasn't on the incoming side. One note naming the count of
   pages, stay `working`.
4. Stage everything, then **build** (`npm run build`, 300 s cap, `node_modules`
   linked into the worktree).
5. Build failure or timeout: one automatic repair prompt carrying the tail of the
   build log, then a rebuild. A second failure ends the sync with the "couldn't
   get your site working again" copy. `repair_attempted` bounds this at one.
6. Build passes: write the tree, create the merge commit, and land under the git
   lock (`advance_branch`; a 409 is `DraftMoved`). Then remove the worktree,
   abandon the conflict session, drop previews, phase `combined`.

A sync chat keeps a per-turn watchdog (10 minutes, cancel + note). Ordinary chats
do not.

## Ending

All three terminal phases share one cleanup: cancel a running turn, remove the
worktree, mark `ended_at`, append a final status line, and make the chat
read-only. They differ only in copy and in the conflict session's fate:

| Phase | Trigger | Conflict session |
| --- | --- | --- |
| `combined` | build + land passed | abandoned (done) |
| `ended` | **End sync**, hard failure, draft moved, second build failure, delete of the chat | kept, so "Pick the changes myself" and a fresh sync still work |
| `expired` | the sync's own 1-hour TTL (the conflict session may already be gone) | expiring |

The final transcript line carries **Pick the changes myself** and **Combine
again**. Ending a sync is not abandoning the conflict.

Deleting a sync chat through the normal chat delete path ends the sync first, so
a worktree cannot be orphaned from history.

## Recovery

On boot, every non-terminal sync record is checked:

- Worktree exists with the expected `HEAD` and `MERGE_HEAD` → leave it `working`.
  The agent restarts lazily on the next message.
- Anything else → end with the recovery copy and clean up.

The worktree is only removed while no agent process is running in it: the ending
cancels the turn and waits for the manager to report the chat idle.

## API

New, under the existing conflict-session routes:

- `POST /api/changes/conflicts/{id}/sync` `{ project }` → `{ chatId }`. 409 if a
  sync is already live for the project.
- `GET /api/changes/conflicts/{id}/sync` → `{ chatId, phase }` or 404. Lets the
  UI reopen a sync after a refresh.
- `POST /api/changes/conflicts/{id}/sync/{chatId}/end` → 204.

Removed: `POST …/auto`, `POST …/auto/answer`, `POST …/auto/cancel`, and the
`changes.auto_resolve` event (replaced by `sync.changed` plus the chat's own
stream). `sync.changed` carries `{ projectId, conflictSessionId, chatId, phase }`
and fires on creation, every phase transition, and ending.

Chat endpoints are unchanged. `GET /api/chat/{project}/{chat}` additionally
returns `kind` and `sync` so the sidebar can badge and gate the composer.
`not_combining` consults the sync record instead of an in-memory job.

## Frontend

- `SyncDialog.vue`, its tests, and the `syncStatus`/`answerSync`/`stopSync`/
  `retrySync`/`onAutoResolve` machinery are deleted.
- `startSync` becomes: call the sync endpoint, ensure the chat list contains the
  new chat, select it, open the panel.
- Sync chats render only intro, prose, stage lines, and notes. Tool calls,
  reasoning, diffs, paths, and raw errors are filtered at render time (raw data
  stays recorded for support).
- Header: phase label and **End sync** while non-terminal. Composer is locked
  during `checking` and read-only after any terminal phase.
- Sidebar list: state badge (Combining / Combined / Ended), title from the
  collided pages.
- Changes view keeps the collision banner and its manual escape hatch. On
  `sync.changed` with phase `combined`, the changes store refreshes the tree,
  preview, change list, and conflict sessions, as the old done handler did.

## Retired

`AutoResolveJobs`, `JobState`, `JobStatus`, `Timeouts` (agent/answer), `MAX_ROUNDS`,
`parse_result`, the `needs_input` questionnaire, `result.json`, and
`tests/auto_resolve.rs` (replaced by sync-chat tests). `stage_and_verify` is
refactored to return a typed verdict instead of a single `AgentFailed`.

## Out of scope

- More than one live sync per project.
- Retargeting a finished sync chat at the site checkout; it ends read-only.
- Choosing a provider or model per sync.
- Automatic retries beyond the single build repair.
- Live co-editing, or any change to publish/share flows.
- Changing ACP itself beyond recording one new CMS-only chunk (`sync_status`).

## Testing

**Backend unit**

- Phase transitions, including that `checking` never persists across a restart.
- Each verification verdict: draft moved, agent moved git, markers, symlink,
  build failure, second build failure, pass.
- Every user-visible string is checked against the forbidden-word list.
- Ending removes the worktree and marks the record; deleting a sync chat does the
  same.

**Backend integration**

- Collision → sync chat exists, its cwd is the worktree, draft tip and preview
  unchanged.
- Agent resolves → turn end → build → land → `combined`, branch moved once.
- Markers remain → note recorded, chat stays live, no land.
- Draft moves mid-sync → `ended`, draft untouched, no land.
- End sync → cleanup, `ended`, conflict session still usable.
- TTL → `expired`.
- Daemon restart → resume when the worktree matches; `ended` otherwise.
- Missing assistant → immediate `ended` with the right copy.
- Turn watchdog cancels and notes.
- Two syncs in one project → 409.

**Frontend**

- SyncDialog is gone; store starts and selects a sync chat.
- Tool and reasoning parts are filtered from sync transcripts.
- Composer locked in `checking`, read-only when terminal, both actions present.
- Badge and title appear in the sidebar list.
- Transcript vocabulary assertion on a full scripted sync.

**Acceptance (extends `2026-10-22` test 4)**

A writer who has never used git triggers a forced collision, is combined
automatically, and the entire transcript contains no git vocabulary, no commands,
no diffs, and no file paths. A forced failure leaves the draft byte-identical and
offers "Combine again" and "Pick the changes myself".
