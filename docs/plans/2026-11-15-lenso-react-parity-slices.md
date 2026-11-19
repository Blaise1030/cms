# Vue → React + Lenso parity — Slice Plan

- Spec: this plan is the spec. "Parity" means: every route, flow and `data-testid` of the Vue SPA in
  `web/` (as of `main` @ 571e117) works the same in `web-react/`, proven by running the **existing
  Playwright suite unchanged** against the React build. Seed: [lenso-spike-findings.md](lenso-spike-findings.md).
- Feature branch: `spike/lenso-web-react` (slice branches `lenso/<NN>-<slug>` chain onto it; final PR → `main`)
- Base SHA: `571e117`
- Tracker: GitHub Issues, `Blaise1030/cms`. Parent: one tracking issue, `Lenso React parity` (opened in Phase 3)
- CI checks on PR: `.github/workflows/ci.yml` gains `web-react` (tsc + vitest + build) and `e2e-react` (Playwright, allowlisted specs) in Slice 0
- E2E: Playwright, specs in `e2e/specs/`, run with `ORSO_E2E_UI=react pnpm -C e2e e2e`. The conventions doc goes in `e2e/README.md` (React section added in Slice 0)
- Conventions source: `web-react/src/core/*` (spike), `web/src/modules/*` (behaviour to copy), `e2e/README.md`

## Decisions (agreed 2026-11-15)

