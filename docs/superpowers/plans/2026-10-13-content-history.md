# Per-entry Content History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A History tab in the Content editor listing every commit that touched the open entry, with Current/Published/Draft badges, a diff of any version against the saved file, and Restore-into-draft.

**Architecture:** Two git helpers in `src/git.rs` (per-file log, file-at-commit) exposed by a new `src/http/history.rs` keyed by `collection`+`id`. The frontend adds a Pinia history store, two components under `web/src/modules/content/history/`, and wires them into `ContentPanel.vue`'s right aside. The diff is built client-side with `parseDiffFromFile` from `@pierre/diffs` and rendered by the existing `checkpoints/DiffView.vue`.

**Tech Stack:** Rust (axum, tokio, git CLI), Vue 3 + Pinia, `@pierre/diffs`, Vitest.

**Spec:** `docs/superpowers/specs/2026-10-13-content-history-design.md`

## Global Constraints

- Git is invoked only through `git::run(dir, args)`; no `git2`.
- Client never supplies a file path. Paths come from `catalog::read_entry(root, collection, id).path` or from `file_log` output.
- SHAs are validated by the existing `git::resolve()` (4–40 hex, reachable from HEAD).
- Published = reachable from local `main`. If `main` doesn't exist, all `false`.
- Badges, in order of precedence: index 0 → `Current`; `published` → `Published`; else `Draft`.
- Diff direction: old = version source, new = `store.entry.source` (saved file). Never the draft.
- Restore never writes; it fills `draftFields`/`draftBody` only.
- Copy: tab labels `Fields`, `History`; buttons `Back to editor`, `Restore this version`; empty `No history yet`; unsaved notice `Comparing with the last saved version. Unsaved edits aren't included.`; unparseable `This version can't be restored because it couldn't be parsed.`
- Run Rust tests with `cargo test --test content_history`; web tests from `web/` with `npm test -- <file>`.

## Review Focus

1. **Entry renamed between versions** — older versions must be read from their old path, not the current one. Test in Task 1 (`file_at` with `version.path`) and Task 2 (endpoint returns old content after rename).
2. **HEAD moved under a selected version** (Get latest, branch switch) — version fetch 404s; store reloads list and clears selection instead of showing a stuck error. Test in Task 3.
3. **Switching to another entry while a version is selected** — the diff must not stay up showing the previous entry's history. Test in Task 5.
4. **Restore over unsaved edits** — silently discarding the writer's typing is surprising; Restore asks for confirmation via `AlertDialog` when the draft differs from the saved entry. Test in Task 5.
5. **SHA that exists but didn't touch this entry** — must 404, not leak another file. Test in Task 2.

Known limitation (not fixed): squash-merged PRs leave branch commits non-ancestors of `main`, so those show as `Draft` until the branch is cleaned up.

---

### Task 1: Per-file git log and file-at-commit

**Files:**
- Modify: `src/git.rs` (add after `diff`, ~line 430)
- Create: `tests/content_history.rs`

**Interfaces:**
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize)] pub struct FileVersion { pub sha: String, pub summary: String, pub author: String, pub ts: i64, pub path: String, pub published: bool }`
  - `pub async fn file_log(dir: &Path, path: &str) -> Result<Vec<FileVersion>, ApiError>` — newest first.
  - `pub async fn file_at(dir: &Path, sha: &str, path: &str) -> Result<String, ApiError>`

- [ ] **Step 1: Write failing tests** in `tests/content_history.rs`. Copy the local helpers `config`, `state`, `git_cli`, `project` from `tests/checkpoints.rs`; use `HERO = "src/content/sections/hero.mdx"`.

```rust
#[tokio::test]
async fn file_log_lists_commits_touching_path_newest_first() {
    // project(); on branch content/x: edit HERO, git::checkpoint "one"; edit other file, checkpoint "other"; edit HERO, checkpoint "two"
    // let log = git::file_log(&dir, HERO).await.unwrap();
    // assert_eq!(log[0].summary, "two"); assert_eq!(log[1].summary, "one");
    // assert!(log.iter().all(|v| v.summary != "other"));
    // assert!(log.iter().all(|v| v.path == HERO));
}

