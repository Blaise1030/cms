# Brief 002 — Landing page

- **Owner:** Marketer
- **Opened:** 14 Nov 2026
- **Status:** draft — needs approval
- **Blocks:** launch gate #4, #6

## Goal

A freelance developer who has never heard of ORSO understands what it is, why it is different from
a hosted CMS, and installs it — in under ninety seconds of reading.

## Done when

- [ ] One-sentence position on the page, above the fold, in the user's voice
- [ ] A "why this exists" section that names the enemy (hosted CMS, lost content, agents you cannot
      watch work) without naming competitors rudely
- [ ] Three proof points, each with a screenshot or GIF **of the real app**
- [ ] An install block that works: one command, copy button, platform labelled honestly
- [ ] Comparison section: ORSO vs a hosted CMS vs a git-based CMS — factual, no strawmen
- [ ] Meta title, description, OG image, JSON-LD, `llms.txt`, markdown mirror
- [ ] Every claim present in `claims.md` and `proven`
- [ ] Copy reviewed against `tabsh.cc` for voice, sentence length and tone
- [ ] No mention of localisation or Windows until those ship

## Inputs

- `tabsh.cc` and `blaisetiong.dev` — the voice and the standard
- `README.md` — what the product actually does
- `docs/launch/claims.md` — the evidence bar
- The live app: run `just dev` and use it. Do not write about a product you have not opened.

## Guardrails

- Do not invent users, testimonials, logos, benchmarks, or "trusted by".
- Do not promise Windows, localisation or pricing. Those are open decisions.
- Do not edit `src/` or `web/`. Copy and assets only. (`src-tauri/` is unsupported.)
- Do not use the banned word list in your instructions.

## Output

Copy as files in `docs/launch/site/` (page by page, markdown), plus a screenshot brief for each
image needed. A PR, not a commit to `main`.

## Notes

Nothing yet.
