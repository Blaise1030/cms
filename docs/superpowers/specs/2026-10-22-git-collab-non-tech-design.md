# Git collaboration for non-technical teammates (spec)

Status: **approved, 2026-10-22**. Decisions below were settled in conversation
with the product owner.

Builds on:

- `2026-10-04-phase2-changes-preview-review-design.md` (changes, draft PRs, preview)
- `2026-10-15-agent-conflict-resolution-design.md` (agent combines sync collisions)
- `2026-10-10-visual-conflict-resolution-design.md` (Advanced / manual fallback)
- `2026-10-18-creator-publishes-owner-greenlight-design.md` (in-app Publish; see
  **Relationship to greenlight** below)

## Problem

Internal company writers (blog, help docs, marketing) share a GitHub-backed site but
should not maintain any collaboration server, learn git, or visit GitHub for daily
work. Today the hard parts are: getting onto a project the first time, publishing
without github.com, keeping the agent from switching drafts mid-chat, and surviving
rare same-page collisions without seeing “merge conflict.”

## Goal

Teammates with GitHub accounts (required for access) collaborate asynchronously via
GitHub as the only shared store. Success: invite → open project → edit on a change →
sync when needed (agent combines) → **Publish** in-app → live, using only words like
**project, change/draft, get latest, combine, publish, live**. No hosted collab
backend; no live co-editing.

## Users & constraints

- **Writers** — blog, help docs, marketing; peers on the same company sites.
- **Publish policy** — anyone on the team may publish their own change (no owner
  greenlight required for this audience).
- **Collisions** — usually different posts/pages; same-page overlap is rare but must
  be handled safely when it happens.
- **Infrastructure** — GitHub only; no server-side collaboration service to operate.
- **Accounts** — writers may not have used GitHub before, but they will be given
  accounts and repo access as part of onboarding.

## Settled vocabulary

| User word | Mechanism underneath |
| --- | --- |
| Project / site | GitHub repo |
| Change / draft | `content/<slug>` branch |
| Save / checkpoint | Commit (existing checkpoints) |
| Get latest / Sync with live site | Fetch + merge from `main` |
| Combining… | Agent auto-resolve on collision |
| Share preview | Push + draft PR |
| Publish | Merge change into `main` (deploy as configured) |
| Clean up | Delete merged change branch |
| Live site | `main` (never said as “main” in UI) |

Never surface in normal UI: branch, commit, merge, conflict, PR, checkout, clone.

## Relationship to greenlight

`2026-10-18-creator-publishes-owner-greenlight-design.md` remains valid for sites that
want an owner gate. **This design targets internal teams that chose anyone-publishes:**
greenlight UI stays off (equivalent to the solo / no-distinct-owner path in that
spec). GitHub branch protection should allow collaborators to merge via the app’s
Publish flow. If protection later blocks merge, show a plain error — do not dump the
writer into GitHub review UI as the happy path.

## Agent must not switch changes

Confusion rule: **only the human switches drafts** (Change picker).

1. While a chat/run is active (and whenever a change is selected), agent tooling must
   not `checkout` or otherwise switch the project change. Soft prompt guidance is not
   enough; enforce a hard lock.
2. Follow-ups stay on the active change. Unrelated work → agent proposes starting a
   **new** change; user confirms before any switch.
3. Agent never edits the live site (`main`). If the checkout is live, refuse and ask
   to create a change.
4. If the user switches changes mid-chat: pause or clear the run and ask plainly
   whether to continue on the new draft. No silent continue on the wrong change.

## Switching changes (user-initiated)

In-place checkout of the selected `content/*` change (existing model; no per-change
worktree for normal editing).

| Step | Behavior |
| --- | --- |
| Guard | Dirty tracked tree → “Save or discard before switching drafts” (existing) |
| Checkout | Switch project folder to that change |
| Deps | Reinstall only if lockfiles / package manifests / `orso.json` differ |
| Preview | Restart preview server only if install ran or server config changed; else reload |
| UI | Reload file tree, editor, preview for the new change |

Slow-path toasts: “Opening this draft…” / “Updating tools for this draft…” — never
“npm install” or “restarting server.”

## Sync collisions (agent-owned)

Default and only normal path: agent combines, per
`2026-10-15-agent-conflict-resolution-design.md`.

1. User clicks **Get latest** / **Sync with live site**.
2. On collision: dialog — *“Someone else also updated this site. Combining your
   changes…”* (stage lines as today).
3. Agent works in a temporary worktree; user’s draft and preview stay untouched until
   success.
4. Needs input → short plain questions in SyncDialog (not file diffs).
5. Daemon verifies (no markers, build OK) → *“Done. Your changes and the latest site
   are combined.”*
6. Failure → *“Nothing changed. Your draft is safe.”* + Try again. Manual path only
   via a small **I’ll choose myself** (Advanced) control after failure — never the
   primary experience. Do not say “merge conflict.”

## Onboarding (friction A)

1. **Owner (once):** Add teammate to the GitHub repo (or org team). Optional light
   in-app **Invite**: checklist + link that opens GitHub’s invite; ACL stays on GitHub.
2. **Writer first run:** Sign in with GitHub → list accessible projects → open project
   (“Opening [site]…”) → app clones/opens locally → steer into **Create a change**
   before editing.
3. Daily work must not require the terminal, `git clone`, or github.com.

## Publish (friction C)

1. Edit on a change; optional **Share preview**.
2. **Publish** in-app merges the change to live; success: *“It’s live.”*
3. Offer **Clean up** afterward.
4. Publish errors: one plain sentence + one next step (e.g. ask an owner to fix repo
   access/protection). Prefer configuring GitHub so collaborators can publish via the
   app rather than teaching PR review.

## Error copy (canonical)

| Situation | Message direction |
| --- | --- |
| No repo access | “Ask an owner to add you to this site.” |
| Switch while dirty | “Save or discard before switching drafts.” |
| User switched mid-chat | “You switched drafts — this chat was on [old]. Continue on [new]?” |
| Combine failed | “Couldn’t combine. Your draft is safe.” |
| Publish failed | “Couldn’t publish…” + one concrete next step |

## Out of scope

- Live co-editing / CRDTs
- Hosted shared working copy (server-side collab)
- Requiring greenlight for this internal-team profile
- Full Writer mode shell (hide Code/agent) beyond jargon-free paths above
- Replacing GitHub accounts with a different identity provider

## Testing (acceptance)

1. New GitHub user invited to repo can sign in, open project, create a change, edit,
   and publish without visiting github.com after the invite.
2. Agent cannot switch changes during an active prompt; mid-chat human switch prompts
   before continuing.
3. Switching between two changes with identical lockfiles does not reinstall; a change
   that alters the lockfile does reinstall/restart once, with soft toasts only.
4. Forced sync collision: agent combines successfully; writer never sees “conflict” /
   “merge” / “branch.” Failure leaves draft unchanged and offers Try again / I’ll
   choose myself.
5. Two writers on different posts: both publish independently without coordinating.
`}