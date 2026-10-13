# Design module (DESIGN.md) — design

Date: 2026-10-13

## Goal

Give each project a **Design** tab where the user describes the site's visual identity, saved as
`DESIGN.md` at the project root. The file is the design source of truth for coding agents, the same
way `AGENTS.md` is for conventions.

- The user writes the first draft through a form (tokens) and the tiptap Markdown editor (prose).
- After that, the agent keeps `DESIGN.md` and `src/styles/global.css` `@theme` in sync. The user can
  ask it to do so with **Apply to site**.

### What the user asked for

- Form drives the first draft; the agent keeps `DESIGN.md` and `@theme` in sync afterwards.
- v1 covers brand basics, colours, typography, shape and spacing, and voice and tone.
- Editing reuses the tiptap Markdown editor.
- Follow the standard `DESIGN.md` format rather than a custom one.

### Assumptions

- `DESIGN.md` lives at the project root, next to `AGENTS.md`.
- It describes the generated Astro site, not the workbench app's own UI.

### Out of scope (v1)

- Linting with `@google/design.md` (it would add a Node dependency to the save path).
- Editing `components` tokens in the form (they are kept, not shown).
- Images in `DESIGN.md`.
- Preserving YAML comments on save.
- The daemon editing `global.css` itself. Only the agent does that.

## File format

Follows Google's open [DESIGN.md spec](https://github.com/google-labs-code/design.md/blob/main/docs/spec.md),
version `alpha`.

- **YAML frontmatter** holds the normative tokens: `version`, `name`, `description`, `colors`,
  `typography`, `rounded`, `spacing`, `components`.
  - Token names are free-form (`colors.primary`, `typography.h1`, `rounded.md`).
  - Values are real CSS values: colours (`"#4f46e5"`, `oklch(...)`), dimensions (`16px`, `1rem`).
  - Typography tokens have `fontFamily`, `fontSize`, `fontWeight`, `lineHeight`, and optionally
    `letterSpacing`, `fontFeature`, `fontVariation`.
  - References use `{path.to.token}`.
- **Markdown body** holds the rationale in `##` sections, in this order when present: Overview,
  Colors, Typography, Layout, Elevation & Depth, Shapes, Components, Do's and Don'ts.
  - Voice and tone has no spec section; we add `## Voice & Tone` after Do's and Don'ts. The spec
    allows unknown sections and requires tools to keep them.
- Unknown token names, token groups and sections must survive a save.

How the requested fields map to the spec:

| Requested | Spec |
|---|---|
| Brand basics | `name`, `description`, `## Overview` |
| Colours | `colors.*` |
| Typography | `typography.*` |
| Shape and spacing | `rounded.*`, `spacing.*`, `## Elevation & Depth`, `## Shapes`, `## Layout` |
| Voice and tone | `## Voice & Tone` |

### Starter draft

Used by **Create design** and shipped as `templates/astro-marketing/DESIGN.md`, with values taken
from what the template's components already use (Tailwind defaults):

- `version: alpha`, `name` and `description` from the template's `src/site.ts`.
- `colors`: `primary`, `secondary`, `tertiary`, `neutral`.
- `typography`: `h1`, `h2`, `body-md`, `label`.
- `rounded`: `sm`, `md`, `lg`, `full`.
- `spacing`: `sm`, `md`, `lg`.
- Body: the spec's section headings in order plus `## Voice & Tone`, each with a one-line prompt for
  the user to replace.

The frontend's `starter.ts` and the template file hold the same starter; a test keeps them equal.

## Backend

New `src/design.rs` (logic) and `src/http/design.rs` (handlers), routes registered in
`src/http/mod.rs`.

### `GET /api/design?project=<id>`

Returns `{ exists, tokens, body, hash, branch }`.

- `tokens` is the frontmatter parsed to JSON (an empty object when there is no frontmatter).
- `body` is everything after the closing `---`.
- `hash` is the content hash used by content entries (`catalog::hash`).
- `branch` is the branch the file was read on.
- Missing file: `{ exists: false, branch }`.
- Frontmatter that is not valid YAML, or not a mapping: `400` with reason `invalid_design` and the
  raw source, so the tab can show it read-only and point the user to the agent. The file is never
  overwritten in this state.

### `PUT /api/design`

Body: `{ project, branch, hash?, tokens, body }`.

1. Take the per-project lock (`lock_for`).
2. `require_content_change(root, branch)`. On a protected branch this returns `CHANGE_REQUIRED`,
   the same as content saves.
3. Without `hash`: the file must not exist, otherwise `stale()`. With `hash`: the current file's
   hash must match, otherwise `stale()`.
4. `tokens` must be a JSON object, otherwise `400`.
5. Render `---\n<yaml>---\n<body>` with `serde_yaml` (block style, keys in received order), keeping
   the file mode on update, and write it with `write_atomic`.
