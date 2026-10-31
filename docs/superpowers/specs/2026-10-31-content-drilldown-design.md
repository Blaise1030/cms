# Content drill-down: collections → entries → entry

**Status:** approved design, awaiting implementation plan
**Date:** 2026-10-31
**Scope:** `web/src/modules/content`, one e2e spec. No backend change.

## Problem

The Content tab renders three horizontal regions at once: a resizable sidebar
holding a file tree (every collection as a folder with its entries nested
inside), the entry editor, and the editor's own Fields/History panel. Nested
entries inside a narrow column consume width that the writing surface needs,
and the entry list is never shown at a size where scanning it is pleasant.

## Goal

Adopt the Contentful-style drill-down: content collections first, then a list of
one collection's entries, then the entry. Writers reach an entry in two clicks
and the editor gets the full width.

## Navigation model

URLs do not change, so deep links, reloads, and browser history keep working.

| Route | Screen |
| --- | --- |
| `/p/:id/content` | Collection list in the sidebar; empty pane reads "Choose a collection." |
| `/p/:id/content/:collection` | Collection list + entry list for that collection |
| `/p/:id/content/:collection/:entry` | Editor at full width; the sidebar is not rendered; the header shows a back link |

The back link navigates to the collection's list. Browser back/forward steps
through the same three states.

## Layout

### Collection sidebar

- Flat list, one row per editable collection: name and its kind icon. No
  nesting, no entry counts, no search box.
- The open collection is highlighted.
- Each row's "…" menu offers **New entry** when the collection accepts one; the
  existing disabled reason appears as the menu hint.
- Unsupported collections stay hidden behind the existing warning toast.
- Collapsible in the list state only. The editor state hides the sidebar
  entirely, so the editor owns the panel's full width.

### Entry list

Rendered in the main pane beside the sidebar.

- Header: collection name, entry count, a filter input, and **New entry**.
- Rows: kind icon, slug, kind label. The whole row navigates to the editor.
- Row "…" menu: **Open**, **Copy link**, **Copy file path**, **Rename…**,
  **Delete**. Singletons offer none of the last three, matching today.
- Entries sort by slug. Nested ids stay flat and display their slashes
  (`guides/setup`); flattening is the point of the list.
- States: loading, error, empty ("No entries yet" plus the create action),
  filtered empty ("No matches"), and read-only (rows still open; the editor
  explains why).

### Editor

Internals are unchanged: body editor or JSON fields, the Fields/History side
panel, the unsaved-changes bar, lifecycle dialogs, conflict handling, and the
dirty-draft guards. The header gains the back link in place of the collections
toggle.

## Data flow

No API change. The list renders `store.selectedCollection` and `store.entries`,
which `selectCollection` already loads through `listContentEntries`.

The panel's parallel preload of every collection's entries
(`treeEntries`, `entryLoads`, `treeSelected`, `treeActions`) is deleted with
the tree; the sidebar no longer needs entry data.

## Components

**Create**

- `web/src/modules/content/EntryList.vue` — the list view: filter, rows, row
  menu, empty states, create entry point.
- `web/src/modules/content/entryList.ts` — pure helpers the list and tests
  share: filtering, row ordering, and `entryActions()` (the row menu logic that
  `ContentPanel.vue` owns today).
- `web/src/modules/content/EntryList.test.ts`.

**Modify**

- `ContentPanel.vue` — route-driven surface switches (sidebar only, list,
  editor), back link, wiring to `EntryList`, removal of the tree wiring, and
  the collection row menu.
- `ContentPanel.test.ts` — rewritten for the three states.

**Delete**

- `ContentTree.vue`, `contentTree.ts`, `contentTree.test.ts`, and
  `web/src/test/contentTreeStub.ts`.
- The `ContentTree` stub mocks in `lifecycle.test.ts`, `create.test.ts`,
  `draft.test.ts`, `ImageDialog.test.ts`, and `ContentHistory.test.ts` (they
  exist only because @pierre/trees cannot lay out in happy-dom).

