# Desktop app

The Tauri macOS app is back in `src-tauri/`: a window that starts the `orso` daemon as a
sidecar, loads its web UI, and stops it on quit.

- Run it: `just desktop-dev`. Bundle `orso.app` and a DMG: `just desktop-build` (builds the
  release daemon as the sidecar first).
- CI builds the app on macOS (the *Desktop (macOS)* job in `.github/workflows/ci.yml`) and
  checks `src-tauri` with `cargo fmt`, `clippy -D warnings` and its tests.
- Each release attaches `orso-vX.Y.Z-<target>.dmg` for Apple Silicon and Intel
  (`.github/workflows/desktop-release.yml`, called by `release.yml`; run it by hand to backfill
  a release). The app is ad-hoc signed only: notarization and an in-app updater are not set up,
  so macOS asks users to allow it once (System Settings → Privacy & Security → Open Anyway).

The desktop app takes review screenshots with a native snapshot of its own webview
(`capture_region`), which needs no Screen Recording permission.
