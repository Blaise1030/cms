#!/usr/bin/env bash
# The desktop shell in dev mode (`just desktop-dev`): `tauri dev` rebuilds and relaunches the
# shell on src-tauri changes; its window shows Vite (HMR) with the token of a debug daemon the
# shell starts on the port Vite proxies to. Ctrl-C stops the shell, its daemon and Vite.
# Usage: desktop-dev.sh [vite-port] [daemon-port]   (defaults clear of `just dev`'s 5173/7800)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"
vite_port="${1:-5180}"
daemon_port="${2:-7810}"

cargo build
triple="$(rustc -vV | sed -n 's/^host: //p')"
# `tauri dev` needs the bundled sidecar file to exist even though dev mode doesn't run it.
[[ -f "src-tauri/binaries/orso-$triple" ]] || src-tauri/scripts/stage-sidecar.sh "$triple"

export ORSO_VITE_PORT="$vite_port" ORSO_DAEMON_PORT="$daemon_port"
export ORSO_DEV_UI="http://localhost:$vite_port"
export ORSO_DEV_DAEMON="${ORSO_DEV_DAEMON:-$repo_root/target/debug/orso}"

trap 'trap - INT TERM EXIT; kill 0' INT TERM EXIT
pnpm -C web dev &
vite=$!
# Without this the window could open on whatever else holds the port.
until nc -z localhost "$vite_port" 2>/dev/null; do
  kill -0 "$vite" 2>/dev/null || { echo "Vite did not start on :$vite_port" >&2; exit 1; }
  sleep 0.5
done

# Its own identifier, so an open orso.app doesn't take it for a second instance.
pnpm --prefix src-tauri tauri dev --no-dev-server \
  --config '{"identifier":"com.orso.desktop.dev","productName":"orso Dev"}' &
wait
