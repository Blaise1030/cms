# Orso mark

The app mark is Lucide's [`paw-print`](https://lucide.dev/icons/paw-print), drawn on its
24-unit grid with the default 2-unit stroke. It replaced the Orso bear on 2026-10-21.

- `../orso-logo.svg` and `../orso-mark.svg` — the paw on a 512 px rounded square
  (`#0f0f12`), served as the browser favicon.
- `../*.png` and `../../../../src-tauri/icons/*` — rasters of the same artwork, generated
  by `web/scripts/gen-icons.mjs` (`node web/scripts/gen-icons.mjs`; it also assembles
  `icon.icns` with macOS's `iconutil`).
- `../../../../src-tauri/ui/splash.html` — inlines the icon so it inherits the page's text
  colour instead of carrying a baked-in one.

Lucide is ISC licensed: https://github.com/lucide-icons/lucide/blob/main/LICENSE

## Previous artwork

The Orso bear was replaced with the user-supplied `bear_chrome_rounded_exact.svg` on
2026-10-20; its embedded 1940×1420 PNG was downscaled to 256 px wide (4× the 64 px display
size) and its C2PA manifest dropped, keeping the files under the PWA precache budget. The
bear is no longer rendered anywhere.

The bear itself was adapted from **Simple Black Bear** by **qubodup**.

- Source: https://openclipart.org/detail/202456/simple-black-bear-by-qubodup-202456
- Original SVG: https://openclipart.org/download/202456/simplebear.svg
- License: CC0 / public domain, per https://openclipart.org/share
- License text: https://creativecommons.org/publicdomain/zero/1.0/
- Original preserved as `simple-black-bear.svg` in this directory.

Changes: replaced polygon corners with quadratic curves (up to 5 source units
along each adjacent edge, capped at 30% of its length), added transparent
padding, and normalized SVG metadata. A silver gradient, edge highlights,
lower bevel, shallow extrusion, and soft shadow give the bear a raised metal finish.
Subtle isotropic grain, warm silver tones, and broad edge lighting add a cast-silver surface.

This shared artwork is not exclusive to Orso. Modifying it does not establish
trademark clearance.
