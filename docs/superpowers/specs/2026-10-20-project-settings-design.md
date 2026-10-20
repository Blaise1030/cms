# Project settings (orso.json) — design

Date: 2026-10-20

## Goal

Give each project a **Settings** tab that defines where the Astro project lives in the repo and which
commands install its dependencies and run its dev server. The settings are saved as `orso.json` at the
repo root and committed, so every collaborator and clone gets the same configuration.

Today orso assumes the Astro project is the repo root and uses npm: `npm install` and
`npm run dev -- --port N --host 127.0.0.1 --ignore-lock` are hardcoded, and content, images and template
checks resolve `src/...` against the repo root.

### What the user asked for

- Support repos where the Astro site lives in a subfolder (monorepos, e.g. `apps/web`).
- Support pnpm, yarn and bun as well as npm.
- Support dev scripts that are not named `dev`, or that need extra flags or environment.
- Store the settings as a file at the repo root.
- Edits are committed and go through a change, like `DESIGN.md`.
- The install folder is configurable separately from the Astro folder.
- The dev command receives the port through placeholders.

### Assumptions

- `DESIGN.md` and `orso.json` stay at the repo root even when the Astro project is in a subfolder.
- `{host}` is always `127.0.0.1`: the preview proxy dials IPv4 loopback.
- Commands run through `sh -c`, so users can chain commands and set environment inline.

### Out of scope (v1)

- A dedicated agent tool for editing settings (agents edit `orso.json` as a normal file).
- Per-machine (uncommitted) overrides.
- Auto-detecting the package manager to prefill commands.
- More than one Astro project per repo.

## The file

`orso.json` at the repo root. The file and every field are optional.

```json
{
  "root": "apps/web",
  "installDir": ".",
  "install": "pnpm install",
  "dev": "pnpm dev --port {port} --host {host}"
}
```

| Field        | Meaning                                         | Default                                                  |
| ------------ | ----------------------------------------------- | -------------------------------------------------------- |
| `root`       | Astro project folder, relative to the repo root | `.`                                                      |
| `installDir` | Folder the install command runs in              | `.`                                                      |
| `install`    | Install command                                 | `npm install --no-audit --no-fund --no-progress`         |
| `dev`        | Dev-server command                              | `npm run dev -- --port {port} --host {host} --ignore-lock` |

With no file, behaviour is identical to today.

### Validation

- `root` and `installDir` are relative, contain no `..` component that escapes the repo, and exist as
  directories.
- `root` contains an `astro.config.*` file.
- `install` and `dev` are non-empty strings; `dev` contains `{port}`.
- The file is a JSON object; a known field with the wrong type is an error.
- Unknown keys are preserved on save (order-preserving object, as in `DESIGN.md` tokens).

Validation returns per-field messages so the UI can point at the broken field.

### Placeholders

`{port}` and `{host}` are substituted in `dev` before it runs. They are the only placeholders; any
other `{...}` text is passed through unchanged.

## Backend

### `src/settings.rs`

- `pub const SETTINGS_FILE: &str = "orso.json";`
- `ProjectSettings { root, install_dir, install, dev }`: resolved values with defaults applied, plus
  absolute `root_dir()` and `install_dir()` helpers.
- `load(project_dir) -> Result<ProjectSettings, SettingsError>`: read, parse, apply defaults, validate.
  `SettingsError` carries per-field messages and converts to an `ApiError`.
- `dev_command(&settings, port) -> String`: placeholder substitution.
- `read` / `save` for the API, following `design.rs` (see API below).

The file is the only source of truth: no copy lives in the project registry. Each launch and install
reads the working tree of the current branch, so switching branches or changes takes effect on the next
restart.

### Dev server (`src/devserver.rs`)

- `Launcher::Npm` becomes `Launcher::Configured`. On every start it calls `settings::load`, builds
  `sh -c "<dev command>"`, and runs it with `current_dir = root`. Process group, ready-line detection,
  crash restarts with backoff, `PUBLIC_BASE` and `ORSO_PROXY_PORT` are unchanged.
- If `load` fails, the project goes to `crashed` with the message (e.g.
  `orso.json: dev must contain {port}`) and the crash budget is not spent on retries: a bad file is not
  fixed by retrying. A manual restart or a settings change retries.
- `Launcher::Custom` (test hook) and `Launcher::Disabled` are kept.

### Install (`src/projects.rs`)

