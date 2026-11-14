# ORSO — how we run the launch

The operating model for a launch run by one person and three agents: the **Chief of Staff** (plans,
briefs, reviews), the **Marketer** (content and assets), and the **Product Engineer** (code).

The premise: agents do not share a conversation. They share **files**. So every instruction,
decision and piece of work has to exist as a file in this repo, or it did not happen.

---

## 1. The one rule

> If it is not a file in `docs/launch/`, it is not real.

Chat is for thinking. Files are for decisions. A brief that only exists in a conversation will be
forgotten by the next agent session, and by you by Thursday.

---

## 2. The unit of work: the brief

Everything starts as a brief in `docs/launch/briefs/`, named `NNN-slug.md`. A brief has five parts,
in this order:

1. **Goal** — one sentence, in outcome language.
2. **Done when** — a checklist a stranger could verify.
3. **Inputs** — the files, links and issues to read first.
4. **Guardrails** — what not to touch, what not to decide.
5. **Output** — where the result goes.

No brief, no work. If an agent finds itself working without one, it writes one and asks for
approval first. This is the single most important habit in the system: it stops scope drift before
it becomes a rewrite.

Template: [`briefs/TEMPLATE.md`](briefs/TEMPLATE.md).

---

## 3. The offload mechanism: the decision log

The user has a full-time job. Interrupting him is the expensive failure, and guessing is the
expensive mistake. The resolution is a **decision file**:

```
docs/launch/decisions/NNN-short-question.md
```

Each one contains: the question, **a recommendation**, the alternatives, and the default if nobody
answers by a stated date. Agents write them; they never ask in chat unless the house is on fire.

The user reviews the queue **Mondays and Thursdays, 45 minutes, in one batch**. Decisions are
appended to the file once answered — the log is never rewritten, so we can see what we believed and
when.

Rule of thumb: if an agent has asked more than three questions in a week, the briefs are too thin.
Fix the briefs, not the agent.

---

## 4. The weekly rhythm

| Day | What happens | Artifact |
|---|---|---|
| **Mon** | Chief of Staff writes the week's briefs and the three priorities | `docs/launch/weekly/YYYY-WW.md` |
| **Mon–Thu** | Agents execute briefs; decisions accumulate | briefs + decision files |
| **Thu** | 45-minute decision batch; recompute the launch gate | answers in decision files |
| **Fri** | 20-minute demo: each agent shows one thing that ships | notes appended to the weekly file |

Friday demo is not optional. It is the counter to independent agents quietly building the wrong
thing for two weeks. If an agent has nothing to show, that is the signal — not the excuse.

---

## 5. Definition of done

**Code (Product Engineer).** Acceptance criteria met · `just test` green · new behaviour has a test
· docs updated for user-facing change · PR under ~400 lines · PR body shows how it was verified,
with a screenshot or GIF if visible · linked to its issue.

**Content (Marketer).** A file at a real path · stated reader, channel and call to action · every
factual claim logged in `claims.md` with evidence · voice checked against `tabsh.cc` · no
placeholders, no invented numbers.

**A brief is done** when its "Done when" checklist is entirely ticked by someone other than the
person who did the work.

---

## 6. The launch gate

One file, one source of truth: [`launch-gate.md`](launch-gate.md). Recomputed every Thursday.

No agent may declare the launch ready. The gate is a checklist; the user closes it.

---

## 7. The claims ledger

`claims.md` is the bridge between marketing and product, and the reason a launch does not
overpromise.

Every claim the Marketer wants to make is a row: the sentence, where it appears, the proof (commit,
screenshot, command), and a status of `proven`, `unproven` or `cut`. The Product Engineer can flip
a claim to `proven` by shipping it, but only with evidence attached.

If the site says something the build does not do, the site is wrong. Always.

---

## 8. The board

Two lanes on one board (tabsh), tagged rather than separated:

- `lane:content` — Marketer. Cards are briefs.
- `lane:product` — Product Engineer. Cards are issues.
- `needs-your-input` — the decision queue. This column is the only place the user is required.
- `blocked` — with the reason in the card title, never just "blocked".

The board is a view, not a record. The briefs and issues are the record.

---

## 9. Failure modes and their counter

| Failure | What it looks like | The counter |
|---|---|---|
| Scope drift | Agent builds something impressive nobody asked for | The brief's guardrails section |
| Silent divergence | Two agents assume different product states | `claims.md` + the Thursday gate recompute |
| Question drip | Small asks trickling in all week | Decision files, batched twice weekly |
| Overpromise | Site promises what the build does not do | Claims require evidence to be `proven` |
| Review bottleneck | User becomes the slowest step | Briefs carry recommendations, not open questions |
| Agent amnesia | Same context re-explained every session | `LONG_TERM_MEMORY.md`, updated weekly |

---

## 10. Start here

First three briefs, in order:

1. `001-windows-build.md` — Product Engineer. The launch blocker.
2. `002-landing-page.md` — Marketer. Rewrite the site around the one-sentence position.
3. `003-docs-and-demo.md` — Marketer + Product Engineer. Docs good enough to launch, and the
   three-minute demo.
