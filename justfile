set shell := ["zsh", "-cu"]

default:
    @just --list

# Run daemon and Vite dev server (daemon in background)
dev:
    cargo run -- serve & pnpm -C web dev

# Build SPA, then the release binary
build:
    pnpm -C web build
    cargo build --release

# Run all tests / build checks
test:
    cargo test
    pnpm -C web build
    pnpm -C templates/astro-marketing build
