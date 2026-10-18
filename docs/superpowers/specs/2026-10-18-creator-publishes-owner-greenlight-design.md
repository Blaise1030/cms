# Creator publishes, owner greenlights (spec)

Status: **approved in conversation 2026-10-18**. Decisions below were settled with the
product owner; no *(proposed)* markers remain.
Builds on: `2026-10-04-phase2-changes-preview-review-design.md` (changes, draft PRs,
preview links), `2026-10-15-agent-conflict-resolution-design.md` (agent resolves sync
conflicts).

## Problem

Workbench's audience is non-technical site owners and content creators, but going live
today leaks Git and GitHub mechanics: change branches, draft PRs, "Ready for review",
merging on github.com, and branch protection settings. A creator must understand pull
requests to publish a headline change, and an owner has no in-app moment to approve
what goes live. If this is not fixed, every non-technical user needs a developer
beside them at publish time — the exact step the product exists to remove.

## Goal

A creator with no Git knowledge can take a change from first edit to live, and an
owner can approve or reject it, without either person seeing a branch, commit, PR
state, or the GitHub website. Success: the full flow below completes in-app using
only the words **change, greenlight, ready to publish, publish, live**; GitHub remains
the enforcement layer underneath, unchanged in principle.

## Users & context

- **Content creator** — edits pages/posts, changes design, asks the agent for bigger
  changes. Publishes their own approved work. May be one of several creators on the
  same site, each on their own machine; collaboration is asynchronous via GitHub,
  not live co-editing.
- **Owner** — accountable for what is on the live site. Defined per site repo in
  `.github/CODEOWNERS`. Reviews changes and gives the greenlight; never publishes
  other people's work.
- **Solo creator-owner** — one person who is both. Must never be asked to approve
  their own work.

## Settled model

| User word | Mechanism underneath |
| --- | --- |
| Change | `content/<slug>` branch + draft PR into `main` (Phase 2, already built) |
| Owner | `.github/CODEOWNERS` entry for the site repo |
| Send for greenlight | Draft PR marked ready for review; GitHub auto-requests code owners |
| Greenlight | Code owner submits an **Approve** PR review |
| Send back | Code owner submits a **Request changes** review |
| Ready to publish | PR state `approved` (already computed in `src/github.rs` `PrState`) |
| Publish | The creator merges their own PR; merge into `main` deploys the site |
| History / undo | Checkpoints (`src/git.rs`) and publish history; restore as today |

Enforcement is GitHub's, not the app's invention: branch protection on `main` with
**Require a pull request before merging**, **Require approvals**, and **Require
review from Code Owners**. The app may hide unenforceable states but must never
offer Publish when GitHub would refuse the merge.

**Greenlight rule:** a change needs a greenlight only when a code owner other than
its creator exists for the touched paths. Solo sites (creator is the only applicable
code owner, or no `CODEOWNERS`) skip the step entirely. Adding a distinct owner to
`CODEOWNERS` switches the step on; the owner's own changes still skip it.

## User flow (actor does X → sees Y)

1. **Start a change.** Creator clicks **New change**, types a plain title
   ("Summer sale homepage") → app creates the change from the latest live site and
   opens Preview. No location, branch, or base is ever asked.
2. **Edit.** Creator edits in Content/Design or via the agent. Every edit and every
   settled agent turn auto-saves (checkpoint); the header shows **Saving… → Saved**.
   Preview updates continuously. Editing is impossible outside a change on
   GitHub-backed sites, and the app steers the creator into one without an error
   lecture (existing `require_change` behaviour, reworded).
3. **Share preview (optional).** Creator clicks **Share preview** → branch pushed,
   draft PR opened/updated, preview link shown (Phase 2 behaviour, unchanged).
4. **Send for greenlight.** If a distinct owner exists, creator clicks **Send for
   greenlight** → PR becomes ready for review; owner is requested automatically via
   `CODEOWNERS`. Creator sees **Waiting for greenlight**. On solo sites this step
   and button do not exist; the change is immediately publishable (step 6).
5. **Owner decides.** Owner opens the site, sees the change under **Waiting for
   your greenlight** with a before/after preview (live vs proposed) and a
   plain-English summary. **Give greenlight** → Approve review; **Send back** →
   Request changes with a comment the creator sees verbatim. Creator is notified
   either way.
6. **Ready to publish.** Approved changes (and all solo changes) appear in a shared
   **Ready to publish** list visible to every creator, showing title, creator, and
   what it touches. Every creator can therefore see everything about to go live
   before publishing their own change.
