# Content Drill-down Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Content tab's tree-plus-editor screen with a Contentful-style drill-down: a flat collection list, a per-collection entry list, and a full-width editor.

**Architecture:** The route keeps deciding the surface (`/content`, `/content/:collection`, `/content/:collection/:entry`). A new `EntryList.vue` owns the list view, and a pure `entryList.ts` owns filtering, ordering, and row-menu construction, so both are unit-testable without the router. `ContentPanel.vue` keeps the editor untouched and swaps what surrounds it; the @pierre/trees tree and its preload of every collection disappear.

**Tech Stack:** Vue 3 + TypeScript, Pinia, vue-router, `@vue/test-utils` + happy-dom, reka-ui dropdowns via `@/components/ui/dropdown-menu`, Playwright for e2e.

**Spec:** `docs/superpowers/specs/2026-10-31-content-drilldown-design.md`

## Global Constraints

- No Rust, API, or `sessions.json` change. The list reads `store.entries`, which `selectCollection` already loads.
- Routes stay exactly `/p/:id/content`, `/p/:id/content/:collection`, `/p/:id/content/:collection/:entry`; `contentPath()` is unchanged.
- Writer-facing copy is exactly the spec's table: "Choose a collection.", "No entries yet", "No matches", "Filter entries", "Back to all {collection}".
- Rows show the slug and the kind word as stored (`markdown`, `mdx`, `json`). No titles, dates, or statuses.
- Filtering is a case-insensitive substring match on the slug; the header count is the number of rows on screen.
- The sidebar is flat, shows no counts, and is not rendered while an entry is open. It is collapsible only in the list state.
- A singleton collection never shows a list: `/content/{name}` redirects to its entry, and that entry's back link goes to `/content`.
- Test ids are exactly the spec's table; rows carry `data-collection-name` / `data-entry-id` rather than interpolated ids.
- Web tests: `pnpm -C web test -- <path>`. Typecheck: `pnpm -C web exec vue-tsc --noEmit`. E2E: `just e2e` (its content spec may also be run alone; see Task 4).

## Review Focus

1. **Deep link to `/content/{collection}` before the catalog has loaded** — the list must appear when the collections arrive, not a permanent empty pane or a spinner.
2. **A dirty draft plus the back link or a collection row** — navigation must be refused and the draft kept, exactly as the tree behaves today.
3. **Deep link to a singleton `/content/{singleton}`** — must land on its entry, never on a list of one, and its back link must go to `/content`.
4. **Deep link to a collection the sidebar hides (unsupported)** — must show the neutral empty pane, not a broken list or an error.
5. **Empty collection, and a collection that is read-only because the preview is down** — the empty state offers create, and read-only rows still open.

---

### Task 1: Entry-list helpers

**Files:**
- Create: `web/src/modules/content/entryList.ts`
- Test: `web/src/modules/content/entryList.test.ts`

**Interfaces:**
- Produces: `EntryAction`, `filterEntries`, `sortEntries`, `entryActions` — used by Task 2 (`EntryList.vue`) and Task 3 (`ContentPanel.vue`).

- [ ] **Step 1: Write the failing tests**

