# Brief 001 — Windows support (CLI)

- **Owner:** Product Engineer
- **Opened:** 14 Nov 2026
- **Updated:** 14 Nov 2026 — retargeted from desktop app to CLI zip after Tauri retirement
- **Status:** draft — needs approval
- **Issue:** #24 (Phase 5: packaging and writer mode)
- **Blocks:** launch gate #1

## Goal

A developer on Windows 11 can install the **orso CLI**, open a project in the browser UI,
edit content, preview it, and publish to GitHub Pages.

## Done when

- [ ] `orso.exe` builds and runs on Windows 11 from a clean machine
- [ ] Release matrix ships a Windows zip (or equivalent) alongside macOS/Linux tarballs
- [ ] Install docs: download → put on PATH → `orso serve --open` (no token copy-paste)
- [ ] Startup checks for Node and git work, with a friendly message when either is missing
- [ ] Publish to GitHub Pages verified end to end from Windows
- [ ] `just test` and `just e2e` pass on a Windows CI runner, or a documented list of exclusions
- [ ] A short "install on Windows" section in the docs with real screenshots from a real machine
- [ ] Recorded demo: install → publish, unedited, under four minutes

## Inputs

- `README.md`, `scripts/install.sh`, `.github/workflows/release.yml`
- Issue #24
- `docs/desktop-release.md` — retired desktop path; CLI-only now

## Guardrails

- Do not revive the Tauri / notarized desktop app to make Windows easier.
- Do not change macOS/Linux CLI behaviour without need. All three CLIs must work.
- Do not touch Apple signing keys or re-add a desktop CI job without explicit approval.
- Do not add a dependency to work around a platform path issue without a decision file.
- Do not silently drop a feature on Windows. If something cannot ship, it goes in the brief's notes
  and the launch gate, not in a quiet `#[cfg(not(windows))]`.

## Output

Working Windows CLI artifact in GitHub Releases, install docs, and launch-gate evidence for gate #1.
