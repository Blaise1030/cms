# Project settings (orso.json) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A committed `orso.json` at the repo root defines the Astro folder, install folder, install
command and dev command, editable from a new Settings tab.

**Architecture:** A new `src/settings.rs` loads and validates the file on every launch/install (no
registry copy). The dev-server supervisor, install, dependency fingerprint and content paths read it.
The API and web panel mirror the `DESIGN.md` module (`src/design.rs`, `web/src/modules/design/`).

**Tech Stack:** Rust (tokio, axum, serde_json), Vue 3 + Pinia, Vitest.

**Spec:** `docs/superpowers/specs/2026-10-20-project-settings-design.md`

## Global Constraints

- File name: `orso.json` at the repo root. File and every field optional.
- Defaults: `root` = `.`, `installDir` = `.`, `install` = `npm install --no-audit --no-fund --no-progress`,
  `dev` = `npm run dev -- --port {port} --host {host} --ignore-lock`.
- `{host}` is always `127.0.0.1`. Only `{port}` and `{host}` are substituted.
- Commands run as `sh -c "<command>"`.
- Saves require an active change and a matching hash; checkpoint message `Updated project settings`.
- Unknown keys survive a save; empty-string fields are dropped before writing.
- API and checkpoint paths stay relative to the repo root.
- Copy: "Settings can't be edited on main. Create a change." and
  "orso.json has errors — the dev server won't start until they're fixed."
- Run Rust tests with `cargo test`, web tests with `pnpm -C web test`.

## Review Focus

- `root` or `installDir` that is a symlink pointing outside the repo must be rejected (Task 1).
- `./apps/web/` and `apps/web` must resolve to the same folder and the same content paths (Task 1).
- An invalid `orso.json` present at daemon start must leave the project `crashed` with the message,
  not stuck `starting` and not retried (Task 3).
- Switching to a branch where `orso.json` was deleted must restart the dev server with defaults (Task 2).
- An `orso.json` that is not valid JSON must be repairable from the UI: a PUT with its hash
  overwrites it (Task 4; unlike `DESIGN.md`, which refuses).

---

### Task 1: `src/settings.rs` — load, defaults, validation

**Files:**
- Create: `src/settings.rs`
- Modify: `src/lib.rs` (add `pub mod settings;`)

**Interfaces:**
- Produces:
  - `pub const SETTINGS_FILE: &str = "orso.json";`
  - `pub const HOST: &str = "127.0.0.1";`
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize)] #[serde(rename_all = "camelCase")]`
    `pub struct ProjectSettings { pub root: String, pub install_dir: String, pub install: String, pub dev: String }`
    — `root`/`install_dir` normalized (`"."` or `a/b`, no `./`, no trailing `/`).
  - `impl ProjectSettings { pub fn defaults() -> Self; pub fn site_dir(&self, project: &Path) -> PathBuf; pub fn install_path(&self, project: &Path) -> PathBuf; pub fn dev_command(&self, port: u16) -> String; pub fn site_prefix(&self) -> String }`
    — `site_prefix` is `""` for `"."`, else `"apps/web/"`.
  - `pub type FieldErrors = BTreeMap<String, String>;` (keys: `root`, `installDir`, `install`, `dev`, `_file`)
  - `pub fn validate(project: &Path, raw: &Map<String, Value>) -> Result<ProjectSettings, FieldErrors>`
  - `pub fn load(project: &Path) -> Result<ProjectSettings, FieldErrors>` — missing file → defaults;
    unreadable or non-object JSON → `_file` error.
  - `pub fn describe(errors: &FieldErrors) -> String` — e.g. `orso.json: dev must contain {port}`;
    multiple errors joined with `; `.
  - `pub fn site_prefix(project: &Path) -> String` — `load(project)` prefix, `""` when invalid.

