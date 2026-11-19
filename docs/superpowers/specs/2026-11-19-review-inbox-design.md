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
| Which changes appear | Open, non-draft PRs with any `prLabels` label, yours included (marked **Yours**, view-only, not counted in the badge) |
| Inbox scope | Per site (project) for v1 |
| How the change is shown rendered | The PR head's deploy preview (GitHub Deployments), framed next to the live site; nothing is built locally |
| "Before" side | The live site, framed beside the change (**Side by side**), plus the **View live site** link |
| Send-back comment | Required (GitHub requires a body for Request changes) |
| Who can decide | Push access to the repo, and not your own change |
| Disk use | None: previews are the site's own deploys, so cost doesn't grow with the site |

## User flow

1. **Inbox.** Project rail shows **Review** with a count of other people's open changes
   whose state for you is *New*. The Review tab (`/p/:id/review`) is a card like GitHub's
   pull request list (no checkboxes):
   - **Search** (kept in `?q=`, default `is:open`): free text over title, `#number` and
     author, plus qualifiers `author:`, `label:`, `review:new|greenlit|sent_back|yours`,
     `is:open|closed`, `sort:created-desc|created-asc|updated-desc`; anything else is text.
   - **Header strip**: **Open** *n* / **Closed** *n* (closed is one page of 30, shown as
     `30+` when full); **Author**, **Label**, **Reviews** and **Sort** (Newest, Oldest,
     Recently updated) menus that write their qualifier into the search; picking the active
     one again clears it.
   - **Rows**: state icon (open, draft, merged, closed), title, label chips in GitHub
     colours, `#83 · author opened 19 hours ago · ✕ 8/9`, comment count, and the review
     tag on the change's *current* version: *New*, *You sent back*, *You greenlit*, or
     *Yours* for your own (view-only). A push by the creator returns the row to *New*.
   - **Empty**: "No changes match your search." with **Clear filters**, or "No changes
     waiting for review." / "No closed changes."
   - A closed or merged change opens read-only: decisions off with "This change is
     closed"; its preview still shows its deploy if one is left.
2. **Open a change.** Like GitHub, a change has two pages sharing one header: back,
   title, `#number`, state (Open / Draft / Merged / Closed), creator, labels in their
   GitHub colours, and **Give greenlight** / **Send back**. Below the header, one card
   holds the page: its top strip has the tabs (router links) **Conversation** *n* |
   **Review** *files* and, at the end, **Refresh**, which reads the change again in place
   (detail and diff, plus the conversation or the preview), spinning while it loads.
   - **Conversation** (`/p/:id/review/:number`, where a change opens), laid out like a chat
     with the assistant chat's Nuxt UI components and spacing. PR text is untrusted
     markdown: raw HTML stays inert text, links open outside the app.
     - **Description** pinned at the top; one longer than a few lines folds to its first
       line until opened.
     - **Feed** (`UChatMessages`), oldest at the top, opening on the latest; it follows new
       entries while you're at the bottom and offers a jump down when you've scrolled up.
       A day divider ("Today", "Yesterday", else the date) starts each day.
     - **Messages** (`UChatMessage`): comments and comment-only reviews, with the author's
       GitHub avatar, bold name and time, markdown below. One author's messages within
       5 minutes share one header (follow-ups show their time on hover). Hovering shows
       **Open on GitHub** and the exact time.
     - **Events** as compact single lines: "*Ana* greenlit this change", "*Ana* sent this
       back" (its comment quoted below), "*Blaise* pushed N commits" (folded; short sha +
       first line of each, consecutive commits grouped and dated by the latest, a
       `checkpoint:` prefix as a badge), merged or closed. Pending and empty reviews are
       left out.
     - **Composer** (`UChatPrompt`) pinned at the bottom: "Message…", grows to 8 rows,
       ⌘/Ctrl+Enter sends (Enter is a new line), **Send** disabled while blank. Sending is
       optimistic: the composer clears and the message appears at the bottom at once, muted,
       as *Sending…* under your name and avatar, then GitHub's copy takes its place (same
       row). A failure stays in place as "Couldn't send. **Retry** · **Delete**". Quick sends
       go out in order, and a re-read of the conversation keeps unsent messages. Anyone
       signed in may comment; no push access needed.
     - **Sidebar**: labels, checks (✕/✓ passed/total, expandable to each check linked to
       GitHub), the deploy preview link, and each reviewer's latest standing (or *Awaiting
       review*).
   - **Review** (`/p/:id/review/:number/files`): the change in a card with two panes:
   - **Preview**: a browser frame like the preview page's (back/forward, reload, address
     bar, viewport size with zoom-to-fit, open in new tab) showing **This change** and the
     **Live site** side by side. One address opens the same page on both sides; each side
     fits desktop (1440 px) to its own width. Either side can be folded to a narrow strip
     (label, state, a button to bring it back) so the other fills the width; one at a time.
     A side with nothing to show starts folded; otherwise the browser remembers the last
     fold (`localStorage`).
     - Deploy running: "Deploying preview…", checked again every 10 s while the page is
       visible.
     - Deploy failed: "The preview deploy failed." with a link to the PR's checks.
     - No deployment and no deploy check: "This site doesn't publish previews for
       changes. See What changed."
     - No live site: that side says "No live site found for this project."
     The rest of the screen works in every state.
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

