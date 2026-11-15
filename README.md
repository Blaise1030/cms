# orso

A site builder that runs on your own machine. Edit your site's pages in a visual editor, ask a
coding agent to change things, preview every change live, and publish through a normal GitHub
pull request.

orso is a single `orso` binary (a Rust daemon with the web UI built in). Your site stays a
plain Astro project in a Git repo, so nothing locks you in.

Free and open source under the [MIT licence](LICENSE). The repo path is still
`Blaise1030/cms`; the product is **orso**.

## What you need

- **macOS or Linux.** Windows is not supported yet.
- **A GitHub account.** orso signs you in with GitHub's device flow and opens pull requests for
  you. If your site already lives on GitHub, you need write access to that repo.
- Nothing else. orso downloads and pins its own Node and agent tools on first run.

## Install

**macOS (recommended): Homebrew**

```sh
brew install blaise1030/tap/orso
orso serve --open
```

**macOS or Linux: installer script**, which downloads the matching release and verifies its
sha256:

```sh
curl -fsSL https://raw.githubusercontent.com/Blaise1030/cms/main/scripts/install.sh | sh
orso serve --open
```

Pin a version with `ORSO_VERSION=0.1.5`, or change the install directory with
`ORSO_INSTALL_DIR`. On macOS, prefer Homebrew: a downloaded binary can pick up a Gatekeeper
quarantine flag (the installer clears it when it can).

`orso serve --open` opens `http://orso.localhost:<port>/?t=…` in your browser. The daemon only
listens on `127.0.0.1`, and the `?t=` token in that link is what lets your browser in. Keep the
terminal open while you work. Ctrl-C stops orso.

## Getting started

1. **Create or open a project.** Choose **New project**, then start from the built-in Astro
   marketing template or pick an existing repo from GitHub. Sign in to GitHub when asked.
2. **Look around.** The left rail has the project tabs:
   - **Preview**: your site running live, with the page you are on.
   - **Content**: blog posts, docs and other content collections, with a form for each entry's
     fields and a rich-text editor for the body. Images can be pasted, dropped or uploaded.
   - **Design**: site-wide colours, fonts and other design tokens.
   - **Code**: the project's files.
   - **History**: saved versions, and what changed in each.
   - **Settings**: project settings.
3. **Make a change.** Edits happen on a *change*, a Git branch with a readable title, never
   directly on the live site. orso asks you to start one the first time you edit.