- [ ] **Step 1: Write failing unit tests** in `#[cfg(test)] mod tests` (use `tempfile::tempdir()`):
  - `missing_file_gives_defaults`: `load(dir) == Ok(ProjectSettings::defaults())`, and
    `defaults().dev == "npm run dev -- --port {port} --host {host} --ignore-lock"`.
  - `missing_fields_fall_back`: file `{"install":"pnpm install"}` → `install == "pnpm install"`,
    `root == "."`, `dev` default.
  - `root_is_normalized`: with `apps/web/astro.config.mjs` present, `"./apps/web/"` and `"apps/web"`
    both load as `root == "apps/web"`, `site_prefix() == "apps/web/"`.
  - `rejects_bad_paths`: `"/abs"`, `"../x"`, `"missing"` each → `Err` with key `root`;
    same for `installDir`.
  - `rejects_symlink_escape`: `apps -> <outside tempdir>` symlink → `Err` with key `root`.
  - `root_needs_astro_config`: existing `apps/web` without `astro.config.*` → `Err` key `root`;
    `astro.config.ts` and `astro.config.mjs` both accepted.
  - `dev_needs_port`: `{"dev":"pnpm dev"}` → `Err` key `dev` message containing `{port}`.
  - `wrong_types_and_bad_json`: `{"root":1}` → key `root`; file `not json` → key `_file`;
    file `[]` → key `_file`.
  - `dev_command_substitutes`: `defaults().dev_command(4567) ==
    "npm run dev -- --port 4567 --host 127.0.0.1 --ignore-lock"`; `{other}` left as is.
  - `describe_joins`: two errors → `"orso.json: dev must contain {port}; root …"` (starts with
    `orso.json: `, contains `; `).

- [ ] **Step 2: Run** `cargo test --lib settings::` — Expected: FAIL (module missing).

- [ ] **Step 3: Implement** `src/settings.rs` with the interfaces above. Containment: canonicalize
  `project.join(rel)` and require it `starts_with(project.canonicalize())`. Empty strings for any
  field mean "unset" (default).

- [ ] **Step 4: Run** `cargo test --lib settings::` — Expected: PASS.

- [ ] **Step 5: Commit** `git commit -m "feat(settings): load and validate orso.json"`

---

### Task 2: Configured install and dependency fingerprint

**Files:**
- Modify: `src/projects.rs:575,596,729-749` (`npm_install` → `install`)
- Modify: `src/devserver.rs:39-70` (fingerprint, marker, `record_deps`) and its tests at `:653-679`
- Test: `tests/projects.rs` (existing `real_npm_install_and_site_builds` keeps passing)

**Interfaces:**
- Consumes: `settings::load`, `ProjectSettings::install_path`, `ProjectSettings::site_dir`.
- Produces:
  - `pub(crate) async fn install(project: &Path) -> Result<(), ApiError>` in `src/projects.rs`:
    loads settings (invalid → `ApiError::badreq(settings::describe(..))`), runs
    `sh -c <install>` in `install_path`, same `INSTALL_TIMEOUT`, error text
    `"{install} failed ({status}): {tail}"` / `"{install} timed out"`, then `record_deps(project)`.
  - `fn deps_fingerprint(project: &Path) -> Option<String>`, `fn deps_stale(project: &Path) -> bool`,
    `pub(crate) fn record_deps(project: &Path)` — all now take the **project** dir.
  - `pub fn install_dir_lenient(project: &Path) -> PathBuf` in `src/settings.rs`: the configured
    `installDir` when that field validates on its own, else the project dir.

- [ ] **Step 1: Write failing unit tests** in `src/devserver.rs` tests (replace the two existing deps
  tests, keeping their assertions for the root-only case):
  - `fingerprint_covers_each_lockfile`: for each of `package-lock.json`, `pnpm-lock.yaml`,
    `yarn.lock`, `bun.lock`, `bun.lockb` in `installDir`: `record_deps`, then write the lockfile →
    `deps_stale` true.
  - `fingerprint_covers_site_and_install_manifests`: `orso.json` `{"root":"site","installDir":"."}`,
    `site/astro.config.mjs`, `package.json` in both; changing `site/package.json` → stale.
  - `fingerprint_covers_settings_file`: after `record_deps`, editing `orso.json` → stale; deleting it
    → stale (branch switch to a branch without it).
  - `marker_lives_in_install_dir`: `installDir` `"."` with `root` `"site"` → marker at
    `<project>/node_modules/.orso-deps`.
  - `invalid_settings_still_fingerprint`: `orso.json` = `{"dev":"x"}` (invalid) → `deps_fingerprint`
    is `Some` and uses the default `installDir`.
  - No `package.json` in `installDir` or `root` → `deps_stale` false (non-Node repos never reinstall).
- Add to `src/projects.rs` tests: `install_runs_configured_command`: `orso.json`
  `{"install":"touch installed"}` plus a `package.json` → `install(dir)` Ok and `dir/installed` exists;
  `{"install":"exit 3"}` → Err whose message starts with `exit 3 failed`.

