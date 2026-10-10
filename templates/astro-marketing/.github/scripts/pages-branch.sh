#!/usr/bin/env bash
# Usage: pages-branch.sh <git branch>
# Prints the Cloudflare Pages branch to deploy to: the git branch with every character outside
# [A-Za-z0-9._/-] replaced by "-". A branch name is attacker-controlled (anyone who can push a
# branch picks it) and wrangler-action runs its `command` through a shell, so only this
# sanitised form may reach that command.
set -euo pipefail
safe="$(printf '%s' "${1:-}" | LC_ALL=C tr -c 'A-Za-z0-9._/-' '-')"
printf '%s\n' "${safe:-preview}"
