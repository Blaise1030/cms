# Lenso spike findings

Status: **scaffold complete** — ready for interactive go/no-go (`just dev-lenso`).

## Verified in CI-less checks

- [x] `pnpm -C web-react test` — 8 config tests pass
- [x] `pnpm -C web-react build` — StyleX + Lenso production build succeeds (~540 kB JS gzip 176 kB; glass worker split)
- [x] Vite serves StyleX virtual CSS on `:5174`
- [x] Dev StyleX: exclude `@lenso/*` from `optimizeDeps` + `externalPackages` (fixes `defineConsts` runtime error)
- [ ] Live daemon: token → `open` → project list (run `just dev-lenso`)
- [ ] Visual: nav glass / theme look acceptable for a CMS shell
- [ ] No need to fork `@lenso/glass`

**Decision:** _pending interactive review_

## Notes

- Vue `web/` remains the embedded production SPA (`src/static.rs` unchanged).
- Spike package: [`web-react/`](../../web-react/) — React 19, `@lenso/ui@0.11.1`, StyleX unplugin.
- Dev entry: `just dev-lenso` (daemon `--ui-url http://localhost:5174`).
- Scope: Shell + Home project list only; no content/editor/chat/settings.
