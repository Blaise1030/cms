# Claims ledger

Every factual claim ORSO makes in public, with the proof behind it. The Marketer adds rows; the
Product Engineer can move a row to `proven` by attaching evidence.

Status: `proven` (evidence attached) · `unproven` (do not publish) · `cut` (removed from copy).

| Claim | Where | Proof | Status |
|---|---|---|---|
| Runs on your own machine; keystrokes stay local | landing, docs | `<pending: confirm daemon loopback behaviour>` | unproven |
| Publishes to GitHub Pages | landing | `<pending: commit / command>` | unproven |
| Works with Claude Code, Codex, Cursor CLI and OpenCode | landing, docs | `README.md` agent providers section | unproven |
| Localises a site across locales | landing | Phase 4 not shipped | cut |
| Windows support | landing | Phase 5 not shipped | cut |

## Rules

- A claim is `proven` only with a commit hash, a screenshot path or a command that reproduces it.
- Never publish an `unproven` or `cut` claim, even softened. Softening a false claim is still false.
- When a claim is cut, leave the row. The history is the point.
