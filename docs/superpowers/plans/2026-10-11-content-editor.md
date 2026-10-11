# Phase 3 Content Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let writers browse and edit the bundled Astro template's content on change branches, then preview Blog and Help docs before merging to `main`.

**Architecture:** The template owns its collection definitions and a read-only Node schema helper. Rust owns project identity, safe file mutation, concurrency checks, and scoped checkpoints; the Vue Content tab owns draft state and presentation. Ship the template and browsing, structured editing, then Markdown and images as three usable slices.

**Tech Stack:** Astro 7.3.5, `@astrojs/mdx` 8.0.2, Zod through Astro, Rust/Axum, Vue 3/Vitest, Tiptap 3 with official `@tiptap/markdown`, CodeMirror, Playwright.

**Spec:** [2026-10-11-content-editor-design.md](../specs/2026-10-11-content-editor-design.md)

## Global Constraints

- Scope is the bundled template's `site`, `pages`, `sections`, `blog`, and `helpDocs` collections; unsupported collections remain read-only with a reason.
- Blog files live at `src/content/blog/en/<slug>.md`; Help docs files live at `src/content/helpDocs/en/<slug>.md`. No locale selector or draft frontmatter field in Phase 3.
- Content mutations require an active change branch for GitHub and local-only projects. Production content updates when the branch merges to `main`.
- `site` is update-only; `pages/home` cannot be renamed or deleted. Reserve `blog` and `help` as page slugs.
- Accept PNG, JPEG, WebP, and GIF images up to 10 MiB each; require nonempty alt text. Section MDX stays raw.
- Save is explicit. Unchanged saves make no file write or checkpoint. Mutations check the opened branch and content hash, and checkpoints include only paths owned by the mutation.
- Preserve unrelated Markdown frontmatter and JSON source formatting; failed validation or ordinary I/O leaves no partial entry or asset update.
- Do not alter the existing Preview, Code, or GitHub publication flows except to refresh them after content events.

## Review Focus

1. A symlink inside `src/content` or `src/assets` points outside the project: mutation rejects it without writing outside the project (Task 5).
2. A writer opens an entry, changes branches, then saves: conflict retains the draft and leaves both branches untouched (Tasks 5 and 6).
3. A content save occurs beside unrelated staged or working-tree changes: its checkpoint includes only its own paths and leaves the other changes as they were (Task 5).
4. A Markdown body contains MDX/HTML or another unsupported construct: it opens raw and a toggle cannot silently discard it (Task 8).
5. The browser labels an SVG or arbitrary bytes as PNG: server rejects the upload and rolls back the entry and staged assets (Task 9).

---

### Task 1: Template collections and public routes

**Files:** Modify `templates/astro-marketing/src/content.config.ts`, `src/layouts/Base.astro`, `src/lib/href.ts`, `src/pages/index.astro`, `src/content/site.json`, `package.json`; create `src/content-schemas.mjs`, `scripts/workbench-content.mjs`, `src/pages/[slug].astro`, `src/pages/blog/index.astro`, `src/pages/blog/[slug].astro`, `src/pages/help/index.astro`, `src/pages/help/[slug].astro`, representative `src/content/blog/en/welcome.md`, `src/content/helpDocs/en/getting-started.md`, `scripts/content-pages.test.mjs`; modify `tests/static.rs`.

**Interfaces:** Consumes Astro's `getCollection`, `getEntry`, `render`, and existing `Base.astro`. Produces `blog` and `helpDocs` collection exports and the five public routes, using base-aware links. `src/content-schemas.mjs` exports `collectionRegistry` and `createCollectionSchemas(context)`; `scripts/workbench-content.mjs` initially supports `{op:"list"}` on stdin and returns registry names, loader paths, and content kinds as JSON on stdout.

