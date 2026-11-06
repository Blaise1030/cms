# Skills module — design

Date: 2026-11-06
Status: approved design (chat), spec for review
Scope: daemon skills APIs + agent index injection, web Skills rail tab (sidebar +
markdown editor + Install dialog), App Settings for global skills path. No remote
marketplace.

## Problem

Writers and agents share `AGENTS.md` and `DESIGN.md`, but there is no first-class way to
install reusable guidance packs (SEO, design habits, brand voice) into a project or across
projects. Today that knowledge is either buried in chat, hardcoded in Orso’s guide, or left
to each user to invent ad hoc files agents may never see.

## Goal

A **Skills** tab where the user installs skill packages (built-in catalog or custom
import), edits each skill’s `SKILL.md` in a markdown editor with an installed-skills
sidebar, and has agents automatically discover installed skills via a short index in the
system prompt.

Success: user installs “SEO” into a project, tweaks the markdown, starts a change chat, and
the agent sees the skill in its index and reads `.orso/skills/seo/SKILL.md` when the task
matches.

## Decisions

| # | Decision |
| --- | --- |
| 1 | Install means copying a skill package onto disk (project or global) so agents follow it — not a separate SEO-audit-style capability tab. |
| 2 | Skills live in the current project and/or a configurable global folder. |
| 3 | v1 ships a small built-in catalog and supports custom import / blank create. |
| 4 | Agents get a short installed-skills index in the system prompt; full bodies stay on disk for on-demand reads. |
| 5 | UI: sidebar lists installed skills only; main pane edits `SKILL.md`; Install opens a dialog (catalog + import). |
| 6 | Approach: Design-module twin — new rail tab, reuse `MarkdownBodyEditor`, Design-like change gating for project skills. |
| 7 | Project skill writes require an active change and checkpoint; global skill writes go to the app config dir with no project change. |
| 8 | No remote marketplace, skill versioning, or auto-run workflows in v1. The SEO audit tab (if built) stays separate. |

## Package format

Anthropic-style skill folder:

```text
<skills-root>/<id>/
  SKILL.md          # required
  # optional assets ignored by the editor in v1
```

`SKILL.md` requires YAML frontmatter:

- `name` — display name
- `description` — one-line summary (used in the agent index and sidebar)

Body is freeform markdown instructions for the agent. Unknown frontmatter keys survive a
save. Invalid packages (missing file or required keys) are skipped in list/index and shown
as errors when selected.

### On-disk roots

| Scope | Path |
| --- | --- |
| Project | `<project-root>/.orso/skills/<id>/` (inside the site repo) |
| Global | configurable; default under the app config dir, shown as `~/.orso/skills/<id>/` |

Do **not** store project skills under `{projects-dir}/.orso/` — that directory is Orso app
state (`projects.json`, trash, etc.), not part of a site checkout. Project skills must live
in the project git tree so change gating and checkpoints apply.

`<id>` is a stable slug (catalog id, or derived from import name). Collision on install in
the same scope: `409` refuse (replace-on-install is a follow-up).

### Built-in catalog (v1)

Static templates embedded in the daemon (same idea as Design’s starter `DESIGN.md`):

- **seo** — on-page SEO habits for content and templates
- **design** — visual/identity guidance complementary to `DESIGN.md` (does not replace the
  Design module)

Install copies the template tree into the chosen scope. Catalog is not fetched over the
network.

## Architecture

```text
ProjectRail → Skills tab
                ├─ sidebar: installed (project / global)
                ├─ main: MarkdownBodyEditor (SKILL.md)
                └─ Install dialog → catalog | import | blank

Daemon /api/skills*  →  read/write .orso/skills + global dir
providers::agent_guide + skills_index(project) → system prompt
App Settings → global skills folder path
```

### Module layout

**Daemon**

- `src/skills/mod.rs` — types, list/merge scopes, frontmatter parse, index renderer,
  catalog embeds
- `src/http/skills.rs` — routes
- Extend `src/settings.rs` (or app settings store) for `skills.global_path`
- Extend agent launch in `src/providers.rs` (or the call site that builds the guide) to
  append `skills_index` for the current project

**Web**

- `web/src/modules/skills/` — `SkillsPanel.vue`, store, install dialog, tests
- Rail + router + command palette: new `skills` tab
- Reuse `MarkdownBodyEditor`; follow Design panel save/discard / change-gate UX
- App Settings section for global path

## Backend

### List / merge

`GET /api/skills?project=` returns:

- `installed[]`: `{ id, name, description, scope: "project" | "global", path }`
- `catalog[]`: `{ id, name, description, installedProject, installedGlobal }`

Project and global can both contain the same `id`. They are distinct rows (same id, different
scope). The agent index lists both with scope labels. Sidebar groups by scope.

### Read

