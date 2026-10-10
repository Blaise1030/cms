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
    cargo build --release --example fake_acp_agent
    pnpm -C e2e install
    pnpm -C e2e exec playwright install chromium
    pnpm -C e2e test
    pnpm -C e2e e2e
