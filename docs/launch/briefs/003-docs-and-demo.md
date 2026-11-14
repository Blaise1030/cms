# Brief 003 — Docs and the three-minute demo

- **Owner:** Marketer (lead) with Product Engineer (assists)
- **Opened:** 14 Nov 2026
- **Status:** draft — needs approval
- **Blocks:** launch gate #4, #5

## Goal

Someone can go from "what is this" to "my site is published" without asking a question, and the
one thing no competitor can show — an agent resolving a real conflict — is on video.

## Done when

**Docs**

- [ ] Quickstart: install, open a project, publish. Ten minutes, screenshots from the real app.
- [ ] "Bring your own Astro site" guide, including what ORSO expects of the repo
- [ ] Agent setup: which CLIs work, what ORSO installs for you, what it never sends anywhere
- [ ] One troubleshooting page built from the actual errors the app can produce
- [ ] Every code block copy-pasteable and run at least once by a human

**Demo**

- [ ] A shot list, scene by scene, with the state of the app at each cut
- [ ] Filmed on a real project, not a fixture, with real commit noise
- [ ] Shows: edit in the UI → talk to the agent → preview → conflict → publish
- [ ] Under three minutes, no music, typing sped up at most 2×
- [ ] Captioned, and the same cut reused for the Product Hunt gallery

## Inputs

- `README.md`, `docs/agent-runtimes.md`, `docs/github-setup.md`
- `tabsh.cc` docs and the tabsh demo video — the house standard for both
- Issue #22 and #35 — the editor and conflict flows the demo depends on

## Guardrails

- The demo shows only what is shipped. No mocked panes, no invented error recovery.
- Do not reshoot to hide a rough edge; cut around it or fix it.
- Product Engineer does not write marketing copy; Marketer does not edit code.

## Output

`docs/` pages as PRs. The demo script in `docs/launch/site/demo-script.md`, the video file linked
from the brief. First cut reviewed before publishing anywhere.

## Notes

Depends on #22 and #35 landing. If they slip, the demo ships with editor-only and a note, and the
launch gate stays open.
