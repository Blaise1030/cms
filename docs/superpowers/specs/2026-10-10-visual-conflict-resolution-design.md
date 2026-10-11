# Visual conflict resolution design

Status: **draft for review, 2026-10-10**. Extends the approved Phase 2 conflict flow in
`2026-10-04-phase2-changes-preview-review-design.md` and Slice 9 of
`../plans/2026-10-04-phase2-slices.md`.

## Goal

A writer who clicks **Get latest** can understand conflicting page changes without reading Git
markers or source code. Before saving, they can see what each version looks like and preview the
result of their choice. The app must preserve the branch until the writer finishes, and must make
**Abandon** safe and obvious.

## Writer flow

1. Get latest reports the affected change branches. Opening one shows a list of conflicting files
   with readable labels where available, plus the file path for orientation.
2. Selecting a file shows **Your version** and **Latest from main** side by side. Both previews
   open the same page and use the same viewport. A visible outline points to the affected section
   when the app can locate it reliably. The writer can toggle between desktop and mobile widths.
3. The writer chooses **Keep yours**, **Use latest**, or **Keep both** for each file. **Keep both**
   needs an ordered result when both sides edit the same content; the UI states the order explicitly.
4. A **Result** preview renders the proposed merge. The writer reviews it, then clicks **Finish
   update**. The resulting merge commit uses the signed-in writer's Git identity. **Abandon**
   discards the proposed merge and leaves the branch at its original commit.
5. If the page cannot be rendered, show the conflicting text with changes highlighted and keep
   the choices available. State why the visual preview is unavailable. A broken result preview
   must be shown as an error, never as a blank page that looks successful.

## Conflict model and branch safety

- Keep the current `get_latest` behavior of computing merges before moving refs. For a conflicting
  branch, record its tip, the exact incoming main commit, merge base, and conflicted paths as a
  short-lived resolution session. Do not change the branch or working tree while choices are made.
- Derive each side's file content from those immutable commit IDs. Apply choices to a candidate
  merge tree. Finishing creates a two-parent merge commit and advances the branch only if its tip
  still equals the recorded tip. If it changed, invalidate the session and ask the writer to run
  Get latest again. Abandon removes the session and its temporary resources.
- Serialize resolution with checkpoints, branch switching, and later Get latest operations for
  that branch. Other change branches can remain usable.
- A file choice applies to the whole conflicted file in this first version. Nonconflicting edits
  from the merge must remain in the result. For text conflicts, **Keep both** combines only the
  conflicting regions in a defined order, rather than concatenating whole files. Binary files
  offer only **Keep yours** and **Use latest**. If a combined file cannot be produced safely, show
  the text comparison and require a different choice or developer help.

## Visual preview

- Render the two source commits and proposed result in isolated, temporary Git worktrees, each
  with its own dev server and URL namespace. The active project's checkout and preview stay
  untouched. Reuse dependency files where safe, but never write generated output into the project.
- The preview proxy must route each isolated preview's pages, assets, and HMR requests to its own
  server. Its current root-path `wb_vite` cookie selects one project, so it cannot distinguish
  three previews as written; namespace these requests or use separate origins before enabling
  simultaneous frames.
- Match visible sections using stable content identifiers when available (for example a section
  id in the Astro marketing template). Astro source annotations can help for component files.
  CSS selectors and DOM position alone are insufficient: a section can move or disappear between
  versions. When the match is uncertain, show the whole page without an element outline.
- Selecting a file should navigate both previews to the same route when the route is known. For
  changes shared across pages, let the writer choose the page. Do not imply a single screenshot
  describes all effects of a stylesheet or shared component change.
- Keep visual previews read-only: links may navigate within the preview; forms and external
  actions should not be part of conflict resolution. The text diff remains available for details.

## Boundaries and error handling

- Scope: websites the project can run in its existing dev-server environment. A repository that
  cannot start an isolated preview still gets file-level resolution and a text comparison.
- No pixel-perfect difference detector in the first version. The side-by-side views and section
  outline answer what changed visually; they do not claim to detect every pixel affected.
- If the proposed files have a confirmed syntax or build error, mark **Result** invalid with that
  error and keep **Finish update** disabled until the writer changes the choice or gets help. If
  rendering fails only because the preview service cannot start, keep the text comparison and
  choices usable and say that the visual result could not be checked.
- Limit simultaneous preview processes and clean up worktrees on Finish, Abandon, session expiry,
  and daemon restart. Temporary paths must stay outside user projects.

## Verification

- Rust tests: exact conflict blobs and merge base, per-file choices, preservation of clean merged
  edits, binary behavior, stale-tip refusal, identity and parent commits, and Abandon leaving refs
  and the checkout untouched.
- Web tests: labels and choices, result-loading/error states, missing element outline fallback,
  and Finish disabled until every file is resolved and the result preview is valid.
- End-to-end test with the Astro template: edit one visible section on both sides, compare the
  two rendered versions, choose a resolution, inspect Result, finish, then confirm the final
  branch and live preview. Cover Abandon and a preview-start failure separately.

## Implementation sequence

1. Conflict session and API with safe branch semantics and text resolution.
2. Conflict screen with per-file choices and text fallback.
3. Isolated previews and result rendering.
4. Reliable section mapping and visual outline for the Astro template, with full-page fallback.
