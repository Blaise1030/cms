// Regenerates the PWA icons from the bear in ../public/icons/favicon.svg.
// Run: node scripts/gen-icons.mjs   (needs `rsvg-convert`, e.g. `brew install librsvg`)
// Writes icon-192/512, maskable-512 and apple-touch-icon PNGs plus the orso-logo/orso-mark SVGs:
// a light bear inside the middle 62.5% of a #0f0f12 square, so Android's mask keeps it whole.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const icons = path.resolve(import.meta.dirname, "../public/icons");
const bear = /\sd="([^"]+)"/.exec(readFileSync(path.join(icons, "favicon.svg"), "utf8"))?.[1];
if (!bear) throw new Error("favicon.svg has no bear path");

const square = (rx) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <rect width="512" height="512"${rx ? ` rx="${rx}"` : ""} fill="#0f0f12"/>
  <svg x="96" y="96" width="320" height="320" viewBox="3.6 -2.9 41 41"><path fill="#f5f5f5" d="${bear}"/></svg>
</svg>
`;

// Rounded for places that show the SVG as-is; full-bleed for PNGs the OS masks itself.
for (const name of ["orso-logo.svg", "orso-mark.svg"]) writeFileSync(path.join(icons, name), square(113));

const tmp = mkdtempSync(path.join(tmpdir(), "orso-icons-"));
const source = path.join(tmp, "icon.svg");
writeFileSync(source, square(0));
for (const [name, size] of [["icon-192.png", 192], ["icon-512.png", 512], ["maskable-512.png", 512], ["apple-touch-icon.png", 180]]) {
  execFileSync("rsvg-convert", ["-w", String(size), "-h", String(size), source, "-o", path.join(icons, name)]);
}
rmSync(tmp, { recursive: true });
console.log("icons written to", icons);