- `npm_install(dir)` becomes `install(project_dir)`: loads settings, runs `sh -c "<install>"` in
  `installDir` with the existing timeout and stderr-tail error. Project creation and clone use it.
- Error messages name the command that ran (`pnpm install failed (...)`), not `npm`.

### Dependency-change detection

The fingerprint covers:

- `package.json` in `installDir` and in `root`;
- the lockfile present in `installDir`: `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`
  or `bun.lockb`;
- `orso.json`.

The marker moves to `<installDir>/node_modules/.orso-deps`. The 3-second poll stays; a changed
fingerprint stops the server, reinstalls and restarts it. Because `orso.json` is in the fingerprint, a
settings save, branch switch or agent edit restarts the dev server through the same path.

If `orso.json` does not validate, the fingerprint still uses the default `installDir` so the poll keeps
running and notices when the file is fixed.

### Content, images and template checks

`src/content/catalog.rs`, `src/content/images.rs` and the template checks in `src/static.rs` resolve
their `src/...` paths against `root`. Paths in API responses stay relative to the repo root, so the
file tree, checkpoints and change diffs keep working unchanged.

### API

`GET` and `PUT /api/settings?project=…`, mirroring `/api/design`.

`GET` returns:

```json
{
  "exists": true,
  "settings": { "root": "apps/web", "dev": "pnpm dev --port {port} --host {host}" },
  "defaults": { "root": ".", "installDir": ".", "install": "...", "dev": "..." },
  "hash": "…",
  "branch": "change/…",
  "errors": { "dev": "must contain {port}" }
}
```

`settings` is the file as written (only fields the user set, plus unknown keys). `errors` is empty when
the file validates; a file that is not valid JSON returns `errors: { "_file": "…" }` rather than failing
the request.

`PUT` takes `{ settings, hash, branch }` and:

1. requires an active change on `branch` (`require_content_change`);
2. checks `hash` against the file on disk (`stale` on mismatch; no hash means the file must not exist);
3. validates `settings` and rejects invalid ones with per-field errors, so the UI cannot commit a bad
   config;
4. writes atomically, checkpoints `orso.json` as "Updated project settings" with the usual undo journal;
5. returns the fresh `GET` document.

Empty-string fields from the form are dropped before writing, so the file only holds what the user set.

## Web

### Route and tab

- `ProjectTab` gains `"settings"`; route `project-settings` at `/p/:id/settings`;
  `projectTabFromRoute` maps it.
- `ProjectRail.vue` adds a last tab, **Settings**, with a gear icon.

### `web/src/modules/settings/`

- `store.ts`: same shape as the design store: `doc`, `draft`, `dirty`, `needsChange`, `changedOnDisk`
  (from `fs.change` events for `orso.json`), stale-hash handling, `resumeSave(branch)` after
  `NewChangeDialog`.
- `SettingsPanel.vue`: a form with four fields: Astro project folder, Install folder, Install command,
  Dev command.
  - Empty fields show the default as placeholder text and save as unset.
  - Server errors appear under their field.
  - The dev command field explains `{port}` and `{host}`, and shows a soft hint when the command
    appears to run Astro without `--ignore-lock`.
  - Banner when the change is required: "Settings can't be edited on main. Create a change." (same flow
    as Design).
  - Banner when the file on disk does not validate: "orso.json has errors — the dev server won't start
    until they're fixed."
  - After a save, a dev-server status line (starting → ready / crashed with message).

## Testing

Rust unit tests (`src/settings.rs`, `src/devserver.rs`):

- defaults when the file or a field is missing;
- path validation: absolute, escaping `..`, missing directory, `root` without `astro.config.*`;
- `dev` without `{port}`; wrong field types; invalid JSON;
- unknown keys survive a save;
- placeholder substitution;
- fingerprint changes for each lockfile kind, for `package.json` in `root` and `installDir`, and for
  `orso.json`.

Rust integration tests (`tests/`):

- settings API: get with and without a file, save on a change, rejection on main, stale hash, invalid
  body;
- dev server launched from a configured command through `Launcher::Custom`, and `crashed` without
  retries on an invalid file;
- content catalog listing entries from a subfolder `root`.

Vitest:

- settings store: load, dirty and save, needs-change and resume, stale hash, changed on disk;
- `SettingsPanel`: field errors, placeholders show defaults, banners;
- router and `ProjectRail` tab mapping.
