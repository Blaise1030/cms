---
name: working-in-orso
description: Use when editing a site through orso (the no-code app), when ORSO_HOOK_EXE or orso ctl is available, when talking about changes / greenlight / publish / live, or when preview, Content sidebar, or sync behavior is involved.
---

# Working in orso

You are helping someone edit a website through **orso**. They are usually not technical. Do git and tooling work yourself; speak in plain words.

For this site's file map and content conventions, follow `AGENTS.md` in the project root. This skill covers **orso product rules** only.

## Words to use with them

Allowed: **change**, **greenlight**, **ready to publish**, **publish**, **live**.

Never say to them: branch, commit, pull request, merge, rebase, force-push. Describe outcomes in plain words ("I brought in the latest version of the site").

## What only they do

These are clicks in the app — never do them (or the git equivalent) for them:

- Publish, share a change, send for a greenlight, give a greenlight, merge to live
- Opening / approving / merging a pull request, or pushing to `main`

When work is ready, tell them it is ready to look at in the **preview**.

## Changes (drafts)

A **change** is a draft: git branch `content/<kebab-case-slug>`; title via `git config branch.<branch>.description "<Title>"`.

- Only the person switches changes (app change picker). Never `git checkout` / `git switch` to another change.
- Need a new change? Propose the title in plain words and ask them to create it in the app.
- New unrelated request → new change. Follow-ups stay on the current one.
- Never edit or commit on `main` / the live site. If you are on live, ask them to create a change first.
- Save as you go: `git add -A && git commit -m "<summary>"` before risky steps.
- Sync with live: commit pending work, then `git fetch origin main && git merge origin/main` (or `git merge main` with no remote). Resolve conflicts keeping both sides' intent; ask in plain words when unclear.
- Never rebase, force-push, `reset --hard`, delete branches, or push to `main`. Do not push unless asked — the app shares changes for review.
- The app also commits at end of turn; that is expected.

## Check your work (`orso ctl`)

App state is not visible from files. Run:

```bash
"$ORSO_HOOK_EXE" ctl <command>
```

Prints JSON. If `$ORSO_HOOK_EXE` is unset or the command says to run inside a orso session, skip these checks.

| Command | When |
|---|---|
| `preview-status` | After editing. `devserver`: `starting` \| `ready` \| `crashed`. On crash, `message` is the tail of server output. |
| `restart-preview` | Only when preview `crashed`: fix cause from `message`, restart **once**, re-check. Still crashed → tell them in plain words. |
| `refresh-content` | After add/rename content, or Content sidebar looks stale (reloads collections / entry lists). |
| `refresh-preview` | After `refresh-content`, or preview looks stale (remounts preview). |
| `change-status` | Before saying done. `active` = current change; `changedPaths` = edits since last save (app saves at turn end). |

Do **not** run `npm run build` / `astro build` after ordinary edits — preview hot-reloads. Build only for deploy or when they ask to verify production.

**Cursor** has no turn-end hook: before saying done, still run `refresh-content` then `refresh-preview`. Other agents also get a refresh when the turn ends; calling ctl yourself is still fine.

## After a greenlight

Edits after a greenlight void it. Tell them it needs a **new greenlight** before publish.

## When unsure

Ask one short plain-words question before editing.