```ts
import { expect, it } from "vitest";
import { entryActions, filterEntries, sortEntries } from "./entryList";

const row = (id: string) => ({ collection: "blog", id, path: `src/content/blog/${id}.md`, kind: "markdown" as const });
const handlers = { open: () => {}, copyLink: () => {}, copyPath: () => {}, rename: () => {}, remove: () => {} };
const ids = (actions: ReturnType<typeof entryActions>) =>
  actions.flatMap(a => (a === "separator" ? [] : [a.id]));

it("filters by a case-insensitive slug substring", () => {
  const rows = [row("Welcome"), row("spring-launch"), row("about")];
  expect(filterEntries(rows, "SPRING").map(r => r.id)).toEqual(["spring-launch"]);
  expect(filterEntries(rows, "")).toHaveLength(3);
  expect(filterEntries(rows, "zzz")).toEqual([]);
});

it("sorts rows by slug and leaves the input alone", () => {
  const rows = [row("b"), row("a"), row("c")];
  expect(sortEntries(rows).map(r => r.id)).toEqual(["a", "b", "c"]);
  expect(rows.map(r => r.id)).toEqual(["b", "a", "c"]);
});

it("offers open, link, path, rename and delete for an editable entry", () => {
  expect(ids(entryActions({ singleton: false, path: "src/content/blog/welcome.md", dirty: false, handlers })))
    .toEqual(["open", "copy-link", "copy-path", "rename", "delete"]);
});

it("omits the file path when the list did not supply one", () => {
  expect(ids(entryActions({ singleton: false, path: undefined, dirty: false, handlers })))
    .toEqual(["open", "copy-link", "rename", "delete"]);
});

it("keeps a singleton to open and link", () => {
  expect(ids(entryActions({ singleton: true, path: "src/content/site.json", dirty: false, handlers })))
    .toEqual(["open", "copy-link", "copy-path"]);
});

it("disables rename and delete while a draft is dirty", () => {
  const actions = entryActions({ singleton: false, path: "src/content/blog/welcome.md", dirty: true, handlers })
    .filter(a => a !== "separator");
  const rename = actions.find(a => a.id === "rename")!;
  const remove = actions.find(a => a.id === "delete")!;
  expect(rename.disabled).toBe(true);
  expect(rename.hint).toBe("Save or discard your changes first");
  expect(remove.disabled).toBe(true);
  expect(remove.destructive).toBe(true);
});
```

- [ ] **Step 2: Run them and watch them fail**

Run: `pnpm -C web test -- src/modules/content/entryList.test.ts`
Expected: FAIL — `./entryList` has no such exports.

- [ ] **Step 3: Implement `entryList.ts`**

Signatures:

```ts
import type { Component } from "vue";
import type { ContentEntrySummary } from "@/core/types";

/** One row of an entry's menu; `"separator"` draws a divider. */
export type EntryAction = "separator" | {
  id: string;
  label: string;
  icon?: Component;
  destructive?: boolean;
  disabled?: boolean;
  /** Why the action is disabled, shown under it. */
  hint?: string | null;
  run: () => void;
};

export interface EntryActionHandlers {
  open: () => void;
  copyLink: () => void;
  copyPath: () => void;
  rename: () => void;
  remove: () => void;
}

export function filterEntries(entries: readonly ContentEntrySummary[], query: string): ContentEntrySummary[];
export function sortEntries(entries: readonly ContentEntrySummary[]): ContentEntrySummary[];
export function entryActions(ctx: {
  singleton: boolean;
  path?: string;
  dirty: boolean;
  handlers: EntryActionHandlers;
}): EntryAction[];
```

Labels and order are the ones the tree ships today: `Open`, separator, `Copy link`, `Copy file path` (only with a path), separator plus `Rename…` and `Delete` for non-singletons. Icons are `FileText`, `Link`, `Copy`, `Pencil`, `Trash2` from `@lucide/vue`. `sortEntries` copies before sorting (`.slice().sort((a, b) => a.id.localeCompare(b.id))`).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C web test -- src/modules/content/entryList.test.ts`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add web/src/modules/content/entryList.ts web/src/modules/content/entryList.test.ts
git commit -m "feat(content): entry list helpers"
```

---

### Task 2: Entry list view

**Files:**
- Create: `web/src/modules/content/EntryList.vue`
- Test: `web/src/modules/content/EntryList.test.ts`

**Interfaces:**
- Consumes: `EntryAction`, `filterEntries`, `sortEntries` from Task 1.
- Produces: `<EntryList :collection :entries :loading :create-hint :actions @open @create />` — used by Task 3.

- [ ] **Step 1: Write the failing tests**

