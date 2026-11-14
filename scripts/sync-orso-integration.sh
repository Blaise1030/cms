#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

dest="templates/astro-marketing/integrations/orso"
src="packages/astro/src"

mkdir -p "$dest"
# Readable integration sources only — do not vendor minified panel UI into user projects.
while IFS= read -r -d '' file; do
  rel="${file#$src/}"
  mkdir -p "$dest/$(dirname "$rel")"
  cp "$file" "$dest/$rel"
done < <(find "$src" -type f -name '*.js' ! -path '*/panel/*' -print0)
rm -rf "$dest/panel"
