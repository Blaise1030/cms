# Skills module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A **Skills** rail tab where users install project/global `SKILL.md` packages (catalog or custom), edit them in a Design-like markdown editor with an installed-skills sidebar, and agents receive a short installed-skills index with their guide.

**Architecture:** Rust `skills` module owns package I/O under `<project>/.orso/skills/<id>/` and a configurable global root; HTTP mirrors Design’s change gate for project scope. Vue `web/src/modules/skills` reuses `MarkdownBodyEditor`. Agent index is appended to `agent_guide()` and written into the Cursor rule (and refreshed on ACP chat start) because live chat does not pass a system prompt through ACP `session/new`.

**Tech Stack:** Rust (axum, serde, serde_yaml, rust-embed), Vue 3 + Pinia + Vitest, existing `MarkdownBodyEditor` / shadcn sidebar.

**Spec:** `docs/superpowers/specs/2026-11-06-skills-module-design.md`

## Global Constraints

- Project root: `<project>/.orso/skills/<id>/SKILL.md` (site repo). Never `{projects-dir}/.orso/skills`.
- Global root: configurable absolute path; default `{projects_dir}/.orso/global-skills` (UX may show a friendlier label).
- `SKILL.md` frontmatter must include string `name` and `description`; unknown keys survive save.
- Project install/save/uninstall: `require_content_change` + hash + `write_atomic` + `checkpoint_paths` (summaries: `Installed skill`, `Updated skill`, `Removed skill`).
- Global mutations: no project change gate; hash conflict still applies.
- Catalog ids (v1): `seo`, `design` (complementary to Design module; does not replace `DESIGN.md`).
- Same-scope id collision on install: `409`.
- Invalid skill reason: `invalid_skill`.
- Commands: `cargo test`, `pnpm -C web test`, `pnpm -C web exec vue-tsc --noEmit`.

## Spec refinements

1. **ACP injection:** Live chat uses ACP and does not call `agent_guide()`. v1 refreshes `.cursor/rules/orso-changes.mdc` with `guided_prompt(project, global)` on chat start (same file `write_cursor_rule` already owns), and switches every existing `agent_guide()` injection site that has a project dir to `guided_prompt`.
2. **Global path persistence:** No app-settings daemon API exists today. Store `{ "globalPath": "<abs>" }` in `{projects_dir}/.orso/skills-settings.json`; expose `GET/PUT /api/skills/settings`. App Settings UI calls that API (not `localStorage`).
3. **Import over HTTP:** Browser sends `{ skillMd: string, id?: string }` (file picker reads text), not raw multipart folders.
4. **Duplicate to project:** `POST /api/skills/install` with `{ source: { copy: { id, fromScope: "global" } }, scope: "project" }`.

## File map

| Path | Responsibility |
| --- | --- |
| `src/skills/mod.rs` | Parse/render, list, read/write, install/uninstall, catalog, index, settings path |
| `src/skills/catalog/seo/SKILL.md`, `.../design/SKILL.md` | Embedded catalog (rust-embed or `include_str!`) |
| `src/http/skills.rs` | Axum routes |
| `src/providers.rs` | `guided_prompt`, `write_cursor_rule` refresh with skills |
| `src/chat/manager.rs` | On ACP start, refresh cursor rule for project |
| `tests/skills.rs` | HTTP integration |
| `web/src/modules/skills/*` | Panel, store, install dialog, tests |
| `web/src/core/api.ts`, `types.ts` | Client + types |
| `web/src/router.ts`, `ProjectRail.vue`, `ProjectPage.vue`, palette | Tab wiring |
| `web/src/modules/settings/SkillsSettings.vue` | Global path in App Settings |

## Review Focus

- Project skill save with no active change → `CHANGE_REQUIRED`; global save succeeds without a change. → Task 2.
- Same `id` in project and global lists as two rows; GET/PUT require `scope`. → Task 2.
- Stale hash on project or global save → `stale_content`; file unchanged. → Task 2.
- Invalid / missing frontmatter packages skipped in list + index, but GET returns `invalid_skill`. → Tasks 1–2.
- Chat start refreshes Cursor rule to include newly installed skills without restarting the daemon. → Task 3.

