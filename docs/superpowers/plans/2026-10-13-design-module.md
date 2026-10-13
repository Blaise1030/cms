# Design module (DESIGN.md) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A per-project **Design** tab that creates and edits `DESIGN.md` (Google's DESIGN.md format) with a token form plus the tiptap body editor, and asks the agent to apply it to `global.css` `@theme`.

**Architecture:** A small Rust module reads and writes `DESIGN.md` at the project root through the content save guards (lock, active change, hash check, atomic write, checkpoint). A new Vue module (`web/src/modules/design`) has a Pinia store, a token form, and a panel that reuses `MarkdownBodyEditor`. The starter draft is the template's own `DESIGN.md`, served from the embedded template.

**Tech Stack:** Rust (axum, serde_json, serde_yaml, rust-embed), Vue 3 + Pinia + Vitest, tiptap (existing editor).

**Spec:** `docs/superpowers/specs/2026-10-13-design-module-design.md`

## Global Constraints

- File: `DESIGN.md` at the project root. Format: DESIGN.md spec version `alpha` (YAML frontmatter tokens + `##` sections).
- Body section order: Overview, Colors, Typography, Layout, Elevation & Depth, Shapes, Components, Do's and Don'ts, then `## Voice & Tone`.
- Unknown token groups, token names and body sections must survive every save.
- Saves need an active change (`require_content_change`), exactly like content; errors reuse `CHANGE_REQUIRED`, `stale_content`, `branch_changed`.
- Checkpoint summary: `Updated design`.
- Invalid frontmatter: reason `invalid_design`; never overwrite the file in that state.
- Apply prompt (exact): `Update the @theme block in src/styles/global.css (fonts, colours, radius, spacing) to match DESIGN.md, and follow its guidance in the components.`
- Commands: Rust `cargo test`, web `pnpm -C web test`, typecheck `pnpm -C web exec vue-tsc --noEmit`.

## Spec refinement

The spec had a frontend `starter.ts` kept equal to the template file by a test. Instead, `GET /api/design`
returns `starter: { tokens, body }` parsed from the embedded `templates/astro-marketing/DESIGN.md`
when the project has no `DESIGN.md`. One source, no sync test.

## Review Focus

- A `DESIGN.md` with no frontmatter at all (plain Markdown) opens with empty tokens and its body intact, and saving adds frontmatter without losing the body. → Task 2 test `no_frontmatter_round_trips`.
- Token order is kept through load → save (users and agents read the file). → Task 2 test `unknown_keys_and_order_survive_update`.
- Renaming a colour to a name that already exists must not silently drop the other token. → Task 4 test `rename to an existing name is rejected`.
- A body that itself contains a `---` line (Markdown horizontal rule) must not be mistaken for the frontmatter end. → Task 2 test `body_with_rule_is_kept`.
- The tab opened with a dirty draft and the agent then edits `DESIGN.md`: the draft is kept and flagged, not replaced. → Task 3 test `checkpoint while dirty flags changedOnDisk`.

---

### Task 1: Template DESIGN.md and agent guidance

**Files:**
- Create: `templates/astro-marketing/DESIGN.md`
- Modify: `templates/astro-marketing/AGENTS.md`, `src/static.rs` (test list), `tests/projects.rs` (template file list near line 95)
- Test: `src/static.rs` unit test, `tests/projects.rs`

**Interfaces:**
- Produces: the embedded `DESIGN.md` that Task 2 serves as `starter` and that new projects get.