- **Vue stays.** `web/` remains the embedded SPA (`src/static.rs` unchanged). The cut-over is a separate, later effort.
- **Gaps use React libraries with a Lenso look:** `@tiptap/react`, CodeMirror 6 (framework-free), `@pierre/diffs`, a `cmdk`-style palette, `@ai-sdk/react`, and `react-router` 7 for routing. Everything is styled with StyleX and `@lenso/tokens`. Lenso components are used wherever one exists (Modal, Sheet, Menu, Tabs, Sidebar, Toast, Table, Form, TextField, and so on).
- **State:** one small external store per module (`useSyncExternalStore`, matching the spike's `useDaemon`). There is no Redux or Zustand. Pure `.ts` logic files such as `filter.ts`, `chunks.ts`, `diff.ts`, `markdown.ts` and `prStatus.ts` are copied over with Vue reactivity removed, and their Vitest suites are ported 1:1.
- **Testid contract:** the 471 distinct `data-testid`s in `web/src` are kept with identical names. Each slice ports the testids for its own screens.
- **Parity gate:** `e2e/react-parity.json` lists the specs that must pass against React. Each slice adds its specs. Slice 14 requires all of them.

## Slice 0: CI runs Playwright against the React build

- Issue: TBD
- Depends on: none
- Flow (REQUIRED): CI runs the smoke e2e against `web-react` on every PR
- E2E spec (REQUIRED): `e2e/specs/smoke.spec.ts` (unchanged). With `ORSO_E2E_UI=react`, the fixture builds `web-react`, starts `vite preview` with `/api`, `/ws` and `/preview` proxied to the daemon port, and passes `--ui-url` to the daemon. The spec opens the printed URL. Until Slice 1 lands, it is expected to fail only at `project-grid`. To get the harness green in this slice, the allowlist starts with a `react-harness.spec.ts` check: the page loads, the token is accepted, and the WS status reaches `open`.
- Unit tests (REQUIRED): fixture UI-mode selection (vue/react) and preview-port allocation in `e2e/fixtures/*.test.ts`
- Layers touched: `e2e/fixtures/daemon.ts`, `e2e/playwright.config.ts`, `e2e/react-parity.json`, `web-react/vite.config.ts` (`preview.proxy`), `.github/workflows/ci.yml`, `justfile` (`e2e-react`), `e2e/README.md`
- Out of scope for this slice: any UI port
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 1: Open a project and see its files and preview (walking skeleton)

- Issue: TBD
- Depends on: 0
- Flow (REQUIRED): a writer opens the app → sees the project grid → opens a project → sees the workspace with the tab rail, the Code tab's file tree and the Preview frame
- E2E spec (REQUIRED): `e2e/specs/smoke.spec.ts`, added to the allowlist
- Unit tests (REQUIRED): router helpers (`codePath`, `projectTabFromRoute`, deep-link encoding), the projects store and status mapping, and the `DaemonUnreachable` and `ConnectionStatus` states (ported from `router.test.ts`, `Shell.test.ts`, `Workspace.test.ts`, `ProjectRail.test.ts`, `ProjectPage.test.ts`)
- Layers touched: `web-react/src/{router.tsx,App.tsx,main.tsx}`, `app/{Shell,Workspace,ProjectPage,ProjectRail,HomePage,ConnectionStatus,DaemonUnreachable,PawMark}.tsx`, `modules/projects/{store,status}.ts` + `ProjectsGrid.tsx`, `modules/files/{store.ts,ProjectFilesTree.tsx}`, and a minimal `modules/preview/PreviewPanel.tsx` (iframe plus frame bridge only). Adds the `react-router` dependency, theme tokens and Geist fonts.
- Out of scope for this slice: file viewer, preview chrome, the other tabs (they render placeholders keyed by their testids)
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 2: Create, import and invite to projects; sign in to GitHub

- Issue: TBD
- Depends on: 1
- Flow (REQUIRED): a writer clicks New project → picks a template or a GitHub repo, signing in with device flow when needed → lands in the new project. They can also open Invite and copy a link.
- E2E spec (REQUIRED): `e2e/specs/projects-new.spec.ts` (new, also run against Vue): device-code sign-in against the stub → the repo browser lists `acme/site` → clone → the workspace opens. Template path: scaffold → the workspace opens.
- Unit tests (REQUIRED): `invite.ts`, the github store's device-flow polling, and template and repo filtering
- Layers touched: `modules/projects/{NewProjectDialog,TemplateGallery,RepoBrowser,InviteDialog}.tsx`, `invite.ts`, `modules/github/*`
- Out of scope for this slice: GitHub settings page (Slice 12)
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 3: Browse and read code files

- Issue: TBD
- Depends on: 1
- Flow (REQUIRED): a writer opens the Code tab → picks a file in the tree → sees it highlighted in a read-only CodeMirror view, and the deep link `/p/:id/code/<path>` restores it
- E2E spec (REQUIRED): `e2e/specs/files-view.spec.ts` (new): click `src/pages/index.astro` → `file-viewer` shows its content → reload → same file selected
- Unit tests (REQUIRED): `language.ts` (extension → language), the files store's selection and URL sync
- Layers touched: `modules/files/{FileViewer,FileCodeMirror,FilesPanel}.tsx`, `language.ts`. Adds the CodeMirror deps.
- Out of scope for this slice: editing files
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 4: Navigate and inspect the live preview

- Issue: TBD
- Depends on: 1
- Flow (REQUIRED): a writer navigates inside the preview → uses back and forward → opens page details, page errors and checks → picks an element
- E2E spec (REQUIRED): `e2e/specs/preview-nav.spec.ts`
- Unit tests (REQUIRED): `frameHistory`, `frameFit`, `frameAppearance`, `pageErrors`, `elementPicker`, `conflictHighlight`, and `checks/*` (ported)
- Layers touched: `modules/preview/*` (full `PreviewPanel`, `PageDetailsSidebar`, `checks/`)
- Out of scope for this slice: Share and Publish buttons in the browser bar (Slice 8)
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 5: Edit content entries with the visual editor

- Issue: TBD
- Depends on: 4 (Open in preview)
- Flow (REQUIRED): a writer opens Content → picks or creates an entry → edits the frontmatter and the markdown body (TipTap), inserts an image and a link → saves → opens it in the preview
- E2E spec (REQUIRED): `e2e/specs/content-editor.spec.ts`
- Unit tests (REQUIRED): `markdown.ts`, `draft.ts`, `entryList.ts`, `create.ts`, `labels.ts`, `references.ts`, `openInPreview.ts`, `history/*`, and the `MarkdownBodyEditor` round-trip (ported)
- Layers touched: `modules/content/*`. Adds the `@tiptap/react` deps (same 3.31.3 pins).
- Out of scope for this slice: none
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 6: Chat with an agent and get a checkpoint

- Issue: TBD
- Depends on: 1
- Flow (REQUIRED): a writer opens the agent panel → sends a message, with optional attachments and page context → sees the streamed reply and a checkpoint card
- E2E spec (REQUIRED): `e2e/specs/chat.spec.ts`
- Unit tests (REQUIRED): `chunks`, `parts`, `stream`, `outbox`, `recovery`, `links`, `references`, `attachments`, `modelGroups`, `providers`, `pageContext` (ported)
- Layers touched: `modules/chat/*` and `AgentFloatingHost`. Adds the `@ai-sdk/react` deps.
- Out of scope for this slice: the main-branch guard (Slice 7) and the sync chat (Slice 9)
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 7: Start and switch changes

- Issue: TBD
- Depends on: 6 (an agent session triggers the guard)
- Flow (REQUIRED): a writer on `main` starts a change, or is prompted to by the guard → work checkpoints on the new branch → the writer switches between open changes with the branch picker
- E2E spec (REQUIRED): `e2e/specs/changes-start.spec.ts`, `changes-switch.spec.ts`, `changes-main-guard.spec.ts`
- Unit tests (REQUIRED): changes store (open-change ordering, collision suffixing, switch confirmation)
- Layers touched: `modules/changes/{store.ts,ChangesBranchPicker,NewChangeDialog,ConfirmSwitchDialog}.tsx`
- Out of scope for this slice: share, PR and conflicts
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 8: Share a preview and follow its PR to merge

- Issue: TBD
- Depends on: 7, 4
- Flow (REQUIRED): a writer shares a change → sees the push, draft PR and link → sees the PR state and checks in the browser bar → marks it ready → after merge, cleans up. Get latest merges `main` into every open change.
- E2E spec (REQUIRED): `share-preview.spec.ts`, `pr-status.spec.ts`, `branch-protection-hint.spec.ts`, `ready-for-review.spec.ts`, `merged-cleanup.spec.ts`, `get-latest.spec.ts`
- Unit tests (REQUIRED): `prStatus.ts`, the protection-hint dismissal persistence, and the 401/403 error mapping
- Layers touched: `modules/changes/{SharePreviewButton,PublishButton,PrMenu,ProtectionHint,CleanupChangeButton}.tsx`
- Out of scope for this slice: conflicts
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 9: Resolve sync conflicts

- Issue: TBD
- Depends on: 8, 6
- Flow (REQUIRED): a writer's change conflicts with `main` → they see the conflict view → pick a side per file, or abandon → or the sync chat auto-combines the change
- E2E spec (REQUIRED): `conflicts.spec.ts`, `sync-auto-resolve.spec.ts`
- Unit tests (REQUIRED): conflict-file side selection, and the deleted-vs-edited mapping
- Layers touched: `modules/changes/{ConflictView,ConflictFile}.tsx`, the chat sync mode
- Out of scope for this slice: none
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 10: Review history and diffs

- Issue: TBD
- Depends on: 1
- Flow (REQUIRED): a writer opens History → sees checkpoints → selects one → sees the changed-files tree and a diff, and can restore it
- E2E spec (REQUIRED): `e2e/specs/history.spec.ts` (new): make a content save → History lists it → the diff shows the edited line
- Unit tests (REQUIRED): `checkpoints/diff.ts`, and the store's selection and URL sync
- Layers touched: `modules/checkpoints/*`. Adds the `@pierre/diffs` and `@pierre/trees` deps.
- Out of scope for this slice: none
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 11: Edit design tokens

- Issue: TBD
- Depends on: 1
- Flow (REQUIRED): a writer opens Design → edits a color or font token → saves → the preview reflects it. They can also ask the agent to change a token.
- E2E spec (REQUIRED): `e2e/specs/design-tokens.spec.ts` (new)
- Unit tests (REQUIRED): `tokens.ts`, `agentRequests.ts`
- Layers touched: `modules/design/*`
- Out of scope for this slice: none
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 12: Configure app, project and assistant settings

- Issue: TBD
- Depends on: 2 (GitHub account)
- Flow (REQUIRED): a writer opens Settings → changes appearance, connects or disconnects GitHub, sets the site builder and assistant provider (setup, sign-in) → changes persist across reloads
- E2E spec (REQUIRED): `e2e/specs/settings.spec.ts` (new)
- Unit tests (REQUIRED): settings store, and the assistant `rows`, `presentation` and `dialog` logic
- Layers touched: `modules/settings/*`, `modules/assistant/*`
- Out of scope for this slice: none
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 13: Command palette, menubar and app notices

- Issue: TBD
- Depends on: 3, 5, 10, 11, 12 (the palette's targets)
- Flow (REQUIRED): a writer presses ⌘K → filters commands and jumps to a file, entry or tab. Setup check, update notice and desktop update banners appear when relevant.
- E2E spec (REQUIRED): `e2e/specs/command-palette.spec.ts` (new)
- Unit tests (REQUIRED): `palette/filter.ts`, `usePaletteGroups`, `setup.ts`, `desktopUpdate.ts`, and the `UpdateNotice` and `SetupCheck` states
- Layers touched: `app/{CommandPalette,EditorMenubar,SetupCheck,UpdateNotice}.tsx`, `palette/*`
- Out of scope for this slice: none
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Slice 14: Full parity gate

- Issue: TBD
- Depends on: 0–13
- Flow (REQUIRED): CI runs the whole Playwright suite against both UIs and both pass
- E2E spec (REQUIRED): all of `e2e/specs/*` (the allowlist is replaced by "all")
- Unit tests (REQUIRED): a testid-parity script that diffs the `data-testid`s in `web/src` and `web-react/src` and fails CI on a mismatch
- Layers touched: `e2e/react-parity.json` (removed), `scripts/testid-parity.mjs`, `ci.yml`, `docs/plans/lenso-spike-findings.md` (decision recorded)
- Out of scope for this slice: switching `src/static.rs` to embed `web-react/dist`, and deleting `web/`
- Done when: e2e + unit tests pass locally, all CI checks green, PR closes the issue

## Parallelism

After Slice 1, slices 2, 3, 4, 6, 10 and 11 can run at the same time. After that: 5 needs 4; 7 needs 6; 12 needs 2; 8 needs 7 and 4; 9 needs 8; 13 needs the rest; 14 comes last.
