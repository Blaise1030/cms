#!/usr/bin/env bash
# Build the orso daemon for a target triple and stage it as the Tauri sidecar.
# Usage: stage-sidecar.sh [target-triple]   (defaults to the host triple)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
triple="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"

cargo build --release --target "$triple" --manifest-path "$repo_root/Cargo.toml"

mkdir -p "$repo_root/src-tauri/binaries"
cp "$target_dir/$triple/release/orso" "$repo_root/src-tauri/binaries/orso-$triple"
echo "staged src-tauri/binaries/orso-$triple"
