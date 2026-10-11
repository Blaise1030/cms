#!/bin/sh
# Stand-in coding CLI for terminal tests: a line-driven REPL on the PTY.
echo "FAKE-CLI READY"
while IFS= read -r line; do
  case "$line" in
    exit-test)
      echo "FAKE-CLI EXIT REASON"
      exit 1
      ;;
    size) stty size ;;
    argv) for a in "$@"; do printf '%s\n' "$a"; done ;;
    spawn-child)
      # Ignores SIGHUP, so only a signal to the whole process group ends it (the kernel
      # hangs up the foreground group when the session leader exits).
      (trap '' HUP; exec sleep 300) &
      echo "CHILD $!"
      ;;
    "edit "*)
      text=${line#edit }
      printf '%s\n' "$text" >> notes.md
      "$WORKBENCH_HOOK_EXE" hook turn-end "{\"summary\":\"$text\"}"
      echo "TURN-END"
      ;;
    *) echo "ECHO $line" ;;
  esac
done
