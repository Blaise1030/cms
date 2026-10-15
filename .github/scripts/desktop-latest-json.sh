#!/usr/bin/env bash
# Prints the Tauri updater manifest (latest.json) for a desktop release.
# usage: desktop-latest-json.sh <version> <dir holding Workbench-<version>-<arch>.app.tar.gz[.sig]> <download base URL>
set -euo pipefail

version=$1
dir=$2
base=$3

platform() {
  local file="Workbench-$version-$1.app.tar.gz"
  jq -n --rawfile sig "$dir/$file.sig" --arg url "$base/$file" '{signature: ($sig | rtrimstr("\n")), url: $url}'
}

jq -n \
  --arg version "$version" \
  --arg date "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --argjson arm "$(platform aarch64)" \
  --argjson x64 "$(platform x86_64)" \
  '{version: $version, notes: "Workbench v\($version)", pub_date: $date,
    platforms: {"darwin-aarch64": $arm, "darwin-x86_64": $x64}}'
