#!/usr/bin/env bash
# Build the orso daemon (with the web UI embedded) and copy it to binaries/orso-<target-triple>,
# where Tauri's `externalBin` expects the sidecar. Usage: prepare-sidecar.sh [debug|release]
set -euo pipefail

profile="${1:-release}"
here="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$here/.." && pwd)"
host="$(rustc -vV | sed -n 's/^host: //p')"
# `tauri build --target <triple>` sets TAURI_ENV_TARGET_TRIPLE; cross-build the daemon for it.
triple="${TAURI_ENV_TARGET_TRIPLE:-$host}"

pnpm -C "$root/web" install --frozen-lockfile
pnpm -C "$root/web" build

args=(--manifest-path "$root/Cargo.toml")
[ "$profile" = "release" ] && args+=(--release)
out="$root/target/$profile"
if [ "$triple" != "$host" ]; then
  args+=(--target "$triple")
  out="$root/target/$triple/$profile"
fi
cargo build "${args[@]}"

mkdir -p "$here/binaries"
cp "$out/orso" "$here/binaries/orso-$triple"
# tauri-build copies the sidecar next to the app only when its build script reruns, which a
# changed daemon alone doesn't trigger; refresh that copy too so the app never runs a stale one.
if [ -d "$here/target/$profile" ]; then
  cp "$here/binaries/orso-$triple" "$here/target/$profile/orso"
fi
echo "sidecar: binaries/orso-$triple ($profile)"
