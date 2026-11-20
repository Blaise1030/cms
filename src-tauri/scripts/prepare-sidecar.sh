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
# tauri-build copies the sidecar next to the app only when its build script reruns, which a
# changed daemon alone doesn't trigger; refresh that copy too so the app never runs a stale one.
if [ -d "$here/target/$profile" ]; then
  cp "$here/binaries/orso-$triple" "$here/target/$profile/orso"
fi
echo "sidecar: binaries/orso-$triple ($profile)"
