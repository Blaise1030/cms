# Playwright e2e

End-to-end tests run against a real `workbench serve` process, the built web UI, a local bare
git remote, and a stubbed GitHub API.

## Run

From the repo root (after `pnpm -C web build` so the daemon embeds `web/dist`):

```bash
just e2e
```

## Fixtures

| File | Role |
|------|------|
| `fixtures/githubStub.ts` | HTTP stub for device flow, `/user`, `/user/repos`, pulls / PR status, GraphQL `markPullRequestReadyForReview` |
| `fixtures/bareRemote.ts` | Seeds `acme/site.git` with a `main` branch |
| `fixtures/daemon.ts` | Starts stub + bare remote + release `workbench serve` (+ fake ACP agent), clones via API |
| `fixtures/workbench.ts` | Playwright fixture wiring cleanup |

Fixture helpers create temp dirs under the OS temp directory and remove them in `stop()`.

## Selectors

Prefer stable `data-testid` attributes (`files-panel`, `preview-panel`, `project-grid`,
`project-${id}`, …). Avoid CSS classes or DOM structure.

The default workspace tab is Preview (`/p/:id/preview`); smoke (and later specs) click the
**Code** tab before asserting `files-panel`.

## Unit tests

Vitest covers stub and bare-remote helpers:

```bash
pnpm -C e2e test
```
