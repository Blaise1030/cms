# Orso bear artwork

The active `orso-logo.svg`, `orso-mark.svg`, and desktop copy were replaced
with the user-supplied `bear_chrome_rounded_exact.svg` on 2026-10-20. Its embedded
1940×1420 PNG is downscaled to 256 px wide (4× the 64 px display size) and its C2PA
manifest dropped, keeping the files under the PWA precache budget.
The source and license notes below describe the earlier artwork only, not the
replacement SVG.

## Previous artwork

The Orso bear is adapted from **Simple Black Bear** by **qubodup**.

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
The desktop splash bundles a copy of `../orso-logo.svg` in
`src-tauri/ui/orso-logo.svg`.

This shared artwork is not exclusive to Orso. Modifying it does not establish
trademark clearance.
