# orso-astro → `integrations/orso` copy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Orso Astro plugin as copied sources under `integrations/orso/` instead of a local `file:` npm package, so scaffolds need no npm token or `packages/astro` tree.

**Architecture:** Canonical runtime stays in `packages/astro/src/`. A sync recipe copies those `.js` files into `templates/astro-marketing/integrations/orso/` for in-repo template work. Scaffold extract embeds `packages/astro/src/` (flattened) into `<project>/integrations/orso/`, excludes that folder from the Template embed, and drops all `orso-astro` / `file:` manifest rewrites. Template imports use relative paths; YAML/jsonc deps live on the project.

**Tech Stack:** Rust (`rust-embed`, existing `src/static.rs`), Astro template, npm lockfile, just.

**Spec:** `docs/superpowers/specs/2026-11-07-orso-astro-integrations-copy-design.md`

## Global Constraints

- Target folder name is exactly `integrations/orso/` (flattened `*.js`, no nested `src/`).
- Runtime files: `index.js`, `manifest.js`, `patch.js`, `endpoint.js`, `bridge.js`.
- Hoisted deps (exact ranges from current `packages/astro/package.json`): `js-yaml` `^4.3.2`, `jsonc-parser` `^3.3.1`, `yaml` `^2.9.1`.
- No `orso-astro` dependency; no npm publish; no migration of existing user projects.
- `packages/astro/` remains the unit-test / authoring home (`npm --prefix packages/astro test` unchanged).

## Review Focus

- Template embed still contains `integrations/orso/**` → extract and template disagree; exclude it from `Template` and assert absence in `template_paths()`.
- Extract leaves `orso-astro` or `file:packages/astro` in manifests → assert neither in `package.json` / lock after extract.
- Drift: template copy ≠ `packages/astro/src` → sync-drift test fails until `just sync-orso-integration`.
- `.npmrc` `install-links=true` left in place with no local package → remove file (or line) and drop the template assertion that required `.npmrc` if the file is gone.
- Flattened `readFileSync(new URL("./bridge.js", import.meta.url))` breaks if `bridge.js` is omitted from embed → include `bridge.js` in embed want-list.

## File map

| File | Role |
| --- | --- |
| `scripts/sync-orso-integration.sh` | Copy `packages/astro/src/*.js` → template `integrations/orso/` |
| `justfile` | `sync-orso-integration` recipe |
| `templates/astro-marketing/integrations/orso/*.js` | Synced committed copy |
| `templates/astro-marketing/{astro.config.mjs,src/content.config.ts,package.json,package-lock.json}` | Relative imports; hoist deps; drop `orso-astro` |
| `templates/astro-marketing/.npmrc` | Delete if only `install-links=true` |
| `src/static.rs` | Embed `packages/astro/src/`; extract to `integrations/orso`; exclude template copy; drop rewrites; tests |
| `tests/projects.rs`, `tests/content_e2e.rs` | Path/dep assertions for ignored e2e |
| `packages/astro/README.md`, `skills/setup-orso/SKILL.md`, `templates/astro-marketing/AGENTS.md` | Document copy model |

---

### Task 1: Sync recipe + committed template copy + drift test

**Files:**
- Create: `scripts/sync-orso-integration.sh`
- Create: `templates/astro-marketing/integrations/orso/{index,manifest,patch,endpoint,bridge}.js` (via script)
- Modify: `justfile` (add recipe)
- Modify: `src/static.rs` (add drift test under `#[cfg(test)]`)

**Interfaces:**
- Consumes: `packages/astro/src/*.js`
- Produces: byte-identical files at `templates/astro-marketing/integrations/orso/*.js`; `just sync-orso-integration` runs the script

- [ ] **Step 1: Write the failing drift test** in `src/static.rs`

```rust
#[test]
fn template_integrations_orso_matches_packages_astro_src() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for name in ["index.js", "manifest.js", "patch.js", "endpoint.js", "bridge.js"] {
        let canonical = std::fs::read(root.join("packages/astro/src").join(name)).unwrap();
        let templated = std::fs::read(
            root.join("templates/astro-marketing/integrations/orso").join(name),
        );
        assert!(
            templated.as_ref().ok() == Some(&canonical),
            "{name} missing or drifted; run: just sync-orso-integration"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test template_integrations_orso_matches_packages_astro_src -- --exact --nocapture`  