- [ ] **Step 1: Add `"DESIGN.md"`** to the expected lists in `src/static.rs` `template_contains_content_model_and_excludes_build_artifacts` and `tests/projects.rs`.
- [ ] **Step 2: Run** `cargo test static && cargo test --test projects` — Expected: FAIL (missing file).
- [ ] **Step 3: Write `templates/astro-marketing/DESIGN.md`**
  - Frontmatter: `version: alpha`; `name`/`description` copied from `src/site.ts`; `colors` `primary`, `secondary`, `tertiary`, `neutral`; `typography` `h1`, `h2`, `body-md`, `label` (each `fontFamily`, `fontSize`, `fontWeight`, `lineHeight`); `rounded` `sm`, `md`, `lg`, `full`; `spacing` `sm`, `md`, `lg`. Values: read the section components (`src/components/sections/*.astro`) and `src/styles/global.css` and use the Tailwind defaults they actually use (e.g. the brand colour class on buttons, `rounded-lg` → `0.5rem`).
  - Body: the eight spec sections in order plus `## Voice & Tone`, each one or two sentences describing the template as it is.
  - `AGENTS.md`: add under conventions: "`DESIGN.md` is the design source of truth. When you change styles, update both `DESIGN.md` and the `@theme` block in `src/styles/global.css`."
- [ ] **Step 4: Run** the Step 2 commands — Expected: PASS.
- [ ] **Step 5: Commit** `feat(template): ship DESIGN.md and agent guidance`

---

### Task 2: Backend read/save of DESIGN.md

**Files:**
- Modify: `Cargo.toml` (`serde_json = { version = "1.0.151", features = ["preserve_order"] }`)
- Modify: `src/content/mutation.rs` — make `lock_for`, `stale`, `require_content_change`, `write_atomic`, `project_root`, `read_current`, `emit_checkpoint` `pub(crate)`
- Create: `src/design.rs`, `src/http/design.rs`
- Modify: `src/lib.rs` (`pub mod design;`), `src/http/mod.rs` (`pub mod design;` + route)
- Test: `tests/design.rs`

**Interfaces:**
- Produces (Rust):
  - `pub const DESIGN_FILE: &str = "DESIGN.md";`
  - `#[derive(Serialize)] #[serde(rename_all = "camelCase")] pub struct DesignDoc { exists: bool, tokens: Value, body: String, hash: Option<String>, branch: String, starter: Option<Starter> }` with `hash`/`starter` skipped when `None`.
  - `#[derive(Serialize)] pub struct Starter { tokens: Value, body: String }`
  - `pub fn split(source: &str) -> Result<(Value, &str), ApiError>` — frontmatter only when the source starts with `---\n` and a later line is exactly `---`; the first such line closes it. No frontmatter → `(json!({}), source)`. YAML that fails to parse or is not a mapping → `ApiError::badreq(..).with_reason("invalid_design")`.
  - `pub fn render(tokens: &Map<String, Value>, body: &str) -> Result<String, ApiError>` — empty map → `body` unchanged; otherwise `format!("---\n{}---\n{body}", serde_yaml::to_string(tokens)?)`.
  - `pub async fn read(state: &AppState, project: &str) -> Result<DesignDoc, ApiError>` — branch from the same helper `catalog::read_entry` uses (make `catalog::current_branch` `pub(crate)`); `hash` = `catalog::hash(&source)`; missing file → `exists: false`, `tokens: {}`, `body: ""`, `starter: Some(..)` from `static_files::Template::get("DESIGN.md")` run through `split`.
  - `pub struct SaveDesign { project: String, branch: String, hash: Option<String>, tokens: Value, body: String }`
  - `pub async fn save(state: &AppState, req: SaveDesign) -> Result<DesignDoc, ApiError>` — order: lock → `require_content_change` → `tokens` must be an object (`badreq` "tokens must be an object") → `hash` None requires missing file, Some requires matching hash, else `stale()` → `write_atomic` (mode kept on update, `0o644` on create) → `git::checkpoint_paths(&root, "Updated design", &["DESIGN.md"])` → `emit_checkpoint` → return `read`.
- Produces (HTTP): `GET /api/design?project=` → `DesignDoc`; `PUT /api/design` JSON body `{project, branch, hash?, tokens, body}` → `DesignDoc`.

- [ ] **Step 1: Write failing tests in `tests/design.rs`**