**Preview from the site's own deploys.** The template deploys every PR to its own preview
URL and records a GitHub Deployment; orso already reads that for **Share preview** and
**View live site**. The review preview reuses it:
- `url`: `environment_url` of the newest successful deployment of the head SHA.
- Otherwise the head's deploy check run says `deploying` (running) or `failed`; with
  neither, `none`.
- `liveUrl`: the production URL (deployment of `main`'s tip, or derived from a preview
  URL and the worker name), best effort.

Nothing is checked out or built for a review, so the cost doesn't grow with the size of
the site. Deploy previews are on their own origin, so the frames may run the site's
scripts (`sandbox="allow-scripts allow-same-origin allow-forms allow-popups"`, only
http(s) URLs); the cross-origin frame can't report where a page navigates, so the address
bar holds the path it opened. Conflict previews keep their own local build (unchanged by
this spec). A site whose deploy records no GitHub Deployment gets no rendered preview,
only **What changed**.

**Refs.** `refs/orso/review/*` for PRs no longer open are deleted on each inbox load.

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
| `GET /` | Open inbox rows: number, title, author, createdAt, updatedAt, labels (name, colour), comments, checks + checksPassed/checksTotal, state, draft, myState, mine |
| `GET /?state=closed` | `{ reviews, more }`: the 30 most recently updated labelled closed or merged changes |
| `GET /{n}` | Title, author, description, headSha, myState, `canDecide`, `reason` |
| `GET /{n}/diff` | Changed files + unified diff (checkpoint diff shape) |
| `GET /{n}/conversation` | `{ items, checks, reviewers }`: timeline oldest first (`comment` / `review` / `push` / `state`; comments and reviews carry `avatarUrl`, reviews their `url`), head check runs, reviewers. Any labelled change, open or not |
| `POST /{n}/comments` | `{ body }` → the new `comment` item. 400 when blank, 404 unless open and labelled |
| `GET /{n}/preview` | `{ state: deploying \| ready \| failed \| none, headSha, url?, liveUrl? }` for the current head |
| `POST /{n}/decision` | `{ decision: "greenlight" \| "send_back", comment?, headSha }` |

Errors: 403 → "You don't have push access"; 409 `head_moved`; `send_back` without a
non-blank comment → 400; deciding or commenting on a PR that is not open, an open draft, or an unlabelled PR → 404 "This change is no longer waiting" (a closed labelled change still reads, decisions off).

### Web

- New module `web/src/modules/review/`: store, `ReviewInbox`, `ReviewChange`,
  `SendBackDialog`. Route children `review/:number?` under `/p/:id`; rail entry with a
  count badge. Built with the repo's shadcn-vue primitives.
- Reuses `ChangedFilesTree`, `DiffView` and the preview iframe component.
- `PrMenu` shows **Sent back** + reviewer + comment for `changes_requested`.

### Caching

GitHub is asked as little as possible, in two layers:

- **Daemon** (`src/github_cache.rs`): every REST GET made with a token goes through one cache
  keyed by the token's identity and the full URL (each page of a list is its own entry). A
  response is reused for 15 s without asking; after that it is revalidated with
  `If-None-Match` / `If-Modified-Since`, and a `304` (free against the rate limit) reuses the
  stored body. Identical GETs in flight at once share one request. Any write (comment,
  review, merge, PR create or label, ready for review) drops every entry for that repository
  (GraphQL writes drop everything). At most 500 entries, least recently used evicted; non-2xx
  responses are never stored. The `gh` CLI path is not cached. Tests and e2e set the reuse
  window to 0 (`get_cache_ttl`, `ORSO_GITHUB_CACHE_TTL_MS`) because they change GitHub behind
  the daemon's back.
- **Web** (`@tanstack/vue-query`, review module only): the inbox, closed list, change
  (detail + diff), conversation and preview are queries shared by the rail and the pages.
  Data is fresh for 30 s, read again when the page becomes visible, and polled only while
  visible: the inbox (and badge) every 60 s, a running deploy every 10 s. A decision updates
  the row and the change at once, then marks the inbox and change stale; a comment is added
  from GitHub's answer and marks the inbox stale.

### Screenshot comments and images

Reviewers can point at the page itself, and anyone can attach images to a comment.

- **Picking an area** (Review page): each side of the frame (*This change*, *Live site*) has
  **Pick an area to comment on**. The side dims; a drag marks the area (at least 8 px; Esc
  cancels). The frames are other origins, so the pixels come from sharing this tab
  (`getDisplayMedia`, *this tab* preferred): the browser asks once, the stream is kept while
  in use and stopped after 5 idle minutes or on leaving the page. The area is mapped from CSS
  pixels to the capture's own size (device pixel ratio, a scaled capture), capped at 1600 px
  on its longest side, and sent as PNG (JPEG at 0.85 above 1 MB). A popover at the selection
  takes a note (**Add to queue**, ⌘/Ctrl+Enter). Without screen sharing (unsupported or
  refused) a note alone can still be queued.
