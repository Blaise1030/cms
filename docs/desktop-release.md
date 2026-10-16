# Desktop (macOS) release

The Workbench desktop app (`src-tauri/`) ships in the same GitHub release as the
daemon tarballs. The `desktop` job in `.github/workflows/release.yml` builds it for
`aarch64-apple-darwin` and `x86_64-apple-darwin`, signs it with the Developer ID
certificate, notarizes and staples it, and uploads:

| File | Purpose |
| --- | --- |
| `Workbench-<version>-<arch>.dmg` | What users download and install. |
| `Workbench-<version>-<arch>.app.tar.gz` | The bundle the in-app updater installs. |
| `Workbench-<version>-<arch>.app.tar.gz.sig` | Its updater signature. |
| `latest.json` | Written by `publish` (`.github/scripts/desktop-latest-json.sh`); the updater reads `releases/latest/download/latest.json`. |

`<arch>` is `aarch64` or `x86_64`. `publish` waits for `desktop`, so a release never
goes out without a signed app. The job's first step fails if any secret below is
empty: unsigned or un-notarized builds are never shipped.

## Local desktop development

For the title-bar-less shell with web HMR (separate from `just dev`, which is browser-only):

```sh
just desktop-dev          # Vite :5180, daemon :7810 (defaults clear of just dev)
# or: just desktop-dev 5173 7800
```

That runs `src-tauri/scripts/desktop-dev.sh`: builds a debug daemon, starts Vite, then
`tauri dev` with identifier `com.workbench.desktop.dev` so an open Workbench.app is left
alone. The window shows Vite; `/api`, `/ws` and `/preview` proxy to the daemon. Shell
(`src-tauri/`) edits rebuild and relaunch; daemon (`src/`) edits need a restart.
Ctrl-C stops everything. Release builds ignore the `WORKBENCH_DEV_*` env vars.

## Required GitHub secrets