Copy the `env(change)` / `git_cli` / `link_node_modules` harness from `tests/content_mutation.rs` and add `get(&env)` / `put(&env, json)` helpers returning `(StatusCode, Value)`. New projects include `DESIGN.md` (Task 1); delete it in tests that need a missing file. Tests:

```rust
#[tokio::test] async fn template_project_has_design()         // fresh project → exists=true, tokens.version=="alpha", hash present
#[tokio::test] async fn missing_file_returns_starter()        // exists=false, starter.tokens.version=="alpha", starter.body contains "## Overview"
#[tokio::test] async fn create_writes_file_and_checkpoints()  // PUT without hash → 200, file starts "---\n", git log -1 subject == "Updated design"
#[tokio::test] async fn create_when_present_is_stale()        // 409, error.reason == "stale_content"
#[tokio::test] async fn update_with_old_hash_is_stale()       // 409, "stale_content"
#[tokio::test] async fn unknown_keys_and_order_survive_update() // file with zeta/colors{b,a}/components + "## Iconography"; GET then PUT same tokens → keys order zeta,colors(b,a),components; body keeps "## Iconography"
#[tokio::test] async fn no_frontmatter_round_trips()          // file "# Hi\n\nText\n" → tokens {}, body unchanged; PUT {name:"X"} → file "---\nname: X\n---\n# Hi\n\nText\n"
#[tokio::test] async fn body_with_rule_is_kept()              // "---\nname: X\n---\nA\n\n---\n\nB\n" → body "A\n\n---\n\nB\n"
#[tokio::test] async fn save_without_change_requires_change() // env(false) → error.code == "CHANGE_REQUIRED"
#[tokio::test] async fn invalid_frontmatter_is_reported()     // "---\n: [\n---\n" → 400, reason "invalid_design"; PUT tokens{} with any hash → file unchanged
#[tokio::test] async fn tokens_must_be_object()               // tokens: [] → 400
```

- [ ] **Step 2: Run** `cargo test --test design` — Expected: FAIL (no route / module).
- [ ] **Step 3: Implement** the interfaces above; enable `preserve_order`; register `.route("/api/design", get(design::get).put(design::put))` next to the content routes. Handlers follow `http/content.rs` (`Query` + `query_or_badreq` for GET, `Json` body for PUT).
- [ ] **Step 4: Run** `cargo test --test design` — Expected: PASS. Then `cargo test` — Expected: all PASS (catches anything relying on sorted JSON keys after `preserve_order`; fix assertions that compare serialized key order, not behavior).
- [ ] **Step 5: Commit** `feat(design): read and save DESIGN.md`

---

### Task 3: Frontend API, route, navigation and store

**Files:**
- Modify: `web/src/core/types.ts`, `web/src/core/api.ts`, `web/src/test/fakeDaemon.ts` (defaults for the new calls)
- Modify: `web/src/router.ts`, `web/src/router.test.ts`, `web/src/app/ProjectRail.vue`, `web/src/app/palette/usePaletteGroups.ts`
- Create: `web/src/modules/design/store.ts`, `web/src/modules/design/store.test.ts`
- Create (placeholder, filled in Task 5): `web/src/modules/design/DesignPanel.vue` rendering `<div data-testid="design-panel" />`

**Interfaces:**
- Produces:
  - `types.ts`: `export type DesignTokens = Record<string, unknown>;` `export interface DesignDoc { exists: boolean; tokens: DesignTokens; body: string; hash?: string; branch: string; starter?: { tokens: DesignTokens; body: string } }`
  - `api.ts`: `design: (project: string) => Promise<DesignDoc>` (GET `/api/design`), `saveDesign: (body: { project: string; branch: string; hash?: string; tokens: DesignTokens; body: string }) => Promise<DesignDoc>` (PUT).
  - Router: `ProjectTab` adds `"design"`; child route `{ path: "design", name: "project-design", component: DesignPanel, props: r => ({ projectId: r.params.id }) }`; `projectTabFromRoute("project-design") === "design"`. Rail and palette `TABS` add `{ id: "design", label: "Design", icon: Palette }` after Content.
  - `useDesignStore` (Pinia, id `design`) exposing: `doc: Ref<DesignDoc | null>`, `draftTokens: Ref<DesignTokens>`, `draftBody: Ref<string>`, `dirty: ComputedRef<boolean>`, `loading`, `saving`, `error: Ref<string | null>`, `conflict: Ref<{ reason: "stale_content" | "branch_changed"; message: string } | null>`, `needsChange: Ref<boolean>`, `changedOnDisk: Ref<boolean>`, `invalid: Ref<string | null>` (error message when reason `invalid_design`); actions `open(projectId)`, `create()`, `setTokens(t)`, `setBody(b)`, `save(branch?)`, `resumeSave(branch)`, `discard()`, `reload()`, `canLeave(): boolean`.

