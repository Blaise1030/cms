set shell := ["zsh", "-cu"]

default:
    @just --list

# Install web and template dependencies
setup:
    pnpm -C web install --frozen-lockfile
    npm --prefix templates/astro-marketing ci

# Daemon (API on :7800) + Vite dev server (HMR, proxies /api /ws /preview to the daemon).
# The daemon's ready link points at Vite (:5173), not :7800, which serves the embedded
# `web/dist` build and never hot-reloads. Ctrl-C stops both.
dev:
    trap 'trap - INT TERM EXIT; kill 0' INT TERM EXIT; \
    cargo run -- serve --ui-url http://localhost:5173 & \
    pnpm -C web dev & \
    wait

# Desktop shell in dev mode: `tauri dev` (rebuilds and relaunches on src-tauri changes) with a
# debug daemon, and the title-bar-less window showing Vite with HMR. Ports default clear of
# `dev`, so both can run. Daemon (src/) changes need a restart. Ctrl-C stops everything.
desktop-dev vite_port="5180" daemon_port="7810":
    src-tauri/scripts/desktop-dev.sh {{vite_port}} {{daemon_port}}

# Build the SPA (+ PWA service worker, checked), then the release binary that embeds it
build:
    pnpm -C web install --frozen-lockfile
    pnpm -C web build
    cargo build --release

# Everything: web build first (the daemon embeds web/dist), then Rust, web unit tests, template
test:
    pnpm -C web install --frozen-lockfile
    pnpm -C web build
    cargo test
    pnpm -C web test
    npm --prefix templates/astro-marketing ci
    npm --prefix templates/astro-marketing run build

# Playwright smoke e2e (rebuilds the daemon so `web/dist` is embedded).
e2e:
    pnpm -C web install --frozen-lockfile
    pnpm -C web build
    cargo build --release
    pnpm -C e2e install
    pnpm -C e2e exec playwright install chromium
    pnpm -C e2e test
    pnpm -C e2e e2e
