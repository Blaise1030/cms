---
title: Install orso
description: Install orso on macOS or Linux and open it in your browser.
category: getting-started
order: 2
---

## macOS: Homebrew (recommended)

```sh
brew install blaise1030/tap/orso
orso serve --open
```

## macOS or Linux: installer script

The script downloads the right release for your computer and checks its sha256 before installing it.

```sh
curl -fsSL https://raw.githubusercontent.com/Blaise1030/cms/main/scripts/install.sh | sh
orso serve --open
```

- To install a specific version, set `ORSO_VERSION`, for example `ORSO_VERSION=0.1.5`.
- To install somewhere else, set `ORSO_INSTALL_DIR`.

On a Mac, Homebrew is the better choice. A binary downloaded another way can be flagged by Gatekeeper. The installer clears that flag when it can.

## Open orso

`orso serve --open` opens orso in your browser at an address like `http://orso.localhost:<port>/?t=…`.

- orso only listens on your own computer (`127.0.0.1`).
- The `?t=` token in the link is what lets your browser in. If you lose the tab, run `orso serve --open` again to get a fresh link.
- Keep the terminal open while you work. Press Ctrl-C to stop orso.

## Check your version

```sh
orso --version
```

Include this number when you report a problem.

## Next step

**Sign in with GitHub** so orso can open your repos and create pull requests.
