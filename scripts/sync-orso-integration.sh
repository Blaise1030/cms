#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

dest="templates/astro-marketing/integrations/orso"
src="packages/astro/src"

mkdir -p "$dest"
for name in index.js manifest.js patch.js endpoint.js bridge.js; do
  cp "$src/$name" "$dest/$name"
done
