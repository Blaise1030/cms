#!/bin/sh
# Stand-in headless agent for auto-resolve tests: `fake_resolver.sh <mode> … <prompt>`.
mode=$1
for prompt; do :; done
out=$ORSO_RESOLVE_RESULT

resolved() {
  printf '%s' '{"status":"resolved","summary":"Kept your version"}' > "$out"
}

keep_ours() {
  git checkout --ours -- $(git diff --name-only --diff-filter=U)
}

case "$mode" in
  resolved)
    keep_ours
    resolved
    ;;
  needs-input)
    case "$prompt" in
      *"Answers from the writer:"*)
        answer=$(printf '%s\n' "$prompt" | sed -n 's/^- Which headline should stay?: //p')
        for file in $(git diff --name-only --diff-filter=U); do
          printf '%s\n' "$answer" > "$file"
        done
        resolved
        ;;
      *)
        printf '%s' '{"status":"needs_input","questions":[{"id":"q1","text":"Which headline should stay?","options":["Mine","Theirs"]}]}' > "$out"
        ;;
    esac
    ;;
  garbage)
    printf 'not json' > "$out"
    ;;
  commit)
    keep_ours
    git commit -qam x
    resolved
    ;;
  sleep)
    sleep 30
    ;;
esac