Mount `EntryList` directly with props (no router). Use the pattern in `web/src/modules/changes/PrMenu.test.ts` for opening a reka dropdown in happy-dom. Cases, in order:

```ts
// collection: { name: "blog", kind: "markdown", editable: true, reason: null }
// entries: [row("welcome"), row("spring-launch")], actions: () => menu, createHint: null

it("lists every entry with its slug as an addressable row")   // find('[data-testid="content-entry"]') length 2; attributes data-entry-id = welcome, spring-launch
it("shows the collection name and the number of rows on screen") // header text contains "2 entries"; after typing "spring" into content-filter it contains "1 entry"
it("filters by slug and says when nothing matches") // type "zzz" -> no rows, text "No matches"
it("says there are no entries yet and offers create") // entries: [], createHint: null -> "No entries yet", content-new-entry enabled
it("disables create and carries the reason") // createHint: "Add an image field first" -> content-new-entry disabled, title contains the reason
it("opens an entry when its row is clicked") // click row -> emitted("open") equals [["welcome"]]
it("runs a row menu action") // open the menu, click content-entry-rename -> handler spy called
it("keeps the filter when the same collection reloads but clears it on a new collection") // set filter, setProps same collection -> kept; setProps other collection -> ""
```

- [ ] **Step 2: Run them and watch them fail**

Run: `pnpm -C web test -- src/modules/content/EntryList.test.ts`
Expected: FAIL — cannot resolve `./EntryList.vue`.

- [ ] **Step 3: Implement `EntryList.vue`**

Props and emits:

```ts
const props = defineProps<{
  collection: ContentCollection;
  entries: readonly ContentEntrySummary[];
  loading: boolean;
  /** Why create is unavailable, or null when it works. */
  createHint: string | null;
  actions: (id: string) => EntryAction[];
}>();
const emit = defineEmits<{ open: [id: string]; create: [] }>();
```

Structure:

- Root `<section data-testid="content-entry-list">`.
- Header: `<h2>{{ collection.name }}</h2>`, a `<span>` with `{{ visible.length }} {{ visible.length === 1 ? "entry" : "entries" }}`, `<Input data-testid="content-filter" placeholder="Filter entries">`, and `<Button data-testid="content-new-entry" :disabled="!!createHint" :title="createHint ?? undefined" @click="emit('create')">New entry</Button>`.
- `<ul>` of `<li>` rows. Each row is a `<div class="group flex items-center gap-1">` holding a full-width `<button type="button" data-testid="content-entry" :data-entry-id="entry.id" @click="emit('open', entry.id)">` with a kind icon, `<span class="truncate font-mono">{{ entry.id }}</span>`, and `<span class="text-xs text-muted-foreground">{{ entry.kind }}</span>`; then, when `actions(entry.id).length`, a `DropdownMenu` whose trigger is `<Button variant="ghost" size="icon-sm" data-testid="content-entry-menu" :aria-label="`Actions for ${entry.id}`">`.
- Menu content renders the actions in order: `DropdownMenuSeparator` for `"separator"`, else `<DropdownMenuItem :disabled="a.disabled" :data-testid="`content-entry-${a.id}`" @select="a.run()">`. After a disabled item with a hint, render `<p class="px-2 pb-1 pl-8 text-xs text-muted-foreground">{{ a.hint }}</p>`.
- `visible = computed(() => sortEntries(filterEntries(props.entries, query)))`; `query` is a local `ref("")` reset by `watch(() => props.collection.name, () => { query.value = ""; })`.
- Empty states: when `!loading && !visible.length`, show "No entries yet" when `!query.trim()` else "No matches". Do not show them while `loading`.
- Compute `rows = computed(() => visible.value.map(e => ({ entry: e, menu: props.actions(e.id) })))` so `actions()` is called once per row.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C web test -- src/modules/content/EntryList.test.ts`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add web/src/modules/content/EntryList.vue web/src/modules/content/EntryList.test.ts
git commit -m "feat(content): entry list view"
```

---