---

### Task 1: Skill package core + catalog + index

**Files:**
- Create: `src/skills/mod.rs`, `src/skills/catalog/seo/SKILL.md`, `src/skills/catalog/design/SKILL.md`
- Modify: `src/lib.rs` (`pub mod skills;`)
- Test: unit tests in `src/skills/mod.rs`

**Interfaces:**
- Produces:
  - `pub const SKILL_FILE: &str = "SKILL.md";`
  - `pub const PROJECT_SKILLS_REL: &str = ".orso/skills";`
  - `pub fn default_global_skills_dir(projects_dir: &Path) -> PathBuf` → `projects_dir.join(".orso/global-skills")`
  - `pub enum SkillScope { Project, Global }` serde `"project" | "global"`
  - `pub struct SkillMeta { id, name, description, scope, path: PathBuf }`
  - `pub struct SkillDoc { id, name, description, body, hash, scope, path, valid: bool, error: Option<String> }`
  - `pub fn split_skill(source: &str) -> Result<(Map<String, Value>, String), ApiError>` — same `---` rules as Design; require `name`/`description` strings or `invalid_skill`
  - `pub fn render_skill(frontmatter: &Map<String, Value>, body: &str) -> Result<String, ApiError>`
  - `pub fn parse_meta(id: &str, scope: SkillScope, path: &Path, source: &str) -> SkillMeta` — on parse failure return meta with empty name/description only used internally; prefer `read_doc` for API
  - `pub fn list_dir(root: &Path, scope: SkillScope) -> Vec<SkillMeta>` — skip dirs without parseable required keys
  - `pub fn skills_index(project_root: &Path, global_root: &Path) -> String` — empty string if none; else the exact markdown block from the spec (heading `## Installed skills`, bullet lines, closing “Read a skill's SKILL.md…” paragraph)
  - `pub fn catalog_entries() -> Vec<(id, name, description)>` for `seo` and `design`
  - `pub fn catalog_skill_md(id: &str) -> Option<&'static str>`

- [ ] **Step 1: Write failing unit tests** for `split_skill` (happy path, missing name, body with `---`, unknown keys preserved via render round-trip), `list_dir` (skips invalid), `skills_index` (empty → `""`; one project + one global), `catalog_skill_md("seo")` contains `name:` and `description:`.
- [ ] **Step 2: Run** `cargo test --lib skills` — Expected: FAIL (module missing).
- [ ] **Step 3: Implement** module + two catalog `SKILL.md` files (short, actionable agent instructions; design skill must say it complements `DESIGN.md`, does not replace it).
- [ ] **Step 4: Run** `cargo test --lib skills` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(skills): package parse, catalog, and agent index`

---

### Task 2: Daemon settings, CRUD/install HTTP

**Files:**
- Extend: `src/skills/mod.rs` (IO + settings)
- Create: `src/http/skills.rs`
- Modify: `src/http/mod.rs` (mod + routes), `src/lib.rs` if needed
- Test: `tests/skills.rs` (harness from `tests/design.rs`)

**Interfaces:**
- Consumes: Task 1; `require_content_change`, `stale`, `lock_for`, `write_atomic`, `project_root`, `emit_checkpoint`, `git::checkpoint_paths`, `AppState.config.projects_dir`
- Produces:
  - Settings: `skills_settings_path(projects_dir) -> projects_dir.join(".orso/skills-settings.json")`; `pub fn load_global_root(projects_dir) -> PathBuf`; `pub fn save_global_root(projects_dir, path: &Path) -> Result<(), ApiError>` (absolute + create_dir_all parent; reject non-absolute)
  - `pub async fn list(state, project) -> SkillsList { installed: Vec<SkillMeta>, catalog: Vec<CatalogItem { id, name, description, installed_project, installed_global }> }`
  - `pub async fn get(state, project, id, scope) -> SkillDoc`
  - `pub async fn save(state, SaveSkill { project?, branch?, id, scope, name, description, body, hash }) -> SkillDoc`
  - `pub async fn install(state, InstallSkill { project?, branch?, scope, source }) -> SkillMeta` where `source` is `catalog { id }` | `import { skill_md, id? }` | `blank { name, description }` | `copy { id, from_scope }`
  - `pub async fn uninstall(state, project?, branch?, id, scope) -> ()`
  - Routes:
    - `GET /api/skills?project=`
    - `GET /api/skills/:id?project=&scope=`
    - `PUT /api/skills/:id`
    - `POST /api/skills/install`
    - `DELETE /api/skills/:id?project=&scope=&branch=`
    - `GET /api/skills/settings` → `{ globalPath, defaultGlobalPath }`
    - `PUT /api/skills/settings` → `{ globalPath }` (empty string resets to default)

- [ ] **Step 1: Write failing tests** in `tests/skills.rs`:
  - `list_empty_then_catalog`
  - `install_catalog_project_checkpoints`
  - `install_project_without_change_requires_change`
  - `install_global_without_change_ok`
  - `save_project_stale_hash`
  - `save_global_ok`
  - `uninstall_project`
  - `duplicate_id_conflict`
  - `get_invalid_skill_reason`
  - `settings_round_trip_and_reset`
- [ ] **Step 2: Run** `cargo test --test skills` — Expected: FAIL.
- [ ] **Step 3: Implement** IO + handlers; register routes next to design.
- [ ] **Step 4: Run** `cargo test --test skills` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(skills): HTTP list/install/save/uninstall and global path settings`

---

### Task 3: Agent guided prompt + chat-start refresh

**Files:**
- Modify: `src/providers.rs` (`guided_prompt`, `write_cursor_rule`)
- Modify: `src/chat/manager.rs` (on successful ACP session start for a project cwd)
- Test: `src/providers.rs` unit tests; thin test or existing chat test hook if present

**Interfaces:**
- Consumes: `skills::skills_index`, `skills::load_global_root`
- Produces:
  - `pub fn guided_prompt(project_root: &Path, global_root: &Path) -> String` = `agent_guide()` + optional `"\n\n" + skills_index`
  - `write_cursor_rule(project_dir, global_root)` writes `guided_prompt` into `.cursor/rules/orso-changes.mdc`
  - Claude/Codex/OpenCode launch paths that already have `project`/`dir`: use `guided_prompt` instead of bare `agent_guide()` when the project root is known; keep `agent_guide()` for sites with no project
  - Chat manager: after ACP `session/new` succeeds, `write_cursor_rule(&cwd, &load_global_root(projects_dir))` (ignore IO errors after log/debug, same spirit as OpenCode config writes)

- [ ] **Step 1: Write failing tests** — `guided_prompt` includes catalog-installed skill name after writing a skill dir; `write_cursor_rule` file contains `## Installed skills` when a skill exists; existing `agent_guide()` tests still pass.
- [ ] **Step 2: Run** `cargo test --lib providers` — Expected: FAIL on new assertions.
- [ ] **Step 3: Implement** signatures and chat-start refresh (thread `projects_dir` from `AppState` already available to the manager).
- [ ] **Step 4: Run** `cargo test --lib providers` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(skills): inject skills index into agent guide and cursor rule`

---

### Task 4: Web API, route, rail, store

**Files:**
- Modify: `web/src/core/types.ts`, `web/src/core/api.ts`, `web/src/test/fakeDaemon.ts`
- Modify: `web/src/router.ts`, `web/src/router.test.ts`, `web/src/app/ProjectRail.vue`, `web/src/app/ProjectPage.vue` (`showChat` includes `"skills"`), `web/src/app/palette/usePaletteGroups.ts`
- Create: `web/src/modules/skills/store.ts`, `store.test.ts`, placeholder `SkillsPanel.vue` with `data-testid="skills-panel"`

**Interfaces:**
- Produces types: `SkillScope`, `SkillMeta`, `SkillDoc`, `SkillsList`, `SkillsSettings`, install/save payloads matching Task 2 (camelCase).
- API methods: `skills`, `skill`, `saveSkill`, `installSkill`, `uninstallSkill`, `skillsSettings`, `saveSkillsSettings`.
- `ProjectTab` adds `"skills"`; route `project-skills`; rail label **Skills** (icon: `Sparkles` or similar from lucide); palette tab entry; chat visible on skills.
- Pinia `useSkillsStore`: `list`, `selected: { id, scope } | null`, `doc`, `draftName/Description/Body`, `dirty`, `loading`, `saving`, `error`, `conflict`, `needsChange`, `changedOnDisk`, `invalid`; actions `open(projectId)`, `select(id, scope)`, `setBody`, `save`, `resumeSave`, `discard`, `install(...)`, `uninstall`, `reload`, `canLeave`.

- [ ] **Step 1: Write failing store + router tests** (open lists; select loads doc; project save maps `CHANGE_REQUIRED` → `needsChange`; global save does not set `needsChange`).
- [ ] **Step 2: Run** `pnpm -C web test src/modules/skills/store.test.ts src/router.test.ts` — Expected: FAIL.
- [ ] **Step 3: Implement** types, api, fakeDaemon, routing, store, placeholder panel.
- [ ] **Step 4: Run** same tests + `pnpm -C web exec vue-tsc --noEmit` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(skills): web API, Skills tab route, and store`

---

### Task 5: SkillsPanel UI (sidebar + editor)

**Files:**
- Create/replace: `web/src/modules/skills/SkillsPanel.vue`, `SkillsPanel.test.ts`
- Reuse: content/design sidebar + `MarkdownBodyEditor` patterns from `DesignPanel.vue` / `ContentPanel.vue`

**Interfaces:**
- Consumes: Task 4 store
- UI: header Install / Save / Discard / Uninstall; sidebar groups This project / Global; main editor; empty and invalid states; `NewChangeDialog` resume path like Design; testids `skills-panel`, `skills-install`, `skills-save`, `skills-sidebar`, `skills-editor`

- [ ] **Step 1: Write failing component tests** — empty CTA; select shows editor body; project dirty Save disabled when `needsChange` with create-change control; global Save enabled.
- [ ] **Step 2: Run** `pnpm -C web test src/modules/skills/SkillsPanel.test.ts` — Expected: FAIL.
- [ ] **Step 3: Implement** panel layout (no Install dialog body yet — button can `emit`/set `installOpen` stub).
- [ ] **Step 4: Run** test — Expected: PASS.
- [ ] **Step 5: Commit** `feat(skills): Skills panel with sidebar and markdown editor`

---

### Task 6: Install dialog + App Settings global path

**Files:**
- Create: `web/src/modules/skills/InstallSkillDialog.vue`, `InstallSkillDialog.test.ts`
- Create: `web/src/modules/settings/SkillsSettings.vue`
- Modify: `AppSettingsPage.vue` / types for tab `"skills"` (or nest under Assistant — prefer new **Skills** app-settings section), `AppSettingsPage.test.ts`
- Wire Install button in `SkillsPanel.vue`; palette command “Install skill…” opens Skills tab + dialog (optional if palette pattern is heavy — minimum: dialog from panel header)

**Interfaces:**
- Dialog: Catalog | Import segments; scope picker; catalog install disabled when `installedProject/Global` for scope; Import reads `.md` file → `skillMd`; Create blank fields; calls `store.install`
- SkillsSettings: show `globalPath`, browse/text input, Save, Reset to default via `skillsSettings` / `saveSkillsSettings`

- [ ] **Step 1: Write failing tests** for dialog (catalog install payload scope=project; already-installed disabled) and settings (reset clears to defaultGlobalPath).
- [ ] **Step 2: Run** the new vitest files — Expected: FAIL.
- [ ] **Step 3: Implement** dialog + settings section + wiring.
- [ ] **Step 4: Run** `pnpm -C web test src/modules/skills src/modules/settings/AppSettingsPage.test.ts` and `vue-tsc` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(skills): install dialog and global skills path settings`

---

### Task 7: Verification pass

**Files:** none new unless fixes

- [ ] **Step 1: Run** `cargo test --test skills && cargo test --lib skills && cargo test --lib providers`
- [ ] **Step 2: Run** `pnpm -C web test src/modules/skills && pnpm -C web exec vue-tsc --noEmit`
- [ ] **Step 3: Fix** any failures from integration between tasks.
- [ ] **Step 4: Commit** only if fixes landed: `fix(skills): …`
