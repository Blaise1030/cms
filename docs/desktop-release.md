# Desktop app

The Tauri macOS app is back in `src-tauri/`: a window that starts the `orso` daemon as a
sidecar, loads its web UI, and stops it on quit.

- Run it: `just desktop-dev`. Bundle `orso.app` and a DMG: `just desktop-build` (builds the
  release daemon as the sidecar first).
- CI builds the app on macOS (the *Desktop (macOS)* job in `.github/workflows/ci.yml`) and
  checks `src-tauri` with `cargo fmt`, `clippy -D warnings` and its tests.
- Release artifacts are still daemon tarballs only (`.github/workflows/release.yml`); signing,
  notarization and an in-app updater are not set up.

The desktop app takes review screenshots with a native snapshot of its own webview
(`capture_region`), which needs no Screen Recording permission.
