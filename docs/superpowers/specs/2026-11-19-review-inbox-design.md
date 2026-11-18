# Review inbox: greenlight teammates' changes in orso (spec)

Builds step 5 of [Creator publishes, owner greenlights](2026-10-18-creator-publishes-owner-greenlight-design.md)
("Waiting for your greenlight"). Uses that spec's words on screen: **change, greenlight,
send back**. GitHub stays the enforcement layer.

## Problem

orso opens a labelled PR for every shared change (`orso.json` `prLabels`, default
`["orso"]`), but a content lead can only review it on GitHub: raw diffs, no rendered
page, Git vocabulary. orso itself cannot show a teammate's change at all: there is one
checkout per project and the live preview runs on it.

## Goal

A content lead opens a site in orso, sees the teammates' changes waiting for them, looks
at each one rendered and as a list of what changed, and gives a greenlight or sends it
back with a comment, without leaving orso, touching their own change, or knowing Git.

Success: the E2E flow under **Testing** passes, and a reviewer's own uncommitted work
and checked-out change are untouched throughout.

## Users

- **Content lead (reviewer):** non-technical, has push access to the site repo, reviews
  other people's changes. Primary user of this spec.
- **Creator:** shared the change. Sees the outcome (greenlit / sent back + comment).
- Developers may still review the same PR on GitHub; their reviews count the same way.

## Decisions (settled)

| Question | Decision |
| --- | --- |
| Who reviews | Content leads inside orso; no GitHub knowledge needed |
| Which changes appear | Open, non-draft PRs with any `prLabels` label, not authored by you |
| Inbox scope | Per site (project) for v1 |
| How the change is shown rendered | Static build of the PR head in a temporary worktree |
| "Before" side | **View live site** link; no build of `main` in v1 |
| Send-back comment | Required (GitHub requires a body for Request changes) |
| Who can decide | Push access to the repo, and not your own change |
| Disk use | Worktree exists only during a build; built output cached with limits |

## User flow

1. **Inbox.** Project rail shows **Review** with a count of changes whose state for you is
   *New*. The Review tab (`/p/:id/review`) lists each change: title, creator, last
   updated, checks, and your state on its *current* version:
   - *New*: no review from you on the current head commit.
   - *You sent back*: your latest review on this head requests changes.
   - *You greenlit*: your latest review on this head approves.
   A push by the creator returns the row to *New*. Empty state: "Nothing waiting for
   your greenlight."
2. **Open a change** (`/p/:id/review/:number`). Header: title, creator, description,
   **View live site**. Two panes:
   - **Preview**: the built change, navigable like the normal preview. While building:
     "Preparing the preview…". On failure: the build's last output and **Try again**;
     the rest of the screen still works.
   - **What changed**: the existing `ChangedFilesTree` + `DiffView`, comparing the head
     with its merge-base on `main`.
3. **Decide.**
   - **Give greenlight** → GitHub Approve review on the head commit you viewed.
   - **Send back** → dialog with a required comment → GitHub Request changes review.
   - Disabled with a reason when it would not count: "This is your change" or "You need
     edit access to this site's repository to greenlight".
   - If the creator pushed after you opened it: "This change was updated since you
     opened it" and the view reloads on the new version; nothing is submitted.
4. **Creator sees the outcome.** The creator's change menu (`PrMenu`) shows **Greenlit**
   (existing `approved` state) or **Sent back** with the reviewer's name and comment
   verbatim.

## Architecture

### Daemon

**Fetching a change without touching the checkout.**
`git fetch origin pull/<n>/head:refs/orso/review/<n>` stores the PR's commits in the
object store only. No branch is created and the working tree is untouched.

**What changed** is computed from objects alone:
`git diff <merge-base(origin/main, head)>..<head>`, same response shape as the
checkpoint diff so the web components are reused unchanged.

**Snapshot preview service** (generalizes `src/conflict_preview.rs`). One build pipeline
keyed by a snapshot id: a conflict side, or `(project, PR number, head SHA)`.
- Builds in the `orso.json` `root` (via `settings::load`), linking `node_modules` from the
  `installDir`. This also fixes conflict previews for sites not at the repo root.
- Steps: `git worktree add --detach` at the revision → link `node_modules` →
  `npm run build` with `PUBLIC_BASE` (90 s timeout, process group killed on timeout) →
  move `dist/` into the cache → **remove the worktree**. The worktree is removed on
  success, failure and timeout alike.
