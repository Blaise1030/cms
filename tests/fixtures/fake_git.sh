#!/bin/sh
# Stand-in real git for the git-shim tests: records its argv and echoes it back.
if [ -n "${FAKE_GIT_LOG:-}" ]; then
  printf '%s\n' "$@" >> "$FAKE_GIT_LOG"
fi
printf '%s\n' "$@"
exit "${FAKE_GIT_EXIT:-0}"