### Task 3: Swap the content surfaces

**Files:**
- Modify: `web/src/modules/content/ContentPanel.vue`
- Modify: `web/src/modules/content/ContentPanel.test.ts`
- Modify: `web/src/modules/content/create.test.ts` (create-button id and singleton assertions only)

**Interfaces:**
- Consumes: `EntryList.vue`, `EntryAction`, `entryActions` (Tasks 1-2); the existing `store.{collections, selectedCollection, entries, entry, loading, dirty}`, `store.selectCollection`, `store.selectEntry`, `store.startCreate`, `store.startLifecycle`, `store.cancelCreate`.
- Produces: the three route surfaces, `content-back`, `content-collections`, `content-collection`, `content-collection-new`, `content-new-entry`.

- [ ] **Step 1: Write the failing tests**

Rewrite `ContentPanel.test.ts` around the same harness it uses today (`contentRouter`, `createFakeDaemon`), dropping the `vi.mock("./ContentTree.vue", …)` line. Keep the existing coverage that does not concern the tree, and add/adjust:

- `it("lists collections, then entries, then opens the editor")` — start at `/p/p/content`; no `content-entry-list` yet and the pane says "Choose a collection."; click `[data-collection-name="blog"]`; URL is `/p/p/content/blog`; two rows render (fake daemon returns two entries); click `[data-entry-id="welcome"]`; URL is `/p/p/content/blog/welcome`; `content-source` shows the entry's source; the sidebar (`content-collections`) is absent.
- `it("returns to the list from the editor")` — start at the entry URL; click `content-back`; URL is `/p/p/content/blog`; `content-entry-list` is visible again.
- `it("opens a singleton's entry and returns to the collection index")` — collections are `[{ name: "site", kind: "json", singleton: "site", … }]`; start at `/p/p/content/site`; URL becomes `/p/p/content/site/site`; `content-source` is visible; click `content-back`; URL is `/p/p/content`.
- `it("shows the list for a deep link once the catalog arrives")` — make `listContentCollections` resolve on a deferred promise; start at `/p/p/content/blog`; assert the pane is not an error and no `content-entry` exists yet; resolve; assert rows appear (Review Focus 1).
- `it("refuses the back link while the draft is dirty")` — open an entry, type into the body, stub `confirm` to `false`, click `content-back`; URL still names the entry and the draft text is intact (Review Focus 2).
- `it("shows the neutral pane for a collection the sidebar hides")` — collections include `legacy` with `supportedForEditing: false`; start at `/p/p/content/legacy`; `content-collections` does not list `legacy` and the pane reads "Choose a collection." (Review Focus 4).
- `it("offers create in an empty list and opens a read-only collection")` — empty `listContentEntries` shows "No entries yet" with an enabled `content-new-entry`; with `editable: false, supportedForEditing: true` the collection is listed and its entries still open (Review Focus 5).
- `it("restores the last collection's list when the tab reopens")` — replaces today's entry-restore test: start at the entry URL, route away, route to `/p/p/content`; the URL becomes `/p/p/content/blog`, not the entry.

