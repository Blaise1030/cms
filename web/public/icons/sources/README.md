# Orso mark

Since 2026-11-15 every orso icon is the bear from the marketing site
(content-landing-page-revamp-astro-marketing): a smoothed **Simple Black Bear** by qubodup
(CC0, see below).

- `../favicon.svg` — the bear alone, the browser favicon. Near-black on light browser
  chrome, light in dark mode (`prefers-color-scheme`). The source of every other icon.
- `../orso-logo.svg`, `../orso-mark.svg` — the light bear on a 512 px rounded `#0f0f12` square.
- `../*.png` — PWA and Apple touch icons: the same bear on a full-bleed square, inside the
  middle 62.5% so Android's mask keeps it whole.
- `web/src/app/BearMark.vue` — the in-app logo, the same path in `currentColor`.

Regenerate the SVGs and PNGs after changing `favicon.svg`:
`node web/scripts/gen-icons.mjs` (needs `rsvg-convert`, `brew install librsvg`).

## Previous artwork

The paw (Lucide's `paw-print`, ISC, in cast silver) was the mark from 2026-10-21 to
2026-11-15. Before it, a chrome-finished version of the bear below.


The Orso bear was replaced with the user-supplied `bear_chrome_rounded_exact.svg` on
2026-10-20; its embedded 1940×1420 PNG was downscaled to 256 px wide (4× the 64 px display
size) and its C2PA manifest dropped, keeping the files under the PWA precache budget.

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
