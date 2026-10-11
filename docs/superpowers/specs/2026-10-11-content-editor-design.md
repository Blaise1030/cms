# Phase 3 Content editor (design)

Status: **draft 2026-10-11**, awaiting review.
Tracker: [issue #22](https://github.com/Blaise1030/cms/issues/22).

## Intent

Give writers a Content view for editing the bundled Astro marketing template without editing files by hand. They can browse collections, edit structured data and Markdown articles, add images, and preview changes on a local change branch. Production receives the content when that branch merges into `main`. There is no draft flag in entry frontmatter: the branch is the draft boundary.

This phase is **template-first**. It covers the template's existing `site`, `pages`, and `sections` collections and adds `blog` and `helpDocs`. Other Astro projects can still use Code and Preview; the Content view explains when it cannot safely edit their collection schemas. Full arbitrary Astro schema support is outside this phase.

## Success criteria

1. Content lists the five collections and their entries from the template's collection registry. Multi-entry collections support create, rename, and delete on a change branch; the singleton `site` and required `pages/home` entry are protected from rename and delete.
2. Structured forms reflect the collection schemas. The save path validates against the same Zod schemas Astro uses and shows field-specific errors.
3. Saving an unchanged entry writes nothing and produces no git diff. Editing frontmatter preserves unrelated keys, comments, ordering, quoting, and body text. JSON edits preserve untouched key text, ordering, indentation, and line endings.
4. Blog and Help docs `.md` bodies have a Tiptap editor with toolbar, slash commands, and a synchronized Raw Markdown view. Unsupported Markdown is kept in Raw view rather than silently lost. Existing `sections` `.mdx` bodies use a raw editor.
5. Paste, drop, and file-picker image inputs require alt text, save under `src/assets/<collection>/<slug>/`, and insert a relative image link or image-field value. A failed save leaves no new asset or changed entry.
6. The template serves `/blog`, `/blog/<slug>`, `/help`, and `/help/<slug>` with navigation links. Workbench preview shows branch content; the production deployment remains driven by merges to `main`.

## Decisions

| Area | Decision |
| --- | --- |
| Scope | The bundled template's five collections only. Unknown or unsupported collections are visible where discoverable, but editing is disabled with a reason. |
| Drafts | No frontmatter status. An active change branch owns unfinished work. |
| Publication | Existing GitHub PR and merge flow publishes to production through the template's `main` deployment. Branch preview remains available for review. |
| Editing on `main` | Content mutations require an active change, including local-only projects. Reading and previewing are allowed on `main`. |
| Markdown | Rich editing is for Blog and Help docs `.md`; section MDX bodies stay raw. |
| Save | Explicit Save creates a scoped checkpoint in History. No-op Save creates no write or checkpoint. |
| Concurrency | Every mutation checks the branch and the original entry hash from when the editor loaded. Conflicts retain the unsaved UI state. |

## Template content model and pages

`src/content.config.ts` continues to export Astro's `collections` object. Its schema definitions move to an ESM module importable by both Astro and a template-local content helper. The helper enumerates the same collection registry and exposes the supported loader paths, field model, and validation; a contract test checks its collection names and paths against `content.config.ts`. Astro's schemas remain the validation source of truth. Editor-specific metadata is attached to schema fields only where needed to identify image and reference controls or provide labels. Unsupported schema nodes have a read-only representation.

Existing collections keep their content model:

- `site`: `src/content/site.json`, a single global settings entry.
- `pages`: `src/content/pages/*.json`, page metadata and ordered section IDs. `home` renders at `/`; other page IDs render at `/<slug>` through a dynamic route. `blog` and `help` are reserved page slugs.
- `sections`: `src/content/sections/*.mdx`, discriminated frontmatter for the existing section types and optional MDX body.

New collections use `glob()` with one Markdown file per entry, under `src/content/blog/en/<slug>.md` and `src/content/helpDocs/en/<slug>.md`. The Content view shows English only in Phase 3; the `en/` segment prepares for Phase 4 localization without appearing in public URLs. Slugs are lowercase, URL-safe, unique within a collection and locale, and cannot contain path separators or reserved path segments.

The Blog schema includes required `title`, `description`, and `pubDate` (`YYYY-MM-DD`); optional `coverImage` with required `coverAlt` when present; `tags` as an array of strings defaulting to empty; and `relatedPosts` as optional references defaulting to empty. The Help docs schema includes required `title`, `description`, and `section`, plus numeric `order`. The template adds index and article routes for both collections, adds links to the global navigation, and renders Markdown through Astro's content APIs. Blog indexes sort newest first; Help docs group by section and sort by order, then title. Empty indexes have a useful empty state. Only entries on the branch being previewed appear in that preview; production builds from `main`.

## Workbench architecture

### Content view

A `Content` workspace tab sits beside Preview, Code, and History. Its collection list and entry list come from the daemon's content API, not a walk of every project file in the browser. A selected entry opens a form and, for Markdown or MDX, a body editor. The view tracks dirty fields, shows explicit Save, and warns before navigation with unsaved edits. It refreshes after filesystem events while preserving an open dirty draft for conflict comparison.

The existing Changes picker remains the branch control. If a writer tries to mutate content on `main`, Content offers to create a change, then resumes the intended action. Branch switching and stale-file conflicts never discard an in-progress draft.

### Template helper and schema forms

The template-local Node helper loads the shared schema module in the project's installed Astro environment. It returns a small JSON field description for the supported Zod shapes: strings, dates, enums/literals, booleans, numbers, arrays, nested objects, discriminated section variants, images, and collection references. The same live schemas validate candidate values on save and return paths and messages suitable for field errors. Image and reference controls use schema metadata plus the collection registry to choose files or target entries. Unknown transforms and custom refinements that cannot be represented are shown read-only; creating an entry is disabled when a required unsupported field has no schema default. Unsupported loader shapes make the collection read-only with an explanation.

The Rust daemon owns project identity, change-branch checks, path containment, file size limits, per-project mutation serialization, content hashes, file writes, and checkpoints. It passes values to the helper for schema description and validation rather than duplicating Astro/Zod rules in Rust. The helper does not write project files. If the helper cannot run, Content may list raw entries but structured mutation is disabled with the actual error; Code and Preview remain usable.

### Content API and save flow

The daemon exposes collection and entry listing, entry read, create, update, rename, delete, and image upload as project-scoped operations. A read returns the entry text, parsed fields, file type, branch, and a content hash. Mutations require the branch and hash the client opened (create requires a unique target path instead). The daemon checks the active change, resolves paths within the project without following escaping symlinks, validates the candidate against the template helper, applies the mutation, and creates a checkpoint limited to the changed content paths. It emits the existing filesystem and checkpoint events so Files, Preview, and History refresh.

A no-op update returns success without touching the file. Conflict responses distinguish changed branch, stale content, duplicate slug, invalid schema, unsupported collection, and unsafe path. Save errors leave the editor's input intact and give a direct recovery action. The daemon must not stage or commit unrelated working-tree edits when it checkpoints a content mutation.

## Editing rules

For Markdown and MDX, the entry reader separates frontmatter from body without normalizing the source. Updating fields patches only changed frontmatter nodes, preserving untouched key text, comments, ordering, quoting, and line endings. Body text is copied byte-for-byte unless the body editor changed it. JSON entries use targeted edits so untouched properties and whitespace are preserved. Validation runs on the complete candidate data, including unchanged fields, before any write.

Blog and Help docs use Tiptap with the official `@tiptap/markdown` extension. Toolbar and slash commands cover headings, lists, links, quotes, fenced code, and images; the toolbar also offers common inline emphasis. The Raw Markdown toggle takes the current rich document to Markdown, and raw edits are parsed when returning to rich mode. The editor retains the original source and does not serialize it just because it was opened or toggled. Before rich editing, it checks whether the loaded body can be represented by the configured extensions; unsupported content opens in Raw view and cannot be silently round-tripped through rich mode. Section MDX bodies always use Raw view because MDX JSX/components are outside the Markdown editor's supported model.

Create requests a title and slug and produces a schema-valid starting entry. The singleton `site` is editable but cannot be created, renamed, or deleted; `pages/home` cannot be renamed or deleted. A new section is not visible on a page until the writer adds its ID to that page's `sections` field. Rename changes the entry file and its public URL when it has one, moves its dedicated asset directory, and rewrites that entry's owned relative image paths. It shows affected schema references, including `pages.sections`, and updates them as part of the confirmed rename; local Markdown hyperlinks to an old URL are reported for review rather than silently rewritten. Delete requires confirmation, deletes the entry and its dedicated asset directory, and refuses while schema references point to it. A checkpoint makes these operations reversible through History.

## Images

Paste, drop, and file picker all open one preview and alt-text dialog. The dialog does not insert an image until alt text is non-empty. The same picker can fill an image-valued frontmatter field and its alt-text companion. Uploads accept PNG, JPEG, WebP, and GIF up to 10 MiB per file, enforced by inspecting bytes on the daemon rather than trusting the browser MIME label or filename; SVG and non-image content are rejected. A collision gets a deterministic unique filename rather than overwriting an existing asset.

Before Save, pending images remain in the editor's draft state with local preview URLs. Save stages all new assets under `src/assets/<collection>/<slug>/`, computes paths relative to the entry file, inserts Markdown image syntax with the supplied alt text (or a frontmatter image path), and commits the entry plus assets as one content mutation. Validation or ordinary I/O failure rolls back staged files and leaves the original entry and assets unchanged. Process-crash recovery is handled by staging temporary files inside the project and cleaning orphan staging files on the next content operation; the API does not claim crash-atomic filesystem transactions.

## Failure handling and verification

- On a stale hash or branch mismatch, return a conflict and retain the writer's draft; offer reload and comparison with the current file. Do not auto-merge form or Markdown changes.
- On schema failure, return field paths and Astro-compatible messages; do not write any part of the candidate.
- On helper failure, expose the error and disable structured mutation for that project until refreshed.
- On file/path/upload failure, report the failed action, keep the draft, and leave no partial normal-operation save.
- On rename/delete, show the affected references and assets before confirmation. Refuse deletion of a referenced entry.

Verification includes Rust API tests for branch guards, path containment, hashes/conflicts, scoped checkpoints, rename/delete, and image rollback; helper tests comparing validation with Astro for representative valid and invalid entries; editor tests for form variants, no-diff saves, raw/rich synchronization, unsupported Markdown, and all three image entry points; and a browser flow that creates an article, saves it on a change branch, sees it in Preview, and confirms the generated template route renders. A template build verifies the Blog and Help docs routes and asset links.

## Delivery slices

1. **Template and browsing:** add Blog and Help docs collections/routes and a read-only Content tab for the five template collections. This produces navigable published pages and a useful content inventory.
2. **Structured entry editing:** shared schema helper, forms, safe create/update/rename/delete APIs, conflict checks, and checkpoints. This produces end-to-end editing of metadata and template settings.
3. **Markdown authoring and images:** Tiptap rich/raw editing, image inputs and transactional save, plus end-to-end preview coverage. This completes issue #22.

Each slice should leave the app usable and verifiable on its own. Phase 4 locale switching and translation status, arbitrary Astro project schemas, MDX rich editing, and automatic rewriting of prose hyperlinks are outside this feature.

## References

- [Astro content collections](https://docs.astro.build/en/guides/content-collections/)
- [Astro images](https://docs.astro.build/en/guides/images/)
- [Tiptap Markdown extension](https://tiptap.dev/docs/editor/markdown)
- [Zod JSON Schema conversion](https://zod.dev/json-schema)