- [ ] **Step 1: Write failing tests.** In `scripts/content-pages.test.mjs`, assert the `list` helper reports all five collection names and loader paths; built output has blog/help indexes and sample articles, Blog newest-first ordering, Help section/order grouping, non-home page rendering, empty-state copy when fixtures are absent, and working base-prefixed links. In `tests/static.rs`, assert new template files are embedded.
- [ ] **Step 2: Run failure.** Run `npm run build && node --test scripts/content-pages.test.mjs` in the template and `cargo test --test static`; expect missing collections/routes or output.
- [ ] **Step 3: Implement template.** Move existing schemas into `createCollectionSchemas(context)` and keep `content.config.ts` as the Astro adapter. Define Blog fields exactly as `title`, `description`, `pubDate` (`YYYY-MM-DD`), optional `coverImage` plus conditional `coverAlt`, `tags` default `[]`, and `relatedPosts` default `[]`; Help fields as `title`, `description`, `section`, numeric `order`. Export loader metadata from the same module and expose its `list` operation. Add `glob()` loaders rooted in `en/`; render Markdown through Astro content APIs. Make dynamic `pages` exclude `home`, `blog`, and `help`, and add navigation links.
- [ ] **Step 4: Verify.** Run the two commands from Step 2; expect all tests and Astro build to pass.
- [ ] **Step 5: Commit.** Stage only Task 1 paths and commit `feat: add blog and help template pages`.

### Task 2: Read-only content catalog and Content tab

**Files:** Create `src/content/catalog.rs`, `src/content/mod.rs`, `src/http/content.rs`, `tests/content_catalog.rs`, `web/src/modules/content/store.ts`, `web/src/modules/content/ContentPanel.vue`, `web/src/modules/content/ContentPanel.test.ts`; modify `src/http/mod.rs`, `web/src/router.ts`, `web/src/app/EditorMenubar.vue`, `web/src/core/api.ts`, `web/src/core/types.ts`.

**Interfaces:** Consumes Task 1's `{op:"list"}` helper. Produces `GET /api/content/collections?project=<id>`, `GET /api/content/entries?project=<id>&collection=<name>`, and `GET /api/content/entry?project=<id>&collection=<name>&id=<id>`. `ContentEntry` contains `collection`, `id`, `path`, `kind` (`json|markdown|mdx`), `source`, `fields`, `body`, `branch`, and SHA-256 `hash`. `ContentCollection` contains `fields` descriptors after Task 3, plus `editable` and `reason`; unsupported loaders return `editable:false`.

- [ ] **Step 1: Write failing tests.** `tests/content_catalog.rs` asserts the five template collections and entry paths, English-only Blog/Help IDs, raw source and hash, project isolation, and unsupported project read-only reason. `ContentPanel.test.ts` asserts list/detail navigation, main-branch read access, and dirty-draft preservation when a filesystem refresh arrives.
- [ ] **Step 2: Run failure.** Run `cargo test --test content_catalog` and `cd web && npm test -- --run src/modules/content/ContentPanel.test.ts`; expect absent API/tab.
- [ ] **Step 3: Implement.** Add `catalog::list_collections(project_root: &Path) -> Result<Vec<ContentCollection>, ApiError>` and `catalog::read_entry(project_root: &Path, collection: &str, id: &str) -> Result<ContentEntry, ApiError>`. Read collection paths from Task 1's `list` operation, resolve only its supported loaders and safe project paths, and keep raw entries visible when the helper cannot run. Add `content` route/tab, API methods, selection and draft state, and an unsaved-navigation prompt.
- [ ] **Step 4: Verify.** Run Step 2 commands plus `cd web && npm run build`; expect pass.
- [ ] **Step 5: Commit.** Stage only Task 2 paths and commit `feat: browse template content`.

### Task 3: Shared schema helper and source-preserving patches

**Files:** Create `templates/astro-marketing/scripts/workbench-content.test.mjs`; modify `templates/astro-marketing/src/content-schemas.mjs`, `templates/astro-marketing/scripts/workbench-content.mjs`, `templates/astro-marketing/src/content.config.ts`, `templates/astro-marketing/package.json` and lockfile if dependency changes are needed; modify `src/content/catalog.rs`, `tests/content_catalog.rs`.

**Interfaces:** Extend Task 1's line-delimited JSON helper with `{op:"describe"}` returning collection/field descriptors, `{op:"validate",collection,fields}` returning `{valid,fields,errors:[{path,message}]}`, and `{op:"patch",collection,source,fields,body}` returning `{source}`. Every schema definition remains shared with Astro; helper never writes files. Descriptors represent strings/dates/enums/literals/booleans/numbers/arrays/objects/discriminated section variants/images/references or `unsupported` with reason.