Behavior: `dirty` compares drafts to `doc` (deep equal on tokens, string equal on body; for a new file, true once `create()` ran). `create()` copies `doc.starter` into the drafts. `save` sends `hash` only when `doc.exists`, and `branch ?? doc.branch`. `CHANGE_REQUIRED` → `needsChange = true`; reasons `stale_content`/`branch_changed` → `conflict`; drafts kept on any failure. `reload()` re-reads; when dirty it only sets `changedOnDisk` if the new hash differs. Subscribe to `checkpoint.created` for the open project the way `web/src/modules/checkpoints/store.ts` does and call `reload()`. `canLeave()` uses `window.confirm` like the content store.

- [ ] **Step 1: Write failing tests**
  - `router.test.ts`: `/p/x/design` resolves to `project-design`; `projectTabFromRoute("project-design") === "design"`.
  - `store.test.ts` (fake daemon pattern from `content/lifecycle.test.ts`):
    - `open loads the doc and drafts`
    - `create fills drafts from starter and is dirty`
    - `save sends full tokens including unknown keys` (doc tokens `{ components: {x:1}, colors: {primary:"#000"} }`, edit primary → `saveDesign` called with `tokens.components` equal `{x:1}`)
    - `save on new file omits hash`
    - `CHANGE_REQUIRED sets needsChange and keeps the draft`; `resumeSave("c/1") saves with branch "c/1"`
    - `stale_content sets conflict`
    - `invalid_design sets invalid`
    - `checkpoint while clean reloads`; `checkpoint while dirty flags changedOnDisk` (draft unchanged)
- [ ] **Step 2: Run** `pnpm -C web test src/router.test.ts src/modules/design` — Expected: FAIL.
- [ ] **Step 3: Implement** types, API, fake daemon defaults, route/rail/palette, store.
- [ ] **Step 4: Run** `pnpm -C web test` and `pnpm -C web exec vue-tsc --noEmit` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(design): API client, route and store`

---

### Task 4: Token form

**Files:**
- Create: `web/src/modules/design/DesignTokensForm.vue`, `web/src/modules/design/tokens.ts`, `web/src/modules/design/DesignTokensForm.test.ts`

**Interfaces:**
- Produces:
  - `tokens.ts`: `renameKey(group: Record<string, unknown>, from: string, to: string): Record<string, unknown> | null` (keeps position; `null` when `to` is empty or already exists), `removeKey(group, key)`, `addKey(group, base: string, value: unknown)` (picks `base`, `base-2`, … to avoid clashes).
  - `DesignTokensForm.vue`: `defineProps<{ modelValue: DesignTokens }>()`, emits `update:modelValue` with a new object containing every original key.

Groups, each collapsible, with `data-testid`s `design-group-<name>`:
- Brand: `name`, `description` inputs.
- Colors: row per token — name input, `<input type="color">` (shown value is the hex, or `#000000` when not hex), text input for any CSS value, swatch `<span :style="{ background: value }">`, remove button; "Add colour" → `addKey(colors, "color", "#000000")`.
- Typography: card per token with `fontFamily`, `fontSize`, `fontWeight`, `lineHeight`, `letterSpacing` (optional — omitted from the object when empty); "Add style" → `addKey(typography, "style", { fontFamily: "Inter", fontSize: "16px", fontWeight: 400, lineHeight: 1.5 })`.
- Rounded, Spacing: name + value rows; add with base `size`, value `"8px"`.
- Rename commits on blur/Enter; a `null` from `renameKey` restores the old name and shows "Name already used".

