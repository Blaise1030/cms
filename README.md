# CMS

## Development

### Prerequisites

- [`just`](https://github.com/casey/just)
- Rust toolchain (`cargo`)
- Node.js with `npm` and `pnpm`
- GitHub CLI (`gh`), the easiest way to sign in to GitHub; see [docs/github-setup.md](docs/github-setup.md)

Before sharing previews, protect `main` so merges need review; see
[docs/branch-protection.md](docs/branch-protection.md).

### Run in dev mode

```sh
just setup   # first time: install web and template dependencies
just dev     # start the daemon and the Vite dev server
```

`just dev` runs two processes:

- **Daemon**: `cargo run -- serve --ui-url http://localhost:5173`, which serves the API on `:7800`
- **Vite dev server**: `pnpm -C web dev`, which runs on `:5173` with hot module reloading and proxies `/api`, `/ws` and `/preview` to the daemon

Open **http://localhost:5173**, which is where the daemon's ready link points. Port `:7800` serves the embedded `web/dist` build, which does not hot-reload. Ctrl-C stops both processes.

Without `just`, run these in two terminals:

```sh
cargo run -- serve --ui-url http://localhost:5173
pnpm -C web dev
```

### Other tasks

```sh
just build   # build the SPA, then the release binary that embeds it
just test    # web build, Rust tests, web unit tests, template build
```