- [ ] **Step 2: Run** `cargo test --lib devserver:: projects::` — Expected: FAIL.

- [ ] **Step 3: Implement.** Fingerprint hashes, in order: `orso.json` bytes (empty if absent), `package.json`
  in `installDir` and `site_dir` (deduplicated), and the first present lockfile in `installDir`.
  `None` only when neither `package.json` exists. On invalid settings use `settings::install_dir_lenient`.
  Callers at `projects.rs:575` and `:596` call `install(dir)`; the clone path keeps its
  `package.json` existence check but checks `install_path`.

- [ ] **Step 4: Run** `cargo test --lib devserver:: projects::` and `cargo test --test projects` — PASS.

- [ ] **Step 5: Commit** `git commit -m "feat(settings): configured install and dependency fingerprint"`

---

### Task 3: Configured dev-server launcher

**Files:**
- Modify: `src/devserver.rs` (`Launcher::Npm` → `Launcher::Configured`, `npm_command`, `supervise`, `run_once`)
- Modify: `src/state.rs:62` (`DevServerOptions::npm` → `DevServerOptions::configured`)
- Test: `tests/devserver.rs`

**Interfaces:**
- Consumes: `settings::load`, `ProjectSettings::{site_dir, dev_command}`, `projects::install`,
  `deps_stale`.
- Produces: `Launcher::Configured`; `DevServerOptions::configured(proxy_port: u16) -> Self`;
  `fn configured_command(settings: &ProjectSettings, spec: &LaunchSpec) -> Command`
  (`sh -c <dev_command(port)>`, `current_dir = site_dir`).

- [ ] **Step 1: Write failing tests.**
  - Unit `configured_command_line` (replaces `npm_command_line`): defaults → program `sh`, args
    `["-c", "npm run dev -- --port 4567 --host 127.0.0.1 --ignore-lock"]`.
  - Integration in `tests/devserver.rs`, using `DevServerOptions::configured(0)` and a project dir
    with `orso.json`:
    - `configured_dev_command_reaches_ready`: `{"root":"site","dev":"echo Local http://127.0.0.1:{port}/; sleep 30"}`
      with `site/astro.config.mjs` → status `ready` on the allocated port; process cwd is `site`
      (assert via `"dev":"pwd > ../cwd.txt; echo Local http://127.0.0.1:{port}/; sleep 30"`).
    - `invalid_settings_crash_without_retry`: `{"dev":"pnpm dev"}` → status `crashed`, message
      contains `orso.json: dev must contain {port}`, and stays `crashed` past `2 × backoff_base`
      (set a short `backoff_base`), with no restart events.
    - `invalid_settings_at_resume`: same file present before `resume_all()` → `crashed` with message.
    - `settings_change_restarts`: with `package.json` and `"install":"true"`, editing `orso.json`'s
      `dev` while ready → a new `starting` then `ready` from the new command.

- [ ] **Step 2: Run** `cargo test --lib devserver:: && cargo test --test devserver` — FAIL.

- [ ] **Step 3: Implement.** In `supervise`, for `Launcher::Configured` call `settings::load(&project.dir)`
  before install/launch; on `Err` set `Crashed` with `settings::describe` and `return` (no backoff,
  crash budget untouched). `watch_deps` and the stale-install branch apply to `Configured`.
  Keep `Custom` and `Disabled` unchanged.

- [ ] **Step 4: Run** the Step 2 commands — PASS. Also `cargo test` overall.

- [ ] **Step 5: Commit** `git commit -m "feat(settings): launch the configured dev command"`

---

### Task 4: Settings API

**Files:**
- Modify: `src/settings.rs` (add `read`/`save`)
- Create: `src/http/settings.rs`
- Modify: `src/http/mod.rs` (`pub mod settings;`, route next to `/api/design` at `:42`)
- Test: `tests/settings.rs` (copy the harness from `tests/design.rs`)

**Interfaces:**
- Consumes: `content::mutation::{project_root, lock_for, require_content_change, resolve, read_current, stale, write_atomic, emit_checkpoint}`,
  `content::lifecycle::{Journal, Undo}`, `git::checkpoint_paths`, `catalog::{current_branch, hash}`.