Expected: FAIL (directory/files missing)

- [ ] **Step 3: Add `scripts/sync-orso-integration.sh` and `just sync-orso-integration`**

Script: `mkdir -p` template dir; `cp` each of the five `.js` files from `packages/astro/src/` (repo-root relative). Recipe: `./scripts/sync-orso-integration.sh`. Run the recipe once to create the committed copy.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test template_integrations_orso_matches_packages_astro_src -- --exact`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add scripts/sync-orso-integration.sh justfile \
  templates/astro-marketing/integrations/orso src/static.rs
git commit -m "$(cat <<'EOF'
chore: sync orso integration sources into template integrations/orso

EOF
)"
```

---

### Task 2: Template imports, deps, lockfile

**Files:**
- Modify: `templates/astro-marketing/astro.config.mjs`
- Modify: `templates/astro-marketing/src/content.config.ts`
- Modify: `templates/astro-marketing/package.json`
- Modify: `templates/astro-marketing/package-lock.json`
- Delete: `templates/astro-marketing/.npmrc` (if it only contains `install-links=true`)

**Interfaces:**
- Consumes: Task 1’s `integrations/orso/index.js`
- Produces: `import { orso } from "./integrations/orso/index.js"`; `import { orsoGlob } from "../integrations/orso/index.js"`; root deps include the three hoisted packages and no `orso-astro`

- [ ] **Step 1: Write failing assertions** (extend later Task 3 extract test expectations conceptually — for this task, change template files then verify with npm)

No new Rust test yet. After edits, `npm --prefix templates/astro-marketing ci` must succeed without `packages/astro` as a dependency.

- [ ] **Step 2: Switch imports**

`astro.config.mjs`: `import { orso } from "./integrations/orso/index.js";`  
`src/content.config.ts`: `import { orsoGlob } from "../integrations/orso/index.js";`

- [ ] **Step 3: Update `package.json`**

Remove `"orso-astro": "file:../../packages/astro"`. Add `"js-yaml": "^4.3.2"`, `"jsonc-parser": "^3.3.1"`, `"yaml": "^2.9.1"`. Delete `.npmrc` if only `install-links=true`.

- [ ] **Step 4: Refresh lockfile and verify install**

Run: `npm --prefix templates/astro-marketing install`  
Then: `npm --prefix templates/astro-marketing ci`  
Expected: clean install; lockfile has no `orso-astro` / `file:../../packages/astro`

- [ ] **Step 5: Commit**

```bash
git add templates/astro-marketing
git commit -m "$(cat <<'EOF'
refactor(template): import orso from integrations/orso; drop file: package

EOF
)"
```

---

### Task 3: Rust embed, extract, and unit tests

**Files:**
- Modify: `src/static.rs` (module docs, `Template` excludes, `OrsoAstro` folder, `extract_template`, remove `ORSO_ASTRO_*`, update tests)

**Interfaces:**
- Consumes: `packages/astro/src/*.js` via `RustEmbed`
- Produces: `extract_template(dest)` writes those files under `dest/integrations/orso/`; no manifest rewrite; `orso_astro_paths()` lists flat names (`index.js`, …)

- [ ] **Step 1: Rewrite failing unit tests** in `src/static.rs`

- `orso_astro_contains_package_sources_only`: want `index.js`, `manifest.js`, `patch.js`, `endpoint.js`, `bridge.js`; assert none of `package.json`, `README.md`, `src/…`, `test/`, `node_modules/`.
- `template_drops_the_legacy_helper_and_schema_registry`: assert `orso_astro_paths()` contains `index.js` (flat).
- `template_does_not_embed_orso_astro`: assert no path starts with `integrations/orso` or `packages/astro`; drop `.npmrc` from the required want-list if deleted in Task 2.
- Rename/replace `extract_template_copies_orso_astro_and_rewrites_manifests` → `extract_template_copies_integrations_orso`:

