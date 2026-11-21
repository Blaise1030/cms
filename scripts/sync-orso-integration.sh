#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

src="packages/astro/src"

# `integrations/orso/` is generated: replace it whole so files removed from the package (such as
# the old `marketing.js` models) do not linger. Content models live in each site's own
# `src/content/collections.js`, never here.
for dest in templates/astro-marketing/integrations/orso site/integrations/orso; do
  rm -rf "$dest"
  mkdir -p "$dest"
  # Readable integration sources only — do not vendor minified panel UI into user projects.
  while IFS= read -r -d '' file; do
    rel="${file#$src/}"
    mkdir -p "$dest/$(dirname "$rel")"
    cp "$file" "$dest/$rel"
  done < <(find "$src" -type f -name '*.js' ! -path '*/panel/*' -print0)
done
