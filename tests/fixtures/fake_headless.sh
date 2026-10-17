#!/bin/sh
# Stand-in headless coding CLI for chat tests: `fake_headless.sh <fixture> [mode] …ignored flags`
# prints a JSON-lines fixture on stdout.
for arg in "$@"; do
  if [ "$arg" = create-chat ]; then
    echo chat-fake-1
    exit 0
  fi
done
fixture=$1
case "$2" in
  edit)
    printf 'hello\n' >> notes.md
    cat "$fixture"
    ;;
  slow)
    head -n 1 "$fixture"
    sleep 1
    tail -n +2 "$fixture"
    # Ignores SIGTERM, so only the SIGKILL that follows ends it.
    (trap '' HUP TERM; exec sleep 300) &
    if [ -n "$FAKE_PIDFILE" ]; then
      echo $! > "$FAKE_PIDFILE"
    fi
    sleep 300
    ;;
  *) cat "$fixture" ;;
esac
exit "${FAKE_EXIT:-0}"