7. **Creator publishes.** Creator clicks **Publish** on their own change → app
   merges the PR, deploys, and reports **Building… → Live ✓** with a **Visit site**
   action. The change leaves Ready to publish, appears in History, and the local
   change is cleaned up (Phase 2 cleanup).
8. **Another change went live first.** When a second creator opens (or attempts to
   publish) a ready change whose base has moved:
   - *Identical result already live* (both made the same edit): the app says
     **Already live — published by \<name\>** and closes the change. No publish, no
     re-greenlight, no owner involvement.
   - *Same area, compatible edits*: the assistant folds the live changes into the
     draft (existing get-latest merge machinery), updates the preview, and shows a
     plain summary ("Updated with Tom's published changes — your photo edit is
     untouched"). Creator re-checks and publishes.
   - *Same content, conflicting versions*: the creator — never the owner — chooses
     between the live text and theirs, shown as finished rendered text, not diffs
     or conflict markers. Resolution is assisted by the agent per
     `2026-10-15-agent-conflict-resolution-design.md`; a human is asked only when
     the agent cannot reconcile.
   - Publish stays locked until one of the above completes.
9. **Edits after greenlight.** If the creator changes content after approval, the
   greenlight no longer covers the change: it returns to **Waiting for greenlight**
   (enforced by GitHub's *Dismiss stale pull request approvals when new commits are
   pushed*). Pure sync updates (step 8, compatible case, with no edit to the
   creator's own content) do not require re-approval of the creator's words, but any
   agent-suggested wording the creator accepts counts as an edit.
10. **Undo.** From History, owner or creator restores the pre-publish checkpoint /
    reverts the publish; the previous live version returns in one action.

## Acceptance criteria

- [ ] A creator can complete steps 1–7 in-app without seeing the words branch,
      commit, pull request, merge, or any github.com page.
- [ ] Solo site: no greenlight UI appears anywhere; Publish is enabled as soon as
      checks pass.
- [ ] Site with a distinct code owner: Publish is disabled before approval, enabled
      after an Approve review from a code owner, and disabled again after a
      Request changes review. An approval from a non-owner does not unlock it.
- [ ] Ready to publish lists every approved change on the site (title, creator,
      touched pages) to every signed-in creator.
- [ ] Second-publisher cases: identical edit → Already live notice and auto-close;
      compatible overlap → updated preview + plain summary, publish proceeds;
      conflicting text → creator-side choice UI with no conflict markers shown.
- [ ] Any content edit after approval returns the change to Waiting for greenlight;
      a no-content-change sync does not.
- [ ] Attempting Publish when GitHub would refuse (unapproved, failing checks,
      behind/conflicting) yields a plain-English reason and one corrective action,
      never a raw Git/GitHub error.
- [ ] Merged change is removed from Ready to publish, local cleanup runs, and the
      publish is visible in History with a working undo.
- [ ] Non-GitHub (local-only) projects keep today's behaviour: editable on `main`,
      no greenlight, no publish step.

## Out of scope

- Live co-editing / presence (CRDT); collaboration stays asynchronous via GitHub.
- Owner publishing or merging on a creator's behalf; bulk "publish all" bundles.
- In-app editing of `CODEOWNERS` or branch protection (docs + hint only, as today);
  scaffolding a starter `CODEOWNERS` is in scope as a template change, an editor UI
  is not.
- Review comments/discussion threads in-app beyond the send-back comment.
- Non-GitHub remotes; GitHub Enterprise differences.

## What exists vs what is new

**Exists:** change branches and draft PRs, Share preview, Ready for review, PR state
including `Approved` (`src/github.rs`), checkpoints and restore (`src/git.rs`),
get-latest merge planning and agent auto-resolve (`src/changes/`), branch-protection
hint and docs (`docs/branch-protection.md`).

**New:** (1) `.github/CODEOWNERS` in the `astro-marketing` template, generated for
the scaffolding user; (2) greenlight state derived from code-owner reviews and the
"distinct owner" rule; (3) Ready to publish list across changes; (4) in-app Publish
(PR merge) gated on approval/checks/up-to-date; (5) second-publisher handling in
step 8 wired into the publish path; (6) UI vocabulary pass (greenlight / ready to
publish / publish / already live); (7) branch-protection doc gains **Require review
from Code Owners** and the stale-approval dismissal.

## Open questions

- [ ] If `CODEOWNERS` names a team rather than a person, any team member's approval
      counts as the greenlight? (GitHub semantics say yes.) — owner: product owner
- [ ] Does the owner get an in-app inbox across all their sites, or per-site only
      for v1? — owner: product owner
- [ ] Should a send-back comment be mandatory? (Spec assumes yes in UI; GitHub does
      not require one.) — owner: product owner