- [ ] **Step 1: Write failing tests.** Assert helper registry names/loader paths equal `content.config.ts`; valid and invalid Blog/Help samples agree with Astro build validation; `coverAlt` is required with `coverImage`; unknown transforms are read-only; patching one YAML or JSON field preserves untouched comments, key text, quote style, ordering, indentation, line endings, and body bytes. Extend `content_catalog` to assert collection responses include field descriptors.
- [ ] **Step 2: Run failure.** Run `cd templates/astro-marketing && node --test scripts/workbench-content.test.mjs`; expect missing describe/validate/patch operations.
- [ ] **Step 3: Implement.** Extend the shared schema factory with image/reference metadata and corresponding helper adapters, with contract tests for equivalent validation. Implement `describe`, `validate`, and syntax-aware targeted `patch` without normalizing untouched source. Return field paths from live Zod issues; label unrepresentable required fields unsupported. Have `catalog::list_collections` attach `describe` results and mark required unsupported fields read-only.
- [ ] **Step 4: Verify.** Run Step 2, `cargo test --test content_catalog`, and `cd templates/astro-marketing && npm run build`; expect pass.
- [ ] **Step 5: Commit.** Stage only Task 3 paths and commit `feat: share template schemas with content helper`.

### Task 4: Scoped checkpoints

**Files:** Modify `src/git.rs`; create `tests/content_checkpoint.rs`.

**Interfaces:** Produce `git::checkpoint_paths(dir: &Path, summary: &str, paths: &[PathBuf]) -> Result<Option<Checkpoint>, ApiError>`. Paths are project-relative, validated, and limited to the content mutation; existing `checkpoint` behavior remains available to its callers.

- [ ] **Step 1: Write failing tests.** Assert a content file and an unrelated staged file yield a checkpoint containing only the content file; the unrelated staged index entry remains staged. Assert unrelated unstaged edits remain unstaged; no changed paths yields `None`; a path outside the project is rejected.
- [ ] **Step 2: Run failure.** Run `cargo test --test content_checkpoint`; expect missing function.
- [ ] **Step 3: Implement.** Add `checkpoint_paths` using a temporary index or equivalent isolated index strategy so commit contents and the user's index state are independent. Preserve path deletion handling and checkpoint metadata.
- [ ] **Step 4: Verify.** Run Step 2 and `cargo test --test checkpoints`; expect pass.
- [ ] **Step 5: Commit.** Stage only Task 4 paths and commit `feat: checkpoint selected content paths`.

### Task 5: Safe create and update API

**Files:** Create `src/content/helper.rs`, `src/content/mutation.rs`, `tests/content_mutation.rs`; modify `src/content/mod.rs`, `src/http/content.rs`, `src/http/mod.rs`, `src/error.rs`, `src/state.rs` as needed for per-project serialization.

**Interfaces:** Produce `POST /api/content/entry` with `{project,collection,slug,branch,fields,body}` and `PUT /api/content/entry` with `{project,collection,id,branch,hash,fields,body}`; return `ContentEntry`. `content::mutation::save_entry(state: &AppState, request: SaveEntry) -> Result<ContentEntry, ApiError>` calls Task 3's helper and Task 4's checkpoint. Conflict codes distinguish `branch_changed`, `stale_content`, `duplicate_slug`, `invalid_schema`, `unsupported_collection`, and `unsafe_path`; validation errors include `{path,message}` details.

- [ ] **Step 1: Write failing tests.** Assert local-only and GitHub projects reject writes on `main`; create/update succeed on active changes; stale hash and changed branch reject without writes; duplicate or invalid slug rejects; schema errors contain field paths; symlink escapes reject; unchanged save preserves mtime and creates no checkpoint; unrelated staged/unstaged files survive a successful save.
- [ ] **Step 2: Run failure.** Run `cargo test --test content_mutation`; expect absent routes/core.
- [ ] **Step 3: Implement.** Resolve only catalog-declared paths and safe parents, verify branch/hash immediately before mutation under a per-project lock, validate the complete candidate with helper, apply its targeted patch, write via same-directory temporary file and atomic rename, checkpoint changed paths, and publish existing filesystem/checkpoint events. Add a content-specific active-change guard rather than weakening the existing GitHub-only guard.
- [ ] **Step 4: Verify.** Run Step 2 and `cargo test --test change_guard`; expect pass.
- [ ] **Step 5: Commit.** Stage only Task 5 paths and commit `feat: safely save structured content`.

### Task 6: Schema-driven forms and draft recovery