- Produces:
  - `#[derive(Serialize)] #[serde(rename_all = "camelCase")] pub struct SettingsDoc { pub exists: bool, pub settings: Map<String, Value>, pub defaults: ProjectSettings, pub hash: Option<String>, pub branch: String, pub errors: FieldErrors }`
  - `#[derive(Deserialize)] pub struct SaveSettings { pub project: String, pub branch: String, pub hash: Option<String>, pub settings: Map<String, Value> }`
  - `pub async fn read(state: &AppState, project: &str) -> Result<SettingsDoc, ApiError>`
  - `pub async fn save(state: &AppState, req: SaveSettings) -> Result<SettingsDoc, ApiError>`
  - `GET /api/settings?project=` and `PUT /api/settings` (body `{project, branch, hash?, settings}`).
  - Invalid save → `ApiError::badreq(describe(..)).with_reason("invalid_settings").with_details(json!([{ "path": [field], "message": msg }, …]))`.

- [ ] **Step 1: Write failing integration tests** in `tests/settings.rs`:
  - `get_without_file`: `exists == false`, `settings == {}`, `defaults.dev` is the default,
    `errors == {}`, no `hash`.
  - `get_reports_field_errors`: file `{"dev":"x"}` → 200 with `errors.dev` containing `{port}`;
    file `nope` → 200 with `errors._file`.
  - `save_on_change_writes_and_checkpoints`: on a change branch, PUT
    `{"root":"","install":"pnpm install","custom":1}` → file content parses to
    `{"install":"pnpm install","custom":1}` (empty dropped), latest commit message
    `Updated project settings` touching only `orso.json`, response `exists == true`.
  - `save_keeps_unknown_keys`: existing `{"x":{"y":1},"install":"a"}` → PUT `{"x":{"y":1},"install":"b"}`
    → file keeps `x`.
  - `save_on_main_requires_change`: 4xx with code `CHANGE_REQUIRED`.
  - `save_with_stale_hash`: reason `stale_content`; PUT without hash when file exists → `stale_content`.
  - `save_rejects_invalid`: `{"dev":"x"}` → 400, reason `invalid_settings`, details path `["dev"]`;
    file unchanged.
  - `save_repairs_invalid_json`: file `nope`, PUT with its hash and valid settings → 200.

- [ ] **Step 2: Run** `cargo test --test settings` — FAIL.

- [ ] **Step 3: Implement** following `design::read`/`design::save` (`src/design.rs:118-186`), except an
  existing file that does not parse is overwritable when the hash matches. Write pretty JSON
  (`serde_json::to_string_pretty`) plus trailing newline, mode preserved or `0o644`.

- [ ] **Step 4: Run** `cargo test --test settings` — PASS.

- [ ] **Step 5: Commit** `git commit -m "feat(settings): read and save orso.json over HTTP"`

---

### Task 5: Content paths under `root`

**Files:**
- Modify: `src/content/catalog.rs` (`helper`, `CATALOG_INPUTS`/`catalog_key`, `fallback`)
- Modify: `src/content/lifecycle.rs:87-89` (`asset_dir`), `src/content/images.rs:542,684`
- Test: `tests/content_catalog.rs`, `tests/content_images.rs`

**Interfaces:**
- Consumes: `settings::site_prefix(project: &Path) -> String`, `settings::load` / `site_dir`.
- Produces: no new public API; `lifecycle::asset_dir(prefix: &str, collection: &str, id: &str) -> String`.

- [ ] **Step 1: Write failing tests.**
  - `tests/content_catalog.rs::catalog_under_subfolder_root`: copy the template into `site/`,
    `orso.json` `{"root":"site"}` at the repo root → `list_entries` for `blog` returns paths starting
    with `site/src/content/blog/en/`, and `read_entry` on one returns its source.
  - `tests/content_images.rs::assets_under_subfolder_root`: same layout, an upload for a blog entry
    lands under `site/src/assets/blog/<id>/`.

- [ ] **Step 2: Run** `cargo test --test content_catalog --test content_images` — FAIL.

- [ ] **Step 3: Implement.** Run the helper with `site_dir` as cwd and prefix each collection's
  `loader.base` with `site_prefix` before `finish` (so `supported`, entries, mutation and history
  all keep using repo-relative paths against the project root). Resolve `CATALOG_INPUTS` and
  `fallback` bases under the prefix. Prefix `asset_dir` and the sweep's `src/content` / `src/assets`.
  With no `orso.json` every prefix is `""` — existing tests must pass unchanged.

