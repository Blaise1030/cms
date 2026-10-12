# Per-entry content history (design)

Status: **draft 2026-10-13**, awaiting review.

## Intent

Writers editing one content entry need to see how that entry evolved, compare any earlier version with what is there now, and bring an old version back. Today the only history is the project-wide Checkpoints panel, which shows whole commits across every file. This adds a History view scoped to a single entry, inside the Content editor.

## Success criteria

1. The Content editor's right panel has **Fields** and **History** tabs. History lists every commit that changed the open entry's file, newest first, with a relative timestamp ("Yesterday at 6:36 PM"), the author and the checkpoint summary.
2. Each version carries one badge. The newest is **Current**. Older versions reachable from `main` are **Published**. Older versions on the change branch only are **Draft**.
3. Selecting a version replaces the body editor with a unified diff from that version to the entry's saved file. "Back to editor" or Esc clears the selection. If the editor has unsaved edits, a notice says the diff is against the last save.
4. **Restore this version** loads the version's fields and body into the editor as unsaved edits. Nothing is written until the writer saves, and that save creates a normal checkpoint. Restore needs an active change, like every other content mutation.
5. Renamed entries keep their history across the rename.
6. An entry with no commits shows "No history yet". An old version that cannot be parsed still shows its diff, but Restore is disabled with a reason.

## Decisions

| Area | Decision |
| --- | --- |
| Scope of history | Commits reachable from HEAD that touched the file. On a change branch that includes everything inherited from `main`. |
| Published | Reachable from local `main`. On `main` itself every older version is Published. |
| Diff target | The saved file (`entry.source`), not the unsaved draft. The frontend has no format-preserving serializer, and serializing in the browser would invent formatting diffs. |
| Restore | Copies into the draft only. No git reset, no direct write. |
| Addressing | `collection` + `id`, matching the existing `/api/content/entry`. The path is resolved through the catalog, never taken from the client. |
| Placement | A History tab in the existing right `<aside>` of `ContentPanel.vue`. No new dialog. |

## Backend

### `src/git.rs`

- `file_log(dir, path) -> Vec<FileVersion>` runs `git log --follow --format=%H%x1f%ct%x1f%an%x1f%s -- <path>`. `FileVersion` extends the fields of `Checkpoint` with `path`, the file's path at that commit (taken from `--name-only` so renames resolve), and `published: bool`.
- `published` is computed with one call, `git log --follow --format=%H main -- <path>`. The result goes into a set, and a version is published when its SHA is in the set. If `main` does not exist, everything is `false`.
- `file_at(dir, sha, path) -> String` validates `sha` with the existing `resolve()`, which accepts only hex SHAs reachable from HEAD. It then runs `git show <full>:<path>`. Non-UTF-8 content returns `badreq`.
- An unborn HEAD returns an empty log, like `log()` does.

### `src/http/history.rs` (new), wired in `src/http/mod.rs`

- `GET /api/content/history?collection=&id=` resolves the entry via `catalog::read_entry`, which gives its path, and returns `{ versions: FileVersion[] }`.
- `GET /api/content/history/{sha}?collection=&id=` resolves the entry and checks that `sha` is in that entry's `file_log`. That check rejects reading other files through a valid SHA. It then reads `file_at(sha, version.path)` and returns `{ source, fields, body, parseable }`, built with `catalog::build_entry`. `parseable` is false when a JSON entry's fields come back `Null`, or when the frontmatter fails to parse.

## Frontend

### `web/src/modules/content/history/`

- **`store.ts`** (Pinia, matching `checkpoints/store.ts`):
  - `versions` is keyed by `collection/id`.
  - `load(collection, id)` fetches the version list and `fetchVersion(sha)` fetches one version.
  - The selected SHA is held per entry.
  - The store reloads on the existing checkpoint WebSocket event when the event touches the open entry's path.
- **`VersionsList.vue`:** a vertical list styled like the reference screenshot. Each row has a radio dot, a relative time and a badge, with author and summary on a second muted line. Uses existing `ui` components (badge, a radio-like button).
- **`VersionDiff.vue`:** builds a patch from `version.source` against `entry.source` using `@pierre/diffs`, and renders it through the lazy-loaded `DiffView` from `modules/checkpoints`. The header holds "Back to editor", "Restore this version" and the unsaved-edits notice.

### `ContentPanel.vue`

- The right `<aside>` gets Fields/History tabs. JSON entries, which have no Fields panel today, get the aside with the History tab only.
- When a version is selected, the body column renders `VersionDiff` in place of the body editor. The Fields form stays hidden while History is the active tab.
- **Restore** sets the draft fields and body from the fetched version, then clears the selection. `diffDraft` marks the draft as changed, and the existing Save button and dirty-state guards apply.

## Error handling

- History fetch failure shows an inline error with Retry in the tab. The editor is unaffected.
- A version fetch for a SHA that is no longer reachable (after Get latest, or a branch switch) returns `enoent`. The store reloads the list and clears the selection.
- Restore while on `main` is disabled with the same "start a change" hint used by other mutations.

## Testing

- **Rust (`tests/`):**
  - `file_log` ordering, `--follow` across a rename, `published` true for commits on `main` and false for branch-only ones, and an empty log on a fresh repo.
  - The version endpoint rejects non-hex SHAs and SHAs that don't touch the entry, and returns parsed fields and body for markdown and JSON.
- **Vitest:**
  - `VersionsList` badges (Current, Published, Draft).
  - Selecting a version shows the diff and hides the body editor.
  - Restore populates the draft and marks it dirty.
  - The empty and error states.

## Out of scope

- Diffing against the unsaved draft.
- Comparing two arbitrary versions.
- Per-field (structured) diff.
- Showing history from other branches that HEAD has not merged.
