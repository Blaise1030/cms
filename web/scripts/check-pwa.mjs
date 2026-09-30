// Post-build PWA checks: app shell precached, manifest/icons valid, no live-data caching.
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const dist = path.resolve(import.meta.dirname, "../dist");
const failures = [];
const check = (ok, msg) => ok || failures.push(msg);
const read = (f) => readFileSync(path.join(dist, f), "utf8");

for (const f of ["index.html", "sw.js", "manifest.webmanifest"]) {
  check(existsSync(path.join(dist, f)), `dist/${f} missing`);
}

if (failures.length === 0) {
  const sw = read("sw.js");
  const precached = [...sw.matchAll(/url:\s*"([^"]+)"/g)].map((m) => m[1]);
  check(precached.includes("index.html"), "index.html is not precached");
  check(
    precached.some((u) => u.startsWith("assets/") && u.endsWith(".js")),
    "no JS bundle precached",
  );
  check(
    precached.some((u) => u.startsWith("assets/") && u.endsWith(".css")),
    "no CSS bundle precached",
  );
  check(precached.includes("manifest.webmanifest"), "manifest is not precached");
  for (const u of precached) {
    check(!/^(api|ws|preview)(\/|$)/.test(u), `daemon path precached: ${u}`);
  }

  // Only the precache route + the navigation fallback may be registered: no runtime caching.
  check(!/\bNetworkFirst\b|\bStaleWhileRevalidate\b|\bCacheFirst\b/.test(sw), "runtime caching strategy found in sw.js");
  check(/createHandlerBoundToURL\(\s*"\/index\.html"\s*\)/.test(sw), "navigation fallback missing");
  for (const p of ["api", "ws", "preview"]) {
    check(sw.includes(`/^\\/${p}(\\/|$)/`), `navigation denylist missing /${p}`);
  }

  const manifest = JSON.parse(read("manifest.webmanifest"));
  check(manifest.display === "standalone", "manifest display must be standalone");
  check(Boolean(manifest.theme_color), "manifest theme_color missing");
  const icons = manifest.icons ?? [];
  check(icons.some((i) => i.purpose === "maskable"), "no maskable icon");
  check(icons.some((i) => i.sizes === "512x512"), "no 512x512 icon");
  for (const i of icons) check(existsSync(path.join(dist, i.src)), `icon missing: ${i.src}`);

  check(/rel="manifest"/.test(read("index.html")), "index.html does not link the manifest");
}

if (failures.length > 0) {
  console.error("PWA check failed:\n - " + failures.join("\n - "));
  process.exit(1);
}
console.log("PWA check ok");
