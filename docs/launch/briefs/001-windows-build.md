# Brief 001 — Windows support

- **Owner:** Product Engineer
- **Opened:** 14 Nov 2026
- **Status:** draft — needs approval
- **Issue:** #24 (Phase 5: packaging and writer mode)
- **Blocks:** launch gate #1

## Goal

A developer on Windows 11 can install ORSO, open a project, edit content, preview it, and publish
to GitHub Pages, without touching a terminal.

## Done when

- [ ] `orso.exe` builds and runs on Windows 11 from a clean machine
- [ ] Installer produced (MSI or NSIS), signed or explicitly documented as unsigned for beta
- [ ] The daemon binds loopback correctly and the app opens paired, with no token copy-paste
- [ ] Startup checks for Node and git work, with a friendly message when either is missing
- [ ] Publish to GitHub Pages verified end to end from Windows
- [ ] `just test` and `just e2e` pass on a Windows CI runner, or a documented list of exclusions
- [ ] A short "install on Windows" section in the docs with real screenshots from a real machine
- [ ] Recorded demo: install → publish, unedited, under four minutes

## Inputs

- `README.md`, `docs/superpowers/plans/`, `docs/superpowers/specs/2026-10-15-tauri-desktop-design.md`
- Issue #24
- `docs/desktop-release.md` — read for context, do not modify

## Guardrails

- Do not change macOS behaviour to make Windows easier. Both must work.
- Do not touch release workflows, signing keys or the updater without explicit approval.
- Do not add a dependency to work around a platform path issue without a decision file.
- Do not silently drop a feature on Windows. If something cannot ship, it goes in the brief's notes
  and the launch gate, not in a quiet `#[cfg(not(windows))]`.

## Output

A branch and a series of small PRs against #24. The first PR should be the smallest thing that
makes CI build for Windows at all — not a full port.

## Notes

Nothing yet.