6. `git::checkpoint_paths(root, "Updated design", ["DESIGN.md"])` and `emit_checkpoint`.
7. Return the same shape as `GET`.

The shared helpers in `src/content/mutation.rs` (`lock_for`, `require_content_change`, `stale`,
`write_atomic`, `project_root`, `read_current`, `emit_checkpoint`) are `pub(super)` today; they
become `pub(crate)` so `src/design.rs` can use them.

Key order: if `serde_json` is not built with `preserve_order`, enable it so tokens keep their order
through the round trip.

### Tests — `tests/design.rs`

Following `tests/content_mutation.rs`:

- `GET` on a project without `DESIGN.md` returns `exists: false`.
- Create writes the file, returns a hash, and creates a checkpoint.
- Create when the file already exists is stale.
- Update with an old hash is stale.
- Unknown token groups, unknown tokens and unknown body sections survive an update.
- Saving on a protected branch returns `CHANGE_REQUIRED`.
- Invalid frontmatter returns `invalid_design` on `GET`.

## Frontend

### Routing and navigation

- New child route `design` named `project-design`, component `DesignPanel`, props `{ projectId }`.
- `ProjectTab` gains `"design"`; `projectTabFromRoute` maps `project-design`.
- The workspace tab bar and the command palette's navigation group list **Design**.

### `web/src/core`

- `types.ts`: `DesignDoc = { exists: boolean; tokens: Record<string, unknown>; body: string; hash?: string; branch: string }`.
- `api.ts`: `design.get(project)` and `design.save({ project, branch, hash?, tokens, body })`.

### Module `web/src/modules/design/`

- **`store.ts`** (Pinia, same shape as the content store)
  - State: `doc`, `draftTokens`, `draftBody`, `dirty`, `loading`, `saving`, `error`, `conflict`,
    `needsChange`, `invalid` (raw source when the frontmatter is invalid).
  - Actions: `open(projectId)`, `create()` (fills the draft from the starter), `save(branch?)`,
    `resumeSave(branch)` for `NewChangeDialog`, `discard()`, `reload()`, `canLeave()`.
  - `save` sends the full `draftTokens` object, so keys the form does not show are kept.
  - `CHANGE_REQUIRED` sets `needsChange`; a stale error sets `conflict` and keeps the draft.
  - Reloads on `checkpoint.created` for the project and after a branch switch, unless the draft is
    dirty (then `changedOnDisk` is shown, as in content).
- **`DesignPanel.vue`**
  - Empty state when `exists` is false: short explanation and a **Create design** button.
  - Invalid state: the raw source read-only, with a message to ask the agent to fix it.
  - Otherwise two columns: `MarkdownBodyEditor` (`kind: "markdown"`, `rich: true`) for the body,
    and `DesignTokensForm` in a collapsible side panel.
  - Header: Save, Discard, **Apply to site**.
  - Route guards use `store.canLeave()`; `NewChangeDialog` opens on `needsChange`.
- **`DesignTokensForm.vue`** (`v-model` on the tokens object)
  - Brand: `name`, `description`.
  - Colours: rows of token name, native colour input, and a text input accepting any CSS colour.
    The native picker only writes hex; non-hex values show a swatch through CSS.
  - Typography: one card per style with `fontFamily`, `fontSize`, `fontWeight`, `lineHeight`, and
    optional `letterSpacing`.
  - Rounded and Spacing: rows of name and value.
  - Add, rename and remove a token in each group. Renaming keeps the token's position.
  - Other keys are not shown and are passed through.
- **`starter.ts`**: the starter tokens and body.
- **`agentRequests.ts`** in the design module: `askAgentToApplyDesign()` sends a request
  (`submit: true`) through the agent outbox:
  "Update the `@theme` block in `src/styles/global.css` (fonts, colours, radius, spacing) to match
  DESIGN.md, and follow its guidance in the components." It shows a toast, like the changes module.
  - Disabled while the draft is dirty or the file does not exist.

### Tests (Vitest)

- `store.test.ts`: open, edit, save; create; conflict; `CHANGE_REQUIRED` then resume; unknown keys
  kept; reload on checkpoint when clean.
- `DesignTokensForm.test.ts`: add, rename and remove tokens in each group; unknown keys passed
  through.
- `DesignPanel.test.ts`: empty state leads to Create; Apply to site sends the outbox message; Apply
  disabled while dirty.
- `router.test.ts`: the `design` tab route and `projectTabFromRoute`.
- `starter.test.ts`: the frontend starter matches `templates/astro-marketing/DESIGN.md`.

## Template

- Add `templates/astro-marketing/DESIGN.md` (the starter, matching the template's look).
- `templates/astro-marketing/AGENTS.md`: add "`DESIGN.md` is the design source of truth. When you
  change styles, update both `DESIGN.md` and the `@theme` block in `src/styles/global.css`."
- `src/static.rs` and the project tests that check template files also check `DESIGN.md`.