#[tokio::test]
async fn file_log_marks_main_commits_published() {
    // branch commits from content/x => published == false; the scaffold commit (on main) => published == true
}

#[tokio::test]
async fn file_log_follows_renames_and_file_at_reads_old_path() {
    // checkpoint edit of HERO ("before"), `git mv` HERO -> src/content/sections/hero2.mdx, checkpoint
    // let log = git::file_log(&dir, "src/content/sections/hero2.mdx").await.unwrap();
    // let old = log.iter().find(|v| v.summary == "before").unwrap();
    // assert_eq!(old.path, HERO);
    // assert!(git::file_at(&dir, &old.sha, &old.path).await.unwrap().contains(<text written in "before">));
}

#[tokio::test]
async fn file_log_is_empty_for_untracked_file() { /* write new file, no commit => vec![] */ }

#[tokio::test]
async fn file_at_rejects_non_hex_sha() { /* git::file_at(&dir, "HEAD~1", HERO) => Err, status 400 */ }
```

- [ ] **Step 2:** `cargo test --test content_history` → FAIL (`file_log` not found).
- [ ] **Step 3: Implement.**
  - `file_log`: unborn HEAD → `Ok(vec![])` (same guard as `log`). Run `log --follow --name-only --format=%x1e%H%x1f%ct%x1f%an%x1f%s -- <path>`; split records on `\x1e`; first line is the header (parse like `log`, stripping `CHECKPOINT_PREFIX`), the first non-empty following line is that commit's `path`.
  - Published: if `rev-parse --verify --quiet main` fails, all `false`. Otherwise run `rev-list HEAD ^main` once into a `HashSet<String>` of branch-only commits; `published = !set.contains(&sha)`. Rename-agnostic and one call.
  - `file_at`: `let full = resolve(dir, sha).await?;` then `run(dir, &["show", &format!("{full}:{path}")])`. Reject `path` starting with `-` or containing `..` with `ApiError::badreq`.
- [ ] **Step 4:** `cargo test --test content_history` → PASS.
- [ ] **Step 5: Commit** `git add src/git.rs tests/content_history.rs && git commit -m "feat(git): per-file log and file-at-commit"`

---

### Task 2: History HTTP endpoints

**Files:**
- Create: `src/http/history.rs`
- Modify: `src/http/mod.rs` (add `pub mod history;` and two routes next to `/api/content/entry`)
- Test: `tests/content_history.rs`

**Interfaces:**
- Consumes: `git::file_log`, `git::file_at`, `catalog::read_entry`, `catalog::build_entry(collection, id, path, kind, source, branch) -> ContentEntry`, `ContentQuery { project, collection: Option<String>, id: Option<String> }` from `src/http/content.rs` (make it `pub(crate)` if needed), `query_or_badreq`, `project_dir` as used in `src/http/checkpoints.rs:50`.
- Produces:
  - `GET /api/content/history?project=&collection=&id=` → `200 { "versions": FileVersion[] }`
  - `GET /api/content/history/{sha}?project=&collection=&id=` → `200 { "sha", "path", "source", "fields", "body", "parseable": bool }`; `404` when `sha` (prefix-matched against full SHAs) isn't in that entry's `file_log`.

- [ ] **Step 1: Write failing tests** (copy `req`, `call` helpers from `tests/checkpoints.rs`).

```rust
#[tokio::test]
async fn history_endpoint_lists_versions() {
    // checkpoint an edit to sections/hero; GET /api/content/history?project=..&collection=sections&id=hero
    // assert 200; body["versions"][0]["summary"] == <summary>; has "published" key
}
#[tokio::test]
async fn version_endpoint_returns_parsed_old_content() {
    // edit hero title to "Old", checkpoint; edit to "New", checkpoint
    // GET /api/content/history/{sha_of_old}?...  => body["fields"]["title"] == "Old"; body["parseable"] == true; body["source"] contains "Old"
}
#[tokio::test]
async fn version_endpoint_404s_for_commit_not_touching_entry() {
    // checkpoint a change to a different file; GET with that sha for hero => 404
}
#[tokio::test]
async fn version_endpoint_returns_old_content_after_rename() { /* rename entry via POST /api/content/entry/rename, fetch pre-rename version => old content */ }
#[tokio::test]
async fn history_requires_collection_and_id() { /* missing id => 400 */ }
```

- [ ] **Step 2:** `cargo test --test content_history` → new tests FAIL (404 route).
- [ ] **Step 3: Implement** `pub async fn list(State<AppState>, Result<Query<ContentQuery>, QueryRejection>) -> Result<Json<Value>, ApiError>` and `pub async fn version(State<AppState>, Path<String>, Result<Query<ContentQuery>, QueryRejection>) -> Result<Json<Value>, ApiError>`. `parseable` = `kind == "json"` ? `!fields.is_null()` : frontmatter parsed to an object.
- [ ] **Step 4:** `cargo test --test content_history` → PASS; `cargo test` → no regressions.
- [ ] **Step 5: Commit** `feat(http): content history endpoints`

---

### Task 3: API client + history store

**Files:**
- Modify: `web/src/core/api.ts` (next to `contentEntry`, line ~168), `web/src/core/types.ts`
- Create: `web/src/modules/content/history/store.ts`, `web/src/modules/content/history/store.test.ts`

**Interfaces:**
- Produces (types):
  - `interface FileVersion { sha: string; summary: string; author: string; ts: number; path: string; published: boolean }`
  - `interface ContentVersion { sha: string; path: string; source: string; fields: Record<string, unknown>; body: string; parseable: boolean }`
- Produces (api): `contentHistory(project, collection, id): Promise<FileVersion[]>`, `contentVersion(project, collection, id, sha): Promise<ContentVersion>`
- Produces (store `defineStore("contentHistory")`):
  - state: `key: string | null` (`"collection/id"`), `versions: FileVersion[]`, `loading`, `error: string | null`, `selectedSha: string | null`, `version: ContentVersion | null`, `versionLoading`
  - actions: `open(project, collection, id)` (sets key, clears selection, loads), `reload()`, `select(sha | null)`, `close()`
  - Subscribes via `useDaemon().on("checkpoint.created", …)` → `reload()` when `key` set (event has no paths, so reload unconditionally for the project); also `onReconnected(reload)`.

- [ ] **Step 1: Write failing tests** using `createFakeDaemon` from `@/test/fakeDaemon` and `vi.mock("@/core/daemon", …)` as in `ContentPanel.test.ts`.
  - `open() loads versions for the entry` → `versions.length === 2`, api called with `("p","blog","post")`.
  - `select() fetches the version` → `version.source === "old"`.
  - `checkpoint.created reloads` → after `daemon.emit("checkpoint.created", {...})`, `contentHistory` called twice.
  - `select() 404 reloads and clears selection` → api `contentVersion` rejects with status 404 → `selectedSha === null`, `contentHistory` called again, `error === null`.
  - `open() of another entry clears selection` → `selectedSha === null`, `version === null`.
- [ ] **Step 2:** `npm test -- history/store` → FAIL.
- [ ] **Step 3: Implement** (mirror `checkpoints/store.ts` structure).
- [ ] **Step 4:** PASS.
- [ ] **Step 5: Commit** `feat(web): content history api and store`

---

### Task 4: VersionsList and VersionDiff components

**Files:**
- Create: `web/src/modules/content/history/VersionsList.vue`, `VersionDiff.vue`, `history.ts`, `history.test.ts`, `VersionsList.test.ts`

**Interfaces:**
- Produces:
  - `history.ts`: `versionBadge(index: number, v: FileVersion): "Current" | "Published" | "Draft"`; `versionTime(ts: number, now?: Date): string` → `"Today at 6:36 PM"`, `"Yesterday at 6:36 PM"`, else `"Oct 3 at 6:36 PM"` (`Intl.DateTimeFormat`, `hour: "numeric", minute: "2-digit"`); `versionDiff(path: string, oldSource: string, newSource: string): FileDiffMetadata` via `parseDiffFromFile({name: path, contents: oldSource}, {name: path, contents: newSource})`.
  - `VersionsList.vue` props `{ versions: FileVersion[]; selectedSha: string | null; loading: boolean; error: string | null }`, emits `select(sha: string)`, `retry()`. `data-testid="history-version"` per row, `aria-pressed` on the selected row.
  - `VersionDiff.vue` props `{ path: string; oldSource: string; newSource: string; dirty: boolean; canRestore: boolean; restoreReason?: string }`, emits `back()`, `restore()`. Renders `DiffView` lazy-loaded with `defineAsyncComponent` as at `CheckpointsPanel.vue:30`. Shows "No differences" when sources are equal.

- [ ] **Step 1: Write failing tests.**
  - `versionBadge`: `(0, {published:true})` → `"Current"`; `(1, {published:true})` → `"Published"`; `(1, {published:false})` → `"Draft"`.
  - `versionTime`: fixed `now = new Date(2026,9,13,12)`; ts of `2026-10-12 18:36` local → `"Yesterday at 6:36 PM"`.
  - `versionDiff("a.md","x\n","y\n").hunks.length === 1`.
  - `VersionsList`: renders 3 rows with badges `Current, Published, Draft`; clicking row 2 emits `select` with its sha; empty `versions` + not loading shows `No history yet`; `error` shows Retry which emits `retry`.
- [ ] **Step 2:** `npm test -- content/history` → FAIL.
- [ ] **Step 3: Implement.** Row style per spec screenshot: hollow circle dot (filled when selected or Current), time on the first line, `author · summary` muted below, `Badge` right-aligned (`secondary` for Current/Draft, a green variant for Published — reuse existing badge variant if one exists, else `class` override).
- [ ] **Step 4:** PASS.
- [ ] **Step 5: Commit** `feat(web): versions list and version diff components`

---

### Task 5: Wire History into ContentPanel

**Files:**
- Modify: `web/src/modules/content/ContentPanel.vue` (aside ~line 273, body ~line 278, `hasFieldsPanel` line 73)
- Modify: `web/src/modules/content/store.ts` (add `restoreVersion`)
- Test: `web/src/modules/content/ContentHistory.test.ts` (new; same setup as `ContentPanel.test.ts`)

**Interfaces:**
- Consumes: Task 3 store, Task 4 components, `store.entry`, `store.draftFields`, `store.draftBody`, `store.setBody`, `diffDraft`, `cloneValue` from `draft.ts`.
- Produces: content store action `restoreVersion(v: ContentVersion): void` — `draftFields = cloneValue(v.fields)`, `setBody(v.body)` (body only for non-JSON).

- [ ] **Step 1: Write failing tests.**
  - `History tab lists versions` → click `History` tab, 2 `history-version` rows.
  - `selecting a version swaps body for diff` → MarkdownBodyEditor not rendered, `Back to editor` visible; clicking it restores the editor.
  - `restore fills the draft and returns to editor` → after `Restore this version`, title input value === old title, Save enabled (draft changed), no PUT sent.
  - `restore over unsaved edits asks first` → type in title, Restore → confirm dialog shown; Cancel keeps typed value.
  - `navigating to another entry clears the selected version` → select version, route to another entry → body editor visible, no diff.
  - `unsaved edits show notice` → type, select version → notice text present.
  - `JSON entry shows History tab` → `site` entry has the aside with History only.
- [ ] **Step 2:** `npm test -- ContentHistory` → FAIL.
- [ ] **Step 3: Implement.**
  - Aside shown when `hasFieldsPanel || store.entry` (JSON gets History only). Use `components/ui/tabs`; local `tab = ref<"fields" | "history">("fields")`, default `history` when no fields panel.
  - On entry change (`watch` on `collection/id`): `history.open(...)` if tab is history, else lazily on first switch to history; always `history.select(null)`.
  - Body column: `v-if="history.version"` render `VersionDiff` with `newSource = store.entry.source`, `dirty = diffDraft(...).changed`, `canRestore = history.version.parseable`.
  - Restore: if dirty, open `AlertDialog` ("Replace unsaved edits?"); on confirm `store.restoreVersion(history.version)`, `history.select(null)`.
  - Esc while a version is selected → `history.select(null)`.
- [ ] **Step 4:** `npm test` (whole web suite) → PASS; `npm run typecheck` if present.
- [ ] **Step 5: Manual check** in the running app: open an entry, save twice, History shows Current + Draft; switch to `main` after merge shows Published.
- [ ] **Step 6: Commit** `feat(content): per-entry history tab with diff and restore`