**Keep**

- `lib/treeExpansion.ts` — the Files module still uses it.
- Every other content component: `FrontmatterForm`, `MarkdownBodyEditor`,
  history, dialogs, `create.ts`, `draft.ts`, `references.ts`.

## Edge cases

- **Singleton** (`ContentCollection.singleton` names the one entry's id):
  selecting the collection opens that entry directly. The list is never shown
  for a singleton: `/content/{singleton}` redirects to the entry, and the
  editor's back link returns to the collection list (`/content`). A one-row
  list would only add a step.
- **Nested ids**: one flat row per entry, slug shown in full.
- **Filter**: case-insensitive substring match on the slug. The header count
  reflects the rows on screen, so a filtered list reads as "3 entries" while
  showing 3 of 12.
- **Preview down**: supported collections keep working as today; unsupported
  ones stay hidden; a read-only collection's rows still open.
- **Dirty draft**: the existing `canLeave()` guards still intercept collection
  changes, entry changes, and route updates, including the back link.
- **Create**: the existing dialog; on success the URL moves to the new entry.
- **Rename/delete**: reachable from both the list row menu and the editor
  header; the lifecycle dialog is unchanged.
- **Empty catalog**: the existing "needs the orso-astro integration" message
  stays.

## Behaviour change

Reopening the Content tab with nothing in the URL restores the last
**collection's list**, not the last entry. The editor still reloads whenever
its URL names an entry. This is deliberate: jumping straight back into an entry
would skip the new layers.

## Copy

Writer-facing strings this change introduces:

| Moment | Text |
| --- | --- |
| Nothing selected | "Choose a collection." |
| Empty list | "No entries yet" |
| Filtered empty | "No matches" |
| Filter placeholder | "Filter entries" |
| Back link label | "Back to all {collection}" |

Existing strings are reused verbatim: "No content collections", the
orso-astro integration explanation, the unsupported-collection warning, the
read-only reason, and every dialog.

The UI calls these "collections" rather than "content types" to stay consistent
with the copy already shipped in the empty catalog state. Renaming that
vocabulary app-wide is out of scope.

## Test ids

| Id | Meaning |
| --- | --- |
| `content-panel` | The module root (unchanged) |
| `content-collections` | The collection sidebar |
| `content-collection` | A collection row, with `data-collection-name` |
| `content-entry-list` | The list view |
| `content-filter` | The list filter |
| `content-entry` | An entry row, with `data-entry-id` (nested ids keep their slashes) |
| `content-back` | The editor's back link |
| `content-new-entry` | The list's create button |
| `content-form`, `content-source`, `content-side`, `content-fields`, `content-unsaved`, `content-save`, `content-discard`, `content-toggle-fields` | Editor internals (unchanged) |
| `content-create-dialog`, `content-lifecycle`, `content-restore-confirm` | Dialogs (unchanged) |

## Tests

**Unit**

- `EntryList.test.ts`: filtering, ordering, empty and filtered-empty states,
  row menu contents per collection kind (singleton, read-only, dirty draft),
  and creating an entry from the list.
- `ContentPanel.test.ts`: the three route states, back navigation, singleton
  direct-open, dirty-draft interception, empty catalog, unsupported-collection
  toast, preview-down, and the reopen-to-list behaviour.
- Deleted with their subjects: `contentTree.test.ts`.

**E2E**

- `e2e/specs/content-editor.spec.ts`: replace the `content-tree` locator and
  the tree's create menu item with a collection row and a list row; the rest of
  the spec (editing, saving, history, disk-change notices) stays.

## Non-goals

- Titles, timestamps, statuses, or any new column (would need backend work).
- Global cross-collection search.
- Pagination or virtualisation. The list filters client-side over one
  collection; if a collection grows large enough to need virtualisation, that
  is a separate change.
- Drag-to-reorder, bulk actions, or a new collection picker in the editor.
- Any Rust, API, or `sessions.json` change.