- Serves cached `dist/` files with the existing safe-route handling, at
  `/review-preview/{project}/{sha}/…` for reviews (conflict URLs unchanged).
- Conflict previews switch to this service and so also stop keeping worktrees.

**Cache limits** (review snapshots):
- Key `(PR, head SHA)`; a new head deletes the previous build for that PR.
- Deleted when the PR is no longer open (checked on each inbox load).
- At most 3 builds per project (least recently viewed evicted) and none older than 24 h.
- On daemon start and shutdown: remove leftover worktree and cache folders that no live
  entry owns, `git worktree prune`, and delete `refs/orso/review/*` for PRs not open.

**GitHub** (`src/github.rs`, token or `gh` CLI as today):
- List: open PRs for the repo filtered by label (search or `GET /pulls` + labels), drop
  drafts and the signed-in user's own.
- Reviews per PR: latest review by the signed-in user and its `commit_id` → state.
- Submit: `POST /repos/{repo}/pulls/{n}/reviews` with `event` `APPROVE` or
  `REQUEST_CHANGES`, `body`, and `commit_id` = the head the client viewed. The daemon
  first re-reads the PR head; if it differs, returns 409 `head_moved` without submitting.
- `PrState` gains `changes_requested`; `PrStatus` gains the latest send-back's reviewer
  login and body for the creator's view. Existing `approved` rule is unchanged.

**Push access**: from the repo's `permissions.push` (already read for the repo browser).

**Endpoints** (under `/api/projects/{id}/reviews`):

| Route | Returns / does |
| --- | --- |
| `GET /` | Inbox rows: number, title, author, updatedAt, checks, myState |
| `GET /{n}` | Title, author, description, headSha, myState, `canDecide`, `reason` |
| `GET /{n}/diff` | Changed files + unified diff (checkpoint diff shape) |
| `POST /{n}/preview` | Start or reuse the build for the current head |
| `GET /{n}/preview` | `building` / `ready` (url) / `failed` (message) |
| `POST /{n}/decision` | `{ decision: "greenlight" \| "send_back", comment?, headSha }` |

Errors: 403 → "You don't have push access"; 409 `head_moved`; `send_back` without a
non-blank comment → 400; PR not open → 404 "This change is no longer waiting".

### Web

- New module `web/src/modules/review/`: store, `ReviewInbox`, `ReviewChange`,
  `SendBackDialog`. Route children `review/:number?` under `/p/:id`; rail entry with a
  count badge. Built with the repo's shadcn-vue primitives.
- Reuses `ChangedFilesTree`, `DiffView` and the preview iframe component.
- `PrMenu` shows **Sent back** + reviewer + comment for `changes_requested`.

## Testing

- **Rust unit:** inbox filtering (label, draft, own); myState from reviews vs head SHA;
  `canDecide` reasons; decision body carries `commit_id`; moved head → 409 and no
  submit; send-back without comment → 400; `changes_requested` mapping; cache eviction
  (new head, PR closed, cap of 3, 24 h); worktree removed after success, failure and
  timeout; startup sweep removes orphans and stale refs; build runs in `orso.json` root.
- **Web unit:** inbox rows and badge count; disabled buttons show their reason; send-back
  requires a comment; head-moved message reloads; `PrMenu` Sent back view.
- **E2E** (GitHub stub + a stub PR head ref in the bare remote): open Review → see the
  PR → open it → preview builds, diff lists the file → Give greenlight → stub received
  `APPROVE` with the right `commit_id` → reviewer's own checkout and branch unchanged.
  Second spec: Send back with comment → creator's project shows **Sent back** and the
  comment.

## Delivery (one PR per slice, each user-visible)

1. **Inbox**: Review tab, list, badge, open view with header + What changed (no
   preview, no decisions).
2. **Decisions**: Give greenlight / Send back with `commit_id` pinning and `head_moved`;
   creator's **Sent back** state and comment.
3. **Preview**: snapshot preview service (orso.json root, worktree removed after build,
   cache limits, startup sweep) wired into the review view; conflict previews migrated.

## Out of scope

- Inline or threaded comments, replies, suggestions (stay on GitHub).
- A cross-site inbox; notifications outside orso.
- Building `main` for a side-by-side "before" preview.
- Merging on the creator's behalf (unchanged: creators publish their own changes).
- Reviewing PRs that orso did not label.