- **The queue** lives per change in IndexedDB (memory when storage isn't available), so a
  reload keeps it. **Queued (n)** in the card's top strip opens it: thumbnail, editable note,
  page and side (click to open that page), remove, and **Send comments** (one review, event
  `COMMENT`). **Give greenlight** and **Send back** carry the queue too (send back still needs
  its summary). Sending clears it; a failure keeps it.
- **Images on comments**: the Conversation composer takes images from its attach button,
  paste or drag and drop (PNG, JPEG, GIF or WebP, 5 MB each, 10 per message), shown as
  thumbnails with ×. Sending stays optimistic: the pending message shows its own images.
- **Where images are stored**: in the same repository, on an orphan branch
  `orso-review-assets` (constant; never merged, sharing no history with the site), under
  `shots/{pr}/{date}/{uuid}.{ext}` and `uploads/{pr}/{date}/{uuid}.{ext}`, committed through
  the Git Data API (blobs → tree → commit → create or fast-forward the ref, retried once when
  another upload landed first). The change's own branch and `main` are never touched. Markdown
  links them as `https://github.com/{repo}/blob/{commit}/{path}?raw=true`, which GitHub shows
  to anyone who can see the repository, private ones included. **Anyone with read access to the
  repository can see these images.**
- **Showing them in orso**: review markdown points those links at
  `GET /api/projects/{id}/reviews/assets?path=&sha=`, which reads the file with the signed-in
  account (so private repositories work), only for `shots/`/`uploads/` paths and by commit,
  and serves it as the image type its bytes are (`nosniff`, cached as immutable).
- **Daemon endpoints**: `POST …/reviews/{n}/review-with-shots`
  `{ event: COMMENT|APPROVE|REQUEST_CHANGES, body, headSha, items: [{ note, path, side,
  viewport, imageBase64?, mime? }] }` (at most 10, PNG/JPEG checked by their bytes, 2 MB
  each; the head is checked before anything is stored, so a moved change (409 `head_moved`)
  stores nothing; greenlight and send back keep their rules). `POST …/reviews/{n}/comments`
  accepts `images: [{ base64, mime, name }]` (blank text allowed with images) and appends an
  `![image](…)` line per image. A repository that refuses the branch (no push access, a
  ruleset restricting branches: 403/404/422) answers 403 `assets_refused`; the message then
  offers **Send without images**, **Copy image** and **Open on GitHub**.
- **Cleanup**: the images of pull requests that are no longer open are removed from the
  branch in one commit, on inbox load (at most every 5 minutes per repository) and after
  publishing; when no open change has images left the branch is deleted, so its history stops
  riding along with `git clone` (which fetches every branch). History is not squashed. The
  branch can be deleted by hand at any time; the next upload creates it again (images in
  existing comments then stop loading).

## Testing

- **Rust unit:** inbox filtering (label, draft, own); myState from reviews vs head SHA;
  `canDecide` reasons; decision body carries `commit_id`; moved head → 409 and no
  submit; send-back without comment → 400; `changes_requested` mapping; preview state from
  deployment and deploy check, with the live URL; preview of a change not waiting → 404.
- **Web unit:** inbox rows and badge count; disabled buttons show their reason; send-back
  requires a comment; head-moved message reloads; `PrMenu` Sent back view; preview states
  and This change / Live site / Side by side; BrowserFrame opens one path on every side,
  reloads, sizes and fits each side, refuses non-http URLs.
- **E2E** (GitHub stub + a stub PR head ref in the bare remote + a stub deployment whose
  page the stub serves): open Review → see the PR → open it → the deploy preview shows,
  diff lists the file → Give greenlight → stub received
  `APPROVE` with the right `commit_id` → reviewer's own checkout and branch unchanged.
  Second spec: Send back with comment → creator's project shows **Sent back** and the
  comment.

## Delivery (one PR per slice, each user-visible)

1. **Inbox**: Review tab, list, badge, open view with header + What changed (no
   preview, no decisions).
2. **Decisions**: Give greenlight / Send back with `commit_id` pinning and `head_moved`;
   creator's **Sent back** state and comment.
3. **Preview**: the change's deploy preview beside the live site in a shared
   BrowserFrame (This change / Live site / Side by side). No local build for reviews;
   conflict previews build in the `orso.json` root and remove their worktree after each
   build.

## Out of scope

- Inline code comments, replies and threads, suggestions, editing or deleting comments,
  and reactions (all stay on GitHub). Issue events beyond merge/close (reopened,
  labelled, assigned…) are not shown: they would need another paginated call.
- A cross-site inbox; notifications outside orso.
- Building anything locally for a review, including `main` (the live site is the "before").
- Merging on the creator's behalf (unchanged: creators publish their own changes).
- Reviewing PRs that orso did not label.