Set them under **Settings → Secrets and variables → Actions** of `Blaise1030/cms`
(`gh secret set NAME`). Notarization uses the Apple ID + app-specific password
variant of [Tauri's macOS signing guide](https://v2.tauri.app/distribute/sign/macos/).

| Secret | What it is | How to get it |
| --- | --- | --- |
| `APPLE_CERTIFICATE` | Base64 of the **Developer ID Application** certificate + private key, exported as `.p12`. | Apple Developer → Certificates → create *Developer ID Application* (needs the Account Holder role). Install it, export cert + key from Keychain Access as `.p12` with a password, then `base64 -i cert.p12 \| pbcopy`. |
| `APPLE_CERTIFICATE_PASSWORD` | The `.p12` export password. | Chosen at export. |
| `APPLE_SIGNING_IDENTITY` | The identity name, e.g. `Developer ID Application: Your Name (TEAMID)`. | `security find-identity -v -p codesigning`. |
| `APPLE_ID` | The Apple ID email used for notarization. | Your developer account. |
| `APPLE_PASSWORD` | An **app-specific password** for that Apple ID (not the account password). | [account.apple.com](https://account.apple.com) → Sign-In and Security → App-Specific Passwords. |
| `APPLE_TEAM_ID` | The 10-character team id. | Apple Developer → Membership details. |
| `TAURI_SIGNING_PRIVATE_KEY` | The updater signing private key (file contents). | `~/.tauri/workbench-updater.key` on the maintainer's Mac: `gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/workbench-updater.key`. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | That key's password. | `gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD < ~/.tauri/workbench-updater.key.password`. |

### The updater key

- Generated with `tauri signer generate`. The private key is
  `~/.tauri/workbench-updater.key` and its password `~/.tauri/workbench-updater.key.password`
  (directory `0700`, files `0600`). Neither is ever committed. Back both up somewhere safe
  (a password manager); the GitHub secrets can't be read back.
- Only the public key is committed, as `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`.
  Installed apps accept an update only if its `.sig` verifies against that key.
- **If the private key or its password is lost, existing installs can never update again.**
  A new keypair needs a new `pubkey`, which only apps built after the change carry; everyone on an
  older build must download the new `.dmg` by hand. The same applies if the key leaks: rotate it
  (generate a new pair, update `pubkey`, replace both secrets) and announce a manual reinstall.

### Renewing Apple credentials

- **Developer ID certificate:** valid for 5 years. Before it expires, create a new *Developer ID
  Application* certificate, export a new `.p12` and replace `APPLE_CERTIFICATE`,
  `APPLE_CERTIFICATE_PASSWORD` and (if the name changed) `APPLE_SIGNING_IDENTITY`. Builds already
  signed and notarized keep working after expiry; only new builds need the new certificate.
- **App-specific password:** stops working if revoked or the Apple ID password changes; create a
  new one and replace `APPLE_PASSWORD`.
- **Apple Developer Program membership** must stay active, or notarization fails.

## Cutting a desktop release

Nothing desktop-specific: merging the release PR (which bumps the root `package.json` version)
runs `release.yml`, and `desktop` builds alongside `build` and `e2e`. The app's version comes
from the root `package.json` (`tauri.conf.json` has `"version": "../package.json"`), so the
app, `latest.json` and the tag always agree. After `publish`, check the release has both `.dmg`
files, both `.app.tar.gz` + `.sig` pairs and `latest.json`.

Installed apps check `latest.json` on launch and every 6 hours, download a newer version in the
background, and show **Update ready — restart to apply** once no terminal session is busy. Nothing
is applied until the user clicks **Restart to update** on the confirmation page; the app then
re-checks that no session is busy, stops the daemon gracefully, installs and relaunches. Updater
errors only go to the shell log (included in **Collect logs**).

## Building locally

```sh
pnpm -C web install --frozen-lockfile && pnpm -C web build   # the daemon embeds web/dist
src-tauri/scripts/stage-sidecar.sh                           # host triple; or pass one
pnpm --prefix src-tauri install --frozen-lockfile
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/workbench-updater.key)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.tauri/workbench-updater.key.password)" \
  pnpm --prefix src-tauri tauri build --bundles app
```

Without the two `TAURI_SIGNING_*` variables the build fails, because `createUpdaterArtifacts`
is on. Output lands in `src-tauri/target/release/bundle/macos/` (`Workbench.app`,
`Workbench.app.tar.gz`, `Workbench.app.tar.gz.sig`). Local builds are unsigned unless the
`APPLE_*` variables are set too; never distribute an unsigned build.

## Local updater end-to-end test

`src-tauri/scripts/update-e2e.sh` exercises the whole update path on one Mac without
publishing anything. It builds 0.0.1 (N) and 0.0.2 (N+1) under the test bundle id
`com.workbench.desktop.updatetest`, so it can run next to a real Workbench. The builds go to
`/tmp/wb-update-e2e` (~2 GB) with the non-default `update-e2e` cargo feature and a loopback
update endpoint. The script installs N in `/tmp/wb-update-e2e/Applications` and serves N+1
from a local `latest.json`. It then checks that N, while a terminal session is busy,
downloads and verifies the update but doesn't apply it; that once the session goes idle N
stops its daemon (exit 0, PTY children reaped), installs N+1 and relaunches; and that N+1's
daemon comes up healthy.

```sh
pnpm -C web build && src-tauri/scripts/stage-sidecar.sh   # once
src-tauri/scripts/update-e2e.sh                            # PASS/FAIL lines; exit 1 on any FAIL
UPDATE_E2E_SKIP_BUILD=1 src-tauri/scripts/update-e2e.sh    # rerun without rebuilding
rm -rf /tmp/wb-update-e2e                                  # afterwards
```

It needs the updater key files in `~/.tauri` (it signs N+1 like CI does). The `update-e2e`
feature only adds test hooks (temp `HOME`, a log file, auto-applying through the same path as
**Restart to update**). Release builds never enable it, and a unit test checks `release.yml`
doesn't either. This doesn't replace the signed, notarized check below.

## Pending: first end-to-end update test

Not yet done; it needs the Apple secrets and two real releases. Use a test version pair (N, N+1):

1. [ ] Set all eight secrets above.
2. [ ] Release N. Confirm the `desktop` jobs pass, including **Verify signature and notarization**,
       and the release has `Workbench-N-aarch64.dmg`, `Workbench-N-x86_64.dmg`, both
       `.app.tar.gz` + `.sig` pairs and `latest.json` (its `version` is N and both `url`s download).
3. [ ] On a clean macOS user account, download the `.dmg` for that Mac, drag Workbench to
       Applications and open it. Gatekeeper must not warn about an unidentified developer.
4. [ ] Create or open a project; start a terminal session; confirm the preview and terminal work.
5. [ ] Release N+1. Within 6 h, or immediately after relaunching N, the app downloads it.
6. [ ] While an agent turn is running, confirm the notice does **not** appear (shell log:
       "waiting for terminal sessions to go idle"). When the turn ends, **Update ready — restart to
       apply** appears within ~15 s.
7. [ ] Click **Restart to update**, then **Not now**: you're back in the app, nothing changed.
8. [ ] Start a turn, open the update page again and click **Restart to update**: it refuses with the
       busy message and **Not now** goes back.
9. [ ] Idle again: **Restart to update** → the app quits, relaunches as N+1 (About / `Info.plist`),
       and no `workbench serve` process from N is left (`pgrep -fl 'Workbench.app'`).
10. [ ] Projects and terminals still work after the update; run `just test` and the e2e suite on the
        release commit.
