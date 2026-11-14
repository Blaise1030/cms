# Desktop release — retired

The Tauri macOS app (DMG, notarization, in-app updater) is **no longer shipped**.

orso is CLI-first:

- Install with Homebrew (`brew install blaise1030/tap/orso`) or
  `curl -fsSL https://raw.githubusercontent.com/Blaise1030/cms/main/scripts/install.sh | sh`
- Run `orso serve --open`

Release artifacts are daemon tarballs only (see `.github/workflows/release.yml`).
The `src-tauri/` tree remains in the repo as unsupported reference code; see
[`src-tauri/UNSUPPORTED.md`](../src-tauri/UNSUPPORTED.md).

Historical signing / notarization / updater notes lived here before this retirement.
Do not re-enable the desktop CI job without a new product decision.
