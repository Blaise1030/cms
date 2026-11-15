# orso web-react (Lenso spike)

Parallel React + Lenso shell. Does **not** replace the embedded Vue app in `web/`.

## Run

```sh
just dev-lenso
```

Open the printed URL (Vite on `:5174` with `?t=`). Vue remains on `:5173` via `just dev`.

## Spike scope

- Daemon auth (`?t=`), REST health/projects, WS events
- Lenso `NavigationBar` + project list
- No content/editor/chat/settings

See `docs/plans/lenso-spike-findings.md` after a go/no-go review.
