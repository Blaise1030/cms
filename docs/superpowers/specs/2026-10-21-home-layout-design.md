# Home layout — design

Date: 2026-10-21

## Goal

Replace the welcome hero on the main page with the three-section layout from the supplied mock:
**Open a project** (searchable GitHub repository list), **Create from a template** (card grid), and
**Recent** (local projects). The app mark moves to the title-bar corner and the account avatar stays
in the opposite corner.

Today's page centres a large paw mark and wordmark, two action buttons, and a "Recent" list. The
Open-from-GitHub flow is a modal, and the template is only reachable through a "New project" button.

### What the user asked for

- The layout of the mock: corner logo, "Open a project" heading with a search field and a list of
  repositories with a per-row **Open** button, then "Create from a template" with cards.
- Keep a way back into existing local projects (a **Recent** section).
- One real template card for now; the grid should be ready for more.
- No owner dropdown and no gear button.

### Assumptions

- One GitHub account. The repository list is whatever `/user/repos` returns; there is no owner filter.
- A repository that is already cloned opens the existing project instead of cloning a second copy.
- Choosing a template opens the existing **New project** dialog for naming; the template itself is
  unchanged.
- macOS owns the window's top-left corner (traffic lights), so the mark sits just to their right,
  matching `Shell.vue`'s existing `pl-20` convention.

### Out of scope

- Additional templates, a template registry, or a template id in the create API.
- Owner/org switching and a settings screen.
- Fabricated template screenshots.

## Layout

- **Title bar row.** One 44 px row at the top of the page holds the mark on the left and the account
  avatar on the right. On macOS it is the existing drag strip (`data-tauri-drag-region`, hidden when
  the preview is fullscreen) with the mark offset past the traffic lights; in a browser it is a plain
  row with no drag region. The avatar's current browser-only floating copy goes away — both
  platforms use the same row.
- **Content.** A centred column (`mx-auto w-full max-w-3xl px-10 pt-16 pb-16`) with the sections
  stacked `gap-12`.
- The hero (large paw, `orso` wordmark, tagline) and the **New project** / **Open from GitHub**
  buttons are removed.

## Open a project

- Heading `Open a project` (`text-lg font-medium`).
- Search input bound to a filter. Placeholder `Search repositories by name`, aria-label
  `Search repositories`.
- A bordered, rounded list of rows. Each row shows the repository's `owner/name`, its relative push
  time when known (`1 day ago`), and an `Open` button.
- **Open** behaviour:
  - If the projects store already has a project with `repo === fullName`, navigate to it.
  - Otherwise `cloneRepo(fullName)`, then navigate to the new project.
- Repositories load when the page mounts (signed in only), not on first modal open. The list keeps
  GitHub's order (most recently pushed first); filtering never reorders.
- States, carried over from the dialog:
  - **Signed out:** device-flow sign-in button when available, otherwise the
    `gh auth login` instruction.
  - **Loading:** a spinner row.
  - **Empty / no match:** a muted message distinguishing "no repositories" from "none match".
  - **Error:** destructive text plus a **Retry** action.

## Create from a template

- Heading `Create from a template`.
- Responsive grid (`grid gap-4 sm:grid-cols-2 lg:grid-cols-3`) of template cards.
- One card for **Astro marketing**: a muted preview area with the Astro glyph (no fabricated
  screenshot), the template name, and a one-line description. The whole card is a button that opens
  `NewProjectDialog`.

## Recent

- Heading `Recent`.
- The existing project list, unchanged in behaviour: status dot, name, repo subtitle, `localhost:port`
  badge, and the actions menu (Delete).
- Keeps its loading skeleton, empty message, and error/retry states.

## Data

`sort=pushed` is already requested from GitHub; the field is parsed and dropped. Expose it:

- `src/github.rs`: `ApiRepo` gains `#[serde(default)] pushed_at: Option<String>`; `RepoInfo` gains
  `pushed_at: Option<String>`, serialized as `pushedAt`.
- `web/src/core/types.ts`: `GithubRepo` gains `pushedAt: string | null`.
- A small relative-time formatter in the web app (`Intl.RelativeTimeFormat`, minutes → years).
  Missing timestamps render no text.

## Components

- `web/src/modules/projects/RepoBrowser.vue` — search, list, open, and its loading/empty/error
  states. Uses the github and projects stores and the router.
- `web/src/modules/projects/TemplateGallery.vue` — the card grid; emits `create`.
- `ProjectsGrid.vue` — title bar row plus the three sections, composing the two new components and
  the existing project list.
- `OpenRepoDialog.vue` is deleted; its tests move to `RepoBrowser.test.ts`.

## Testing

- `RepoBrowser.test.ts` (ported from `OpenRepoDialog.test.ts`): filtering, opening clones when the
  repo is not local, opening reuses a local project without cloning, signed-out prompt, error text.
- `ProjectsGrid.test.ts`: still covers the drag strip and project rows; adds that the three section
  headings render and that the mark and account control share the title bar row.
- `src/github.rs`: a test that `pushed_at` is parsed from the API payload and serialized as
  `pushedAt`.