```rust
assert!(tmp.path().join("integrations/orso/index.js").is_file());
assert!(tmp.path().join("integrations/orso/bridge.js").is_file());
assert!(!tmp.path().join("packages/astro").exists());
let manifest = std::fs::read_to_string(tmp.path().join("package.json")).unwrap();
let json: serde_json::Value = serde_json::from_str(&manifest).unwrap();
assert!(json["dependencies"].get("orso-astro").is_none());
for name in ["js-yaml", "jsonc-parser", "yaml"] {
    assert!(json["dependencies"][name].is_string(), "{name}");
}
let lock = std::fs::read_to_string(tmp.path().join("package-lock.json")).unwrap();
for text in [&manifest, &lock] {
    assert!(!text.contains("orso-astro"));
    assert!(!text.contains("file:../../packages/astro"));
    assert!(!text.contains("file:packages/astro"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib static::tests`  
Expected: FAIL on embed paths / extract assertions

- [ ] **Step 3: Implement embed + extract**

- `Template`: add `#[exclude = "integrations/orso/**"]`.
- `OrsoAstro`: `#[folder = "packages/astro/src/"]` (keep harmless excludes as needed).
- Remove `ORSO_ASTRO_REPO_SPEC` / `ORSO_ASTRO_SCAFFOLD_SPEC` and the rewrite loop.
- `extract_template`: `extract_embed::<OrsoAstro>(&safe_join(dest, "integrations/orso")?)?;` only (after Template).
- Update module docs to describe `integrations/orso` copy, not `file:` package.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib static::tests`  
Expected: PASS  
Also: `npm --prefix packages/astro test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/static.rs
git commit -m "$(cat <<'EOF'
refactor(scaffold): extract orso sources into integrations/orso

EOF
)"
```

---

### Task 4: Docs, skill, and e2e assertions

**Files:**
- Modify: `packages/astro/README.md` (Install → copied to `integrations/orso/`, relative imports)
- Modify: `skills/setup-orso/SKILL.md` (vendor into `integrations/orso/`; relative imports; hoist deps; no `file:packages/astro`)
- Modify: `templates/astro-marketing/AGENTS.md`
- Modify: `tests/projects.rs` (`real_npm_install_and_site_builds` assertions ~500–509)
- Modify: `tests/content_e2e.rs` (assert `integrations/orso/index.js`)
- Modify: `justfile` comments that say scaffolds use `packages/astro` as a path dep, if any

**Interfaces:**
- Consumes: Tasks 1–3 behavior
- Produces: docs/skill match the copy model; ignored e2e asserts new paths/deps

- [ ] **Step 1: Update e2e assertions**

`tests/projects.rs`: `dir.join("integrations/orso/index.js")`; `package["dependencies"].get("orso-astro")` is none; deps include `js-yaml` / `jsonc-parser` / `yaml`; lock/manifest must not contain `file:packages/astro` or `file:../../packages/astro`.  
`tests/content_e2e.rs`: `integrations/orso/index.js`.

- [ ] **Step 2: Update README, skill, AGENTS** to relative imports and `integrations/orso/` overwrite/vendor model (sparse-checkout can still pull `packages/astro/src` and copy into `integrations/orso`).

- [ ] **Step 3: Run focused verification**

Run: `cargo test --lib static::tests && npm --prefix packages/astro test && npm --prefix templates/astro-marketing ci`  
Expected: all PASS  
Optional: `cargo test --test projects real_npm_install_and_site_builds -- --ignored --exact` if network/time allow.

- [ ] **Step 4: Commit**

```bash
git add packages/astro/README.md skills/setup-orso/SKILL.md \
  templates/astro-marketing/AGENTS.md tests/projects.rs tests/content_e2e.rs justfile
git commit -m "$(cat <<'EOF'
docs: describe integrations/orso copy; update scaffold e2e assertions

EOF
)"
```

---

## Spec coverage (self-review)

| Spec requirement | Task |
| --- | --- |
| Copy into `integrations/orso/` | 1, 3 |
| Relative imports | 2 |
| Hoist yaml/js-yaml/jsonc-parser | 2, 3 tests |
| Remove `orso-astro` / rewrites | 2, 3 |
| Canonical `packages/astro/` + sync + drift | 1 |
| Exclude template copy from Template embed | 3 |
| Docs/skill | 4 |
| E2e path updates | 4 |
| Non-goals (publish, migrate existing) | honored / out of plan |