**Files:** Create `web/src/modules/content/FrontmatterForm.vue`, `web/src/modules/content/FrontmatterForm.test.ts`, `web/src/modules/content/draft.test.ts`; modify `ContentPanel.vue`, `store.ts`, `web/src/core/api.ts`, `web/src/core/types.ts`.

**Interfaces:** Consume Task 3 field descriptors through content read API and Task 5 save routes. Produce `saveDraft(): Promise<ContentEntry>` and form controls for each supported descriptor; unsupported fields render read-only with a reason.

- [ ] **Step 1: Write failing tests.** Assert string/date/enum/boolean/number/array/nested/discriminated/reference/image controls map to expected values; changing one field saves only on explicit Save; no-op Save does not call mutation; invalid fields show helper paths; branch/hash conflict retains input and offers reload/compare; main-branch Save offers Create Change and resumes the pending save after creation.
- [ ] **Step 2: Run failure.** Run `cd web && npm test -- --run src/modules/content/FrontmatterForm.test.ts src/modules/content/draft.test.ts`; expect missing controls/action.
- [ ] **Step 3: Implement.** Wire `FrontmatterForm` recursively to descriptors, separate loaded value from draft, pass original branch/hash, and preserve draft through API errors, refresh, and branch switch. Use existing Changes picker/create action for the main-branch flow; show read-only reasons from the API.
- [ ] **Step 4: Verify.** Run Step 2 and `cd web && npm run build`; expect pass.
- [ ] **Step 5: Commit.** Stage only Task 6 paths and commit `feat: edit content schema fields`.

### Task 7: Rename and delete with references

**Files:** Modify `src/content/mutation.rs`, `src/http/content.rs`, `web/src/modules/content/ContentPanel.vue`, `web/src/modules/content/store.ts`; create `tests/content_lifecycle.rs`, `web/src/modules/content/lifecycle.test.ts`.

**Interfaces:** Produce `POST /api/content/entry/rename` with `{project,collection,id,slug,branch,hash}` and `DELETE /api/content/entry` with `{project,collection,id,branch,hash}`. A dry-run `POST /api/content/entry/impact` returns `{references,assets,proseLinks}` for the confirmation dialog. Successful rename/delete returns updated catalog state.

- [ ] **Step 1: Write failing tests.** Assert `site` cannot create/rename/delete, `pages/home` cannot rename/delete, section creation remains invisible until its page lists the ID, rename rewrites `pages.sections`/`relatedPosts` references and owned image paths while reporting prose links, delete refuses referenced entries, and rename/delete make one reversible scoped checkpoint. UI tests assert impact before confirmation and draft retention on conflict.
- [ ] **Step 2: Run failure.** Run `cargo test --test content_lifecycle` and `cd web && npm test -- --run src/modules/content/lifecycle.test.ts`; expect absent operations.
- [ ] **Step 3: Implement.** Add `mutation::inspect_impact(project_root: &Path, collection: &str, id: &str) -> Result<ContentImpact, ApiError>`, `rename_entry(state: &AppState, request: RenameEntry) -> Result<ContentEntry, ApiError>`, and `delete_entry(state: &AppState, request: DeleteEntry) -> Result<(), ApiError>`; apply the same branch/hash, safe path, lock, and helper validation rules as save. Move the owned asset directory on rename, remove it on delete, and include every affected reference file in the scoped checkpoint.
- [ ] **Step 4: Verify.** Run Step 2 commands; expect pass.
- [ ] **Step 5: Commit.** Stage only Task 7 paths and commit `feat: rename and delete content entries`.

### Task 8: Markdown and MDX body editing

**Files:** Modify `web/package.json` and lockfile; create `web/src/modules/content/MarkdownBodyEditor.vue`, `web/src/modules/content/MarkdownBodyEditor.test.ts`, `web/src/modules/content/markdown.ts`; modify `ContentPanel.vue`, `store.ts`.

**Interfaces:** Consume/produce the `body` string in Task 5's save request. `markdown::canRoundTrip(source: string): boolean` gates rich mode; body editor emits `{body,dirty}` without reserializing a merely opened or toggled document.

