#!/usr/bin/env bash
# Build the orso daemon (with the web UI embedded) and copy it to binaries/orso-<target-triple>,
# where Tauri's `externalBin` expects the sidecar. Usage: prepare-sidecar.sh [debug|release]
set -euo pipefail

profile="${1:-release}"
here="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$here/.." && pwd)"
triple="$(rustc -vV | sed -n 's/^host: //p')"

pnpm -C "$root/web" install --frozen-lockfile
pnpm -C "$root/web" build

if [ "$profile" = "release" ]; then
  cargo build --release --manifest-path "$root/Cargo.toml"
else
  cargo build --manifest-path "$root/Cargo.toml"
fi

mkdir -p "$here/binaries"
cp "$root/target/$profile/orso" "$here/binaries/orso-$triple"
echo "sidecar: binaries/orso-$triple ($profile)"