- [ ] **Step 1: Write failing tests**
  - `tokens.ts`: `renameKey keeps position`, `rename to an existing name is rejected` (returns `null`), `addKey avoids clashes`.
  - Form: `editing a colour emits tokens with other groups untouched` (input `{ colors:{primary:"#111"}, components:{btn:{}}, version:"alpha" }`), `removing a spacing token`, `adding a typography style`, `empty letterSpacing is omitted`.
- [ ] **Step 2: Run** `pnpm -C web test src/modules/design` — Expected: FAIL.
- [ ] **Step 3: Implement.** Use existing `@/components/ui` `Input`, `Label`, `Button`, `Accordion`.
- [ ] **Step 4: Run** `pnpm -C web test src/modules/design` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(design): token form`

---

### Task 5: Design panel and Apply to site

**Files:**
- Modify: `web/src/modules/design/DesignPanel.vue` (replace placeholder)
- Create: `web/src/modules/design/agentRequests.ts`, `web/src/modules/design/DesignPanel.test.ts`

**Interfaces:**
- Consumes: `useDesignStore` (Task 3), `DesignTokensForm` (Task 4), `MarkdownBodyEditor` props `{ body, original, kind: "markdown", rich: true }` and `change` event `{ body, dirty }`, `NewChangeDialog` as used in `ContentPanel.vue`, `useAgentOutboxStore`.
- Produces: `askAgentToApplyDesign(): void` — `useAgentOutboxStore().send({ kind: "text", text: <exact prompt from Global Constraints>, submit: true })` and `toast.info("Asked the agent to apply the design")`.

Panel (`defineProps<{ projectId: string }>()`; `watch(projectId, store.open, { immediate: true })`; route guards call `store.canLeave()`):
- `invalid` → message "DESIGN.md has invalid frontmatter. Ask the agent to fix it." plus the error text; no editor (`data-testid="design-invalid"`).
- `!doc.exists && !dirty` → empty state with **Create design** (`data-testid="design-create"`) calling `store.create()`.
- Otherwise: `MarkdownBodyEditor` (async import like `ContentPanel.vue`) bound to `draftBody`; side panel `DesignTokensForm` bound to `draftTokens`; header buttons Save (`design-save`, disabled unless dirty or saving), Discard (`design-discard`), **Apply to site** (`design-apply`, disabled when `dirty` or `!doc.exists`).
- `changedOnDisk` banner with Reload; `conflict` banner with the message and Reload; `needsChange` opens `NewChangeDialog`, whose created branch calls `store.resumeSave(branch)`.

- [ ] **Step 1: Write failing tests** (mock `MarkdownBodyEditor.vue` with a stub emitting `change`)
  - `missing file shows Create, which opens the editor with the starter`
  - `Apply to site sends the exact prompt as a request` (outbox `messages[0]` equals `{ kind: "text", text: <prompt>, submit: true }`)
  - `Apply is disabled while dirty`
  - `invalid frontmatter shows the read-only message`
  - `Save calls saveDesign`
- [ ] **Step 2: Run** `pnpm -C web test src/modules/design` — Expected: FAIL.
- [ ] **Step 3: Implement** the panel and `agentRequests.ts`.
- [ ] **Step 4: Run** `pnpm -C web test`, `pnpm -C web exec vue-tsc --noEmit`, `cargo test` — Expected: PASS.
- [ ] **Step 5: Manual check** with `just dev`: open a project → Design tab shows the template design; edit a colour, Save (prompts Create Change on main), Apply to site → agent receives the prompt.
- [ ] **Step 6: Commit** `feat(design): design panel and apply to site`