- [ ] **Step 1: Write failing tests.** Assert Blog/Help toolbar and slash actions for heading/list/link/quote/fenced code/image and inline emphasis; rich-to-raw-to-rich keeps supported content; unchanged open/toggle keeps exact original bytes; unsupported HTML/MDX opens raw and cannot enter rich mode destructively; `sections` MDX is raw-only.
- [ ] **Step 2: Run failure.** Run `cd web && npm test -- --run src/modules/content/MarkdownBodyEditor.test.ts`; expect missing editor.
- [ ] **Step 3: Implement.** Add official `@tiptap/markdown` alongside installed Tiptap extensions, configure the supported node set and slash commands, and use CodeMirror for raw Markdown/MDX. Compare the parsed/serialized representation before allowing rich mode, retain original source until a body edit, and connect explicit Save.
- [ ] **Step 4: Verify.** Run Step 2 and `cd web && npm run build`; expect pass.
- [ ] **Step 5: Commit.** Stage only Task 8 paths and commit `feat: author markdown content`.

### Task 9: Transactional image save and image input

**Files:** Create `src/content/images.rs`, `tests/content_images.rs`, `web/src/modules/content/ImageDialog.vue`, `web/src/modules/content/ImageDialog.test.ts`; modify `src/content/mutation.rs`, `src/http/content.rs`, `web/src/modules/content/MarkdownBodyEditor.vue`, `FrontmatterForm.vue`, `store.ts`, `web/src/core/api.ts`.

**Interfaces:** Extend create/update to `multipart/form-data` with one JSON `entry` part and zero or more image parts keyed by client IDs; each image has `{clientId,alt,filename}` metadata. `images::validate_upload(bytes: &[u8]) -> Result<ImageFormat, ApiError>` inspects signatures/decodes bytes; `save_entry` resolves client IDs to project-relative `src/assets/<collection>/<slug>/` paths and returns the saved entry.

- [ ] **Step 1: Write failing tests.** Rust tests assert real PNG/JPEG/WebP/GIF <=10 MiB accepted, mislabeled SVG/arbitrary bytes and >10 MiB rejected, deterministic collision naming, correct relative links, rollback on validation/I/O failure, and orphan staging cleanup on next operation. UI tests assert paste/drop/file picker open one preview/alt dialog, cannot insert with blank alt, hold local previews until Save, and retain pending images after failure.
- [ ] **Step 2: Run failure.** Run `cargo test --test content_images` and `cd web && npm test -- --run src/modules/content/ImageDialog.test.ts`; expect absent flow.
- [ ] **Step 3: Implement.** Add multipart decoding and body limits, byte inspection, staging within the project, entry validation before final placement, rollback of ordinary failures, and scoped checkpoint of entry plus images. Insert escaped Markdown syntax or fill image/alt companion fields; keep pending blobs in draft state until success.
- [ ] **Step 4: Verify.** Run Step 2 commands and `cd templates/astro-marketing && npm run build`; expect pass.
- [ ] **Step 5: Commit.** Stage only Task 9 paths and commit `feat: attach images to content saves`.

### Task 10: Browser acceptance flow

**Files:** Create `e2e/specs/content-editor.spec.ts`; modify `e2e/fixtures/workbench.ts`, `web/src/modules/content/store.ts`, and `web/src/modules/preview/store.ts` only where the failing flow identifies a missing fixture or refresh connection.

**Interfaces:** Exercises Tasks 1–9 through public UI/API and template Preview; no new product API.

- [ ] **Step 1: Write failing test.** In Playwright, create a local template project, create a change, create and save a Blog article with an image, see it in Content and Preview at `/blog/<slug>`, verify Help index, switch to `main` and confirm the article is absent there, then switch back and confirm the draft remains on the change branch.
- [ ] **Step 2: Run failure.** Run `cd e2e && npm run e2e -- --grep 'content editor'`; expect missing flow or route.
- [ ] **Step 3: Implement `refreshForBranch(branch: string): Promise<void>` in `web/src/modules/content/store.ts` and connect the existing branch-change event to Content and Preview reloads.** Add the template-project fixture in `e2e/fixtures/workbench.ts` if the existing fixture cannot create one. Keep the current draft object when the branch changes.
- [ ] **Step 4: Verify.** Run the focused Playwright test, template build, `cargo test --test content_catalog --test content_mutation --test content_lifecycle --test content_images`, and `cd web && npm run build`; expect pass.
- [ ] **Step 5: Commit.** Stage only Task 10 test and integration fixes and commit `test: cover content editor branch preview`.