Also update the two existing navigation tests that find `content-entry` while the editor is open (they must assert the editor header's id instead) and keep the preview-down, conflict, and dirty-draft tests as they are.

In `create.test.ts`: replace every `content-create` / `content-create-reason` lookup with `content-new-entry` plus its `title`; the singleton case now asserts `content-new-entry` is absent because the editor opened instead of a list.

- [ ] **Step 2: Run them and watch them fail**

Run: `pnpm -C web test -- src/modules/content/ContentPanel.test.ts src/modules/content/create.test.ts`
Expected: FAIL — no `content-entry-list`, no `content-back`, no `content-new-entry`.

- [ ] **Step 3: Implement the surfaces in `ContentPanel.vue`**

Script changes:

- Delete `ContentTree` and `contentTree` imports, `treeEntries`, `entryLoads`, `loadEntries`, `treeSelected`, `treeActions`, `openEntry`'s tree use, the `watch(validCollections, … loadEntries …)` preload, and the `watch(store.entries, …)` reload. Keep `unsupported`, `reportedInvalid`, the hide toast, `removable`, `createBlocked`, `copy`, `startCreate`, `confirmCreate`, `lifecycleFor`, and every editor concern.
- Add `import EntryList from "./EntryList.vue"; import { entryActions, type EntryAction } from "./entryList";` and `ChevronLeft` from `@lucide/vue`.
- `const editing = computed(() => !!props.entry);`
- `const activeCollection = computed(() => validCollections.value.find(c => c.name === props.collection) ?? null);`
- `const selectedCollection = computed(() => store.selectedCollection);`
- `function openCollection(c: ContentCollection) { void router.push(contentPath(props.projectId, c.name, c.singleton ?? undefined)); }`
- `function backToList() { const c = store.collections.find(x => x.name === props.collection); void router.push(contentPath(props.projectId, c?.singleton ? undefined : props.collection)); }`
- `function rowActions(id: string): EntryAction[]` returns `entryActions({ singleton: !!activeCollection.value?.singleton, path: store.entries.find(e => e.id === id)?.path, dirty: store.dirty, handlers })`, where `handlers` opens the entry, copies the link and the file path with the existing `copy()`, and runs `lifecycleFor("rename" | "delete", name, id)`.
- `openEntry` pushes the entry URL (unchanged).
- `const singletonOf = (name: string) => store.collections.find(c => c.name === name)?.singleton ?? null;`
- Singleton redirect: in `follow()`, when `collection` is given and `id` is missing and the named collection is a singleton, `router.replace(contentPath(props.projectId, collection, singleton))` and return. Add a `watch(validCollections, …)` that applies the same replace when the route names a singleton collection and no entry — this covers the deep-link case where the catalog had not loaded yet.
- Reopen behaviour: in `follow()`'s `!collection` branch, replace `showInUrl()` with `void router.replace(contentPath(props.projectId, store.selectedCollection))` when the store remembers a collection.

Template changes:

- Wrap the surface in `!editing` vs `editing`.
- In `!editing`: keep `SidebarProvider`/`ResizablePanelGroup`; the sidebar panel renders `<nav data-testid="content-collections" class="…px-1 py-2">` with one row per `validCollections` entry: a `<button type="button" data-testid="content-collection" :data-collection-name="c.name" :aria-current="selectedCollection === c.name ? 'page' : undefined" class="…" @click="openCollection(c)">` plus, when `!c.singleton`, a `<Button variant="ghost" size="icon-sm" data-testid="content-collection-new" :aria-label="`New ${c.name} entry`" :title="createBlocked(c) ?? 'New entry'" :disabled="!!createBlocked(c)" @click="startCreate(c.name)"><Plus /></Button>`. Keep the existing `content-toggle-sidebar` button and the collapse/expand ref on the collection panel.
- The main pane in `!editing` renders `<EntryList v-if="activeCollection" :collection="activeCollection" :entries="store.entries" :loading="store.loading" :create-hint="createBlocked(activeCollection)" :actions="rowActions" @open="openEntry(activeCollection.name, $event)" @create="startCreate(activeCollection.name)" />`; otherwise the existing loading line, and `"Choose a collection."` when not loading.
- In `editing`: render only the editor panel, keyed `:key="'editor'"` on the `ResizablePanelGroup`; the sidebar panel and handle are `v-if="!editing"`. In the editor header, replace the `content-toggle-sidebar` button with `<Button type="button" variant="ghost" size="icon" class="size-8 text-muted-foreground" data-testid="content-back" :aria-label="backLabel" @click="backToList"><ChevronLeft class="size-4" /></Button>` where `backLabel = props.collection && !singletonOf(props.collection) ? `Back to all ${props.collection}` : "Back to all content"`. Keep rename/delete/fields toggles, the word count, and the binding `<p>`.
- Keep every existing empty-catalog (`content-empty-catalog`) and editor node unchanged.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C web test -- src/modules/content/ContentPanel.test.ts src/modules/content/create.test.ts src/modules/content/lifecycle.test.ts src/modules/content/draft.test.ts src/modules/content/ImageDialog.test.ts src/modules/content/ContentHistory.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add web/src/modules/content/ContentPanel.vue web/src/modules/content/ContentPanel.test.ts web/src/modules/content/create.test.ts
git commit -m "feat(content): drill down from collections to entries"
```

---

### Task 4: Remove the tree and update e2e

**Files:**
- Delete: `web/src/modules/content/ContentTree.vue`, `web/src/modules/content/contentTree.ts`, `web/src/modules/content/contentTree.test.ts`, `web/src/test/contentTreeStub.ts`
- Modify: the `vi.mock("./ContentTree.vue", …)` line in `lifecycle.test.ts`, `create.test.ts`, `draft.test.ts`, `ImageDialog.test.ts`, `ContentHistory.test.ts`
- Modify: `e2e/specs/content-editor.spec.ts`

**Interfaces:**
- Consumes: the test ids Task 3 produced.
- Produces: no new API; the tree module is gone.

- [ ] **Step 1: Confirm nothing imports the tree**

Run: `grep -rn "ContentTree\|contentTree\|TreeAction\|TreeTarget\|entryTreePath\|treePaths\|treeTarget" web/src`
Expected: only the files listed for deletion. If anything else appears, stop and report it rather than guessing.

- [ ] **Step 2: Delete the files and the mock lines**

Remove the four files and the five `vi.mock("./ContentTree.vue", () => import("@/test/contentTreeStub"));` lines. `lib/treeExpansion.ts` stays: `web/src/modules/files/ProjectFilesTree.vue` still uses it.

- [ ] **Step 3: Run the unit suite**

Run: `pnpm -C web test`
Expected: PASS, with no reference to a missing module.

- [ ] **Step 4: Rewrite the e2e navigation**

In `e2e/specs/content-editor.spec.ts`:

- Replace `treeRow(page, path)` with two helpers: `collectionRow = (page, name) => page.locator(`[data-collection-name="${name}"]`)` and `entryRow = (page, id) => page.locator(`[data-entry-id="${id}"]`)`.
- `openBlog`: click `collectionRow(page, "blog")` and `await expect(page.getByTestId("content-entry-list")).toBeVisible()`; the tree's expand/collapse dance goes away.
- Opening an entry: `await entryRow(page, "welcome").click()`; the `entryBinding` assertion is unchanged.
- Creating: click `page.getByTestId("content-new-entry")` instead of right-clicking the folder and `content-tree-create`.
- After create the editor opens, so replace the two assertions that read the list row `blog/${SLUG}` with the editor's own evidence: `entryBinding(...)` (already asserted) and `field-title`. Replace `await expect(article).toHaveCount(0)` / `await expect(article).toBeVisible()` with the equivalent claim that needs no navigation — the article's absence on the other branch is already proven by the `git show ${other}:…` assertions later in the spec; keep those and drop the locator. If a UI claim is still wanted there, assert the open entry's binding (file + branch) instead.
- Before switching to `blog/welcome` while the editor is open, click `content-back` first, then the entry row.

- [ ] **Step 5: Run the e2e spec**

Run: `just e2e`
Expected: the content spec passes. The four pre-existing `branch-protection-hint` / `share-preview` failures are out of scope for this plan; if they appear, report the counts rather than fixing them here.

- [ ] **Step 6: Full gates**

Run: `cargo test && pnpm -C web test && pnpm -C web exec vue-tsc --noEmit`
Expected: all pass (Rust is untouched, so it is a regression check only).

- [ ] **Step 7: Commit**

```bash
git add -A web/src e2e/specs/content-editor.spec.ts
git commit -m "refactor(content): remove the content tree"
```
