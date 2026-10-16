#!/usr/bin/env bash
# Local updater end-to-end test: builds Workbench 0.0.1 (N) and 0.0.2 (N+1) under a
# test bundle id, installs N in a temp Applications dir, serves N+1 from a local
# latest.json, and checks N defers while a terminal is busy, then installs N+1 and
# relaunches. See docs/desktop-release.md. Nothing is published.
#
# Needs: the sidecar staged (stage-sidecar.sh), ~/.tauri/workbench-updater.key{,.password}.
# Env: UPDATE_E2E_DIR (default /tmp/wb-update-e2e), UPDATE_E2E_SKIP_BUILD=1 to reuse builds.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
src_tauri="$(cd "$here/.." && pwd)"
mkdir -p "${UPDATE_E2E_DIR:-/tmp/wb-update-e2e}"
# Resolved: Tauri refuses to locate the sidecar when the app's path has a symlink
# (/tmp is one on macOS).
work="$(cd "${UPDATE_E2E_DIR:-/tmp/wb-update-e2e}" && pwd -P)"
triple="$(rustc -vV | sed -n 's/^host: //p')"
bundle="$work/target/release/bundle/macos"

[ -x "$src_tauri/binaries/workbench-$triple" ] || {
  echo "missing src-tauri/binaries/workbench-$triple: run stage-sidecar.sh first" >&2
  exit 1
}

if [ -z "${UPDATE_E2E_SKIP_BUILD:-}" ] || [ ! -f "$work/port" ]; then
  python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])' > "$work/port"
fi
port="$(cat "$work/port")"

# Test identity and a loopback update endpoint; tauri.conf.json is untouched.
config() {
  printf '{"version":"%s","identifier":"com.workbench.desktop.updatetest","plugins":{"updater":{"endpoints":["http://127.0.0.1:%s/latest.json"],"dangerousInsecureTransportProtocol":true}}}' "$1" "$port"
}

build() {
  CARGO_TARGET_DIR="$work/target" CARGO_INCREMENTAL=0 \
    TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/workbench-updater.key)" \
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.tauri/workbench-updater.key.password)" \
    pnpm --prefix "$src_tauri" tauri build --bundles app --features update-e2e --config "$(config "$1")"
}

if [ -z "${UPDATE_E2E_SKIP_BUILD:-}" ]; then
  rm -rf "$work/n" "$work/serve"
  mkdir -p "$work/n" "$work/serve"
  build 0.0.1
  ditto "$bundle/Workbench.app" "$work/n/Workbench.app"
  build 0.0.2
  cp "$bundle/Workbench.app.tar.gz" "$work/serve/Workbench-0.0.2.app.tar.gz"
  cp "$bundle/Workbench.app.tar.gz.sig" "$work/serve/Workbench-0.0.2.app.tar.gz.sig"
fi

# A fresh install of N every run (the test replaces it with N+1).
rm -rf "$work/Applications"
mkdir -p "$work/Applications"
ditto "$work/n/Workbench.app" "$work/Applications/Workbench.app"

exec python3 "$here/update_e2e.py" --work "$work" --port "$port"