4. **Ask the agent** (optional). Open the assistant and describe what you want ("add an FAQ
   section to the pricing page"). It edits the project on your current change. **OpenCode** is the
   easiest start: it runs free models and needs no account. Claude Code, Codex and Cursor are also
   supported if you have accounts.
5. **Share and review.** **Share preview** pushes your change and opens a pull request. Mark it
   ready for review when you are done. With the template's deploy workflow, every pull request
   gets its own preview URL.
6. **Publish.** Once the pull request is approved, publish from orso. That merges it to `main`,
   which deploys the live site. **Get latest** pulls in what others have published. If someone
   edited the same lines, orso shows the conflict and can hand it to the agent to resolve.

## Use orso with an existing site

orso works best with sites started from its template, but an existing Astro site works too
once it meets these requirements:

| Needs | For |
| --- | --- |
| Default branch `main` | Changes, sync and publishing |
| Astro `>=7.3.0 <8`, with a `dev` script | Live preview |
| `base` and the HMR socket read `PUBLIC_BASE` / `ORSO_DEV_PORT`; links use `BASE_URL` | Live preview under `/preview/{id}/` |
| orso's integration copied into `integrations/orso/`, `orso()` in the config, plus `yaml`, `js-yaml`, `jsonc-parser`, `linkedom` | Content tab and page checks |
| Collections loaded with `orsoGlob` (`*.md`, `*.mdx` or `*.json`, with a schema) | Editable content |
| `DESIGN.md` at the root | Design tab |
| A deploy that records GitHub Deployments with an `environment_url` | Preview and Open live site links |
| `orso.json` when you use pnpm/yarn or the site isn't at the repo root | Install and dev commands |

Let your coding agent do it. Install the skills, then ask it to "set up this site for orso":

```sh
npx skills add Blaise1030/cms --skill setup-orso       # adopt or update orso in a repo
npx skills add Blaise1030/cms --skill working-in-orso  # day-to-day editing rules
```

The skill checks the repo, wires everything in on a branch, migrates sites still using the
old `orso-astro` package, and verifies the dev server and content manifest before it reports.

## Publishing your site

The built-in template deploys to **Cloudflare Workers** through GitHub Actions
(`.github/workflows/deploy.yml` in the new project). Pushes to `main` go live, and each pull
request gets a stable preview URL. One-time setup in the repo's **Settings → Secrets and
variables → Actions**:

- secret `CLOUDFLARE_API_TOKEN`: an API token with *Workers Scripts: Edit*
- secret `CLOUDFLARE_ACCOUNT_ID`
- variable `CLOUDFLARE_WORKER_NAME` (optional; defaults to the name in `wrangler.toml`)

orso reads the resulting GitHub deployments to show **Open live site** and the preview link.
Any other host works as long as it deploys from `main`.

Before you share previews with others, protect `main` so merges need a review. See
[docs/branch-protection.md](docs/branch-protection.md).

## Troubleshooting

- **The page says the daemon is unreachable.** orso isn't running, or the token is stale. Run
  `orso serve --open` again and use the new link.
- **GitHub sign-in fails or repos are missing.** See [docs/github-setup.md](docs/github-setup.md).
  Make sure you have write access to the repo.
- **The assistant won't start.** Open the assistant setup screen and reinstall the provider.
  orso keeps its tools under `~/Library/Application Support/orso/runtimes` (macOS) or your XDG
  data dir (Linux). See [docs/agent-runtimes.md](docs/agent-runtimes.md).
- **Anything else:** [open an issue](https://github.com/Blaise1030/cms/issues) with your
  `orso --version` and what you clicked.

## Development

### Layout

| Path | Role |
| --- | --- |
| `src/` | Daemon (`orso` binary): API, preview, agents, publish, sync |
| `web/` | Vue SPA (embedded in release builds; Vite in dev), built on Nuxt UI |
| `packages/astro/` | `orso-astro`, the Astro integration that powers content editing |
| `e2e/` | Playwright tests against a release-built daemon |
| `templates/` | Starter site templates |
| `scripts/install.sh` | Installer for GitHub Release tarballs |

### Prerequisites

- [`just`](https://github.com/casey/just)
- Rust toolchain (`cargo`)
- Node.js with `npm` and `pnpm`
- GitHub CLI (`gh`) for GitHub sign-in in development

### Run it

```sh
just setup   # first time: install web, orso-astro and template dependencies
just dev     # daemon on :7800, Vite on :5173
```

Open **http://localhost:5173**. Vite proxies `/api`, `/ws` and `/preview` to the daemon. Port
`:7800` serves the embedded `web/dist` build (no HMR). Ctrl-C stops both.

In this dev UI the preview frame loads the project's dev server directly (`127.0.0.1:<port>`),
so Astro islands hydrate. The daemon-served build frames the proxied preview instead. Because the
direct frame is cross-origin, the element picker and appearance controls use a small
`postMessage` bridge that `orso()` injects into dev pages. The page-error overlay and address-bar
history still need the proxied preview.

Without `just`:

```sh
cargo run -- serve --ui-url http://localhost:5173 --open
pnpm -C web dev
```

### Agent providers

The assistant runs a coding CLI through the daemon over ACP. Built-in providers: OpenCode,
Claude (`claude`), Codex (`codex`) and Cursor CLI (`cursor-agent`). orso installs and pins its own
Node, provider CLIs and Claude/Codex ACP adapters, verified and staged under the runtimes
directory above. A CLI you installed yourself is used as a fallback. Override providers in
`~/.config/orso/agents.json`, or pass `--agents-config`.

### Build and test

```sh
just build   # SPA (+ PWA check), then the release binary that embeds web/dist
just test    # web build, Rust tests, web unit tests, orso-astro package and template tests
just e2e     # Playwright (rebuilds the release daemon with the embedded UI)
```

### Releases

GitHub Releases ship daemon tarballs (`orso-v*-{target}.tar.gz` + sha256) for macOS and Linux,
and the Homebrew formula is updated from the same workflow.

## Licence

[MIT](LICENSE) © 2026 Blaise Tiong