`GET /api/skills/:id?project=&scope=project|global` → `{ name, description, body, hash,
path }`. Prefer explicit `scope`; required when both exist.

### Write

`PUT /api/skills/:id` body: `{ project?, branch?, scope, name?, description?, body, hash }`

- **project:** `require_content_change`, stale-hash check, atomic write of `SKILL.md`,
  `checkpoint_paths` for that file (and new dirs on create)
- **global:** write under global root; no branch/change; still hash/conflict if desired for
  multi-window safety

Frontmatter is re-serialized from `name` / `description` (and preserved unknown keys) plus
body — same preservation spirit as Design.

### Install

`POST /api/skills/install`

```json
{
  "project": "...",
  "branch": "...",
  "scope": "project" | "global",
  "source": { "catalog": "seo" }
           | { "import": { "files": [...] } }
           | { "blank": { "name": "...", "description": "..." } }
}
```

- Catalog: copy embedded template to target `<id>/`
- Import: validate `SKILL.md` frontmatter; slugify id; write files
- Blank: stub `SKILL.md` with frontmatter + short placeholder body
- Project scope: change-gated + checkpoint
- Global: no change gate

### Uninstall

`DELETE /api/skills/:id?project=&scope=&branch=` — delete the skill directory; same gating
as write.

### Settings

Global skills directory:

- Default: platform config dir / `skills` (document as `~/.orso/skills` in UX copy)
- `GET`/`PUT` via existing app settings surface; validate path is absolute and writable on
  save; expose **Reset to default**

### Agent index

When building guided-agent instructions for a project, append:

```markdown
## Installed skills
- **seo** (project): … — `.orso/skills/seo/SKILL.md`
- **voice** (global): … — /absolute/or/tilde/path/voice/SKILL.md

Read a skill's SKILL.md when the task matches its description. Do not load every skill up front.
```

Scan both roots each launch (cheap: small dirs). Skip invalid packages. If none installed,
omit the section entirely.

Cursor rule / OpenCode instruction files that already embed `agent_guide()` must include
the same dynamic index for that project (regenerate when the agent session starts, same as
today’s guide write path).

## Frontend

### Rail & routing

- New `ProjectTab`: `skills`
- `ProjectRail` entry (icon + label “Skills”)
- Route under `/p/:id/skills` (match existing child-route pattern)
- Command palette: switch to Skills; “Install skill…”
- Chat visibility: same as Design/Content (chat shown)

### SkillsPanel layout

- Header: title, **Install…**, Save, Discard, Uninstall (when selected)
- Left sidebar (`SidebarProvider` + resizable): groups **This project** / **Global**;
  search when list is long; row shows name + description snippet + scope badge
- Context menu: Uninstall; Duplicate to project (global → project copy, change-gated);
  Reveal in folder on desktop if already patterned elsewhere
- Main: skill title + path subtitle; `MarkdownBodyEditor`
- Dirty state; project + no change → Save disabled with Design’s “start a change” pattern;
  global → Save always enabled
- Empty: no skills → CTA to Install; no selection → “Select a skill or install one”
- Missing/invalid `SKILL.md`: error + Remove

### Install dialog

- Segments: **Catalog** | **Import**
- Catalog cards with Install; disabled when already present in the chosen scope
- Scope picker: This project (default) | Global
- Import: file/folder picker or paste path; validate before enable Install
- Optional **Create blank** on Import (or tertiary action): name + description → blank
  skill, then select it in the editor

## Errors

| Case | Behavior |
| --- | --- |
| Project write/install/uninstall without active change | Same error shape as Design (`require_content_change`) |
| Stale hash | Conflict; reload or overwrite policy matching Design |
| Invalid import / missing frontmatter | `400` with plain message |
| Unknown catalog id | `404` |
| Id already installed in scope | `409` |
| Global path missing / unwritable | Settings validation error; list global as empty with banner |

## Testing

- **Unit (Rust):** frontmatter parse; list merge; index renderer; catalog copy; gating
  branches for project vs global
- **API:** install/save/uninstall for both scopes; conflict/stale; settings path
- **Unit (web):** store dirty/hash; sidebar grouping; install dialog scope + already-installed
- **Component:** empty states; Save disabled without change for project skills

## Out of scope (v1)

- Remote marketplace, updates, signing, or paid skills
- Skill dependencies / version pins
- In-panel editing of non-markdown assets
- Replacing Design module or `DESIGN.md` with a skill
- Auto-invoking skills as structured workflows (audits, linters)
- Syncing global skills via git or team sharing

## Open follow-ups (not blocking v1)

- Replace-on-install and catalog version bumps
- “Apply skill” chat outbox helper (pre-filled “follow skill X for this change”)
- Team-wide global skills sync (beyond “same machine, same global folder”)
- Ensure new project templates never gitignore `<project>/.orso/skills/` (current Astro
  template does not)