- [ ] **Step 4: Run** `cargo test` — PASS.

- [ ] **Step 5: Commit** `git commit -m "feat(settings): resolve content under the configured root"`

---

### Task 6: Settings tab (web)

**Files:**
- Modify: `web/src/core/types.ts` (next to `DesignDoc` at `:445`), `web/src/core/api.ts` (next to `design` at `:218`)
- Create: `web/src/modules/settings/store.ts`, `web/src/modules/settings/SettingsPanel.vue`,
  `web/src/modules/settings/store.test.ts`, `web/src/modules/settings/SettingsPanel.test.ts`
- Modify: `web/src/router.ts` (`ProjectTab`, route, `projectTabFromRoute`), `web/src/router.test.ts`,
  `web/src/app/ProjectRail.vue:15-20`, `web/src/app/ProjectRail.test.ts`

**Interfaces:**
- Consumes: `GET/PUT /api/settings` from Task 4; `NewChangeDialog`; the projects store's devserver status.
- Produces:
  - `export interface ProjectSettings { root: string; installDir: string; install: string; dev: string }`
  - `export interface SettingsDoc { exists: boolean; settings: Record<string, unknown>; defaults: ProjectSettings; hash?: string; branch: string; errors: Record<string, string> }`
  - `api.settings(project: string): Promise<SettingsDoc>`;
    `api.saveSettings(body: { project: string; branch: string; hash?: string; settings: Record<string, unknown> }): Promise<SettingsDoc>`
  - `useSettingsStore` with `doc, draft, dirty, loading, saving, error, conflict, needsChange, changedOnDisk, fieldErrors, open, setField, save, resumeSave, discard, reload, reloadFromDisk, refreshForBranch, canLeave`
    — same semantics as `useDesignStore` (`web/src/modules/design/store.ts`); `draft` holds the four
    known fields as strings (`""` = unset) merged over the file's unknown keys on save;
    `fieldErrors` comes from `doc.errors`, overridden by `invalid_settings` error details.
  - Route `project-settings` at `settings`, `ProjectTab` includes `"settings"`.

- [ ] **Step 1: Write failing tests.**
  - `store.test.ts`: open loads; `setField("dev", …)` makes `dirty`; save sends only set fields plus
    unknown keys and hash; `CHANGE_REQUIRED` → `needsChange`, `resumeSave("change/x")` retries on that
    branch; `stale_content` → `conflict`; `invalid_settings` details → `fieldErrors.dev`; an
    `fs.change` event for path `orso.json` (or `checkpoint.created`) with a dirty draft and a new hash
    → `changedOnDisk`, with a clean draft → reloaded.
  - `SettingsPanel.test.ts`: four inputs whose placeholders are the defaults; a field error renders
    under its input; on main the banner text is exactly
    "Settings can't be edited on main. Create a change."; with `doc.errors` non-empty the banner text is
    "orso.json has errors — the dev server won't start until they're fixed."; a dev command containing
    `astro dev` without `--ignore-lock` shows the hint; `dev` without `{port}` disables Save.
  - `router.test.ts`: `projectTabFromRoute("project-settings") === "settings"`,
    `projectPath("a","settings") === "/p/a/settings"`.
  - `ProjectRail.test.ts`: a Settings tab is rendered last and links to `/p/<id>/settings`.

- [ ] **Step 2: Run** `pnpm -C web test` — FAIL.

- [ ] **Step 3: Implement.** Copy the design store structure; reuse `NewChangeDialog` exactly as
  `DesignPanel.vue:56-58` does. Add `{ id: "settings", label: "Settings", icon: Settings }` (lucide) as
  the last rail tab. Show the project's devserver status (`starting` / `ready` / `crashed` + message)
  under the form after a save. Chat stays hidden on this tab (`ProjectPage.vue:35` unchanged).

- [ ] **Step 4: Run** `pnpm -C web test` and `pnpm -C web build` — PASS.

- [ ] **Step 5: Commit** `git commit -m "feat(web): project settings tab"`

---

### Task 7: Full verification

- [ ] **Step 1:** `just test` — Expected: all Rust, web and template suites pass.
- [ ] **Step 2:** Manual: in a project, create a change, set `dev` to
  `npm run dev -- --port {port} --host {host} --ignore-lock --verbose`, save → dev server restarts
  and preview loads; set `dev` to `npm run dev` → Save disabled with the `{port}` error.
