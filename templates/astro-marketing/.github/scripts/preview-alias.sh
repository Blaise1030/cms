#!/usr/bin/env bash
# Usage: preview-alias.sh <git branch> <worker name>
# Prints the Workers preview alias for a branch, so each branch keeps one stable preview URL:
# https://<alias>-<worker name>.<account subdomain>.workers.dev
#
# Same rules as wrangler's own alias generation: every run of characters outside [A-Za-z0-9-]
# becomes one "-", lowercased, trimmed of "-"; it must start with a letter, and alias + "-" +
# worker name must fit in one 63-character DNS label (long names are cut and get a short hash
# of the branch so they stay distinct). A branch name is attacker-controlled and wrangler-action
# runs its `command` through a shell, so only this output (only [a-z0-9-]) may reach it.
set -euo pipefail
branch="${1:-}"
worker="${2:?worker name required}"
alias="$(printf '%s' "$branch" | LC_ALL=C tr -c 'A-Za-z0-9-' '-' | tr -s '-' | sed 's/^-*//; s/-*$//' | LC_ALL=C tr 'A-Z' 'a-z')"
case "$alias" in
  "") alias="preview" ;;
  [a-z]*) ;;
  *) alias="b-$alias" ;;
esac
space=$((63 - ${#worker} - 1))
if [ "${#alias}" -gt "$space" ]; then
  hash="$(printf '%s' "$branch" | shasum -a 256 | cut -c1-4)"
  keep=$((space - 5))
  [ "$keep" -ge 1 ] || { echo "worker name too long for preview aliases: $worker" >&2; exit 1; }
  alias="$(printf '%s' "${alias:0:$keep}" | sed 's/-*$//')-$hash"
fi
printf '%s\n' "$alias"
