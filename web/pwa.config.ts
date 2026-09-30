import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import type { VitePWAOptions } from "vite-plugin-pwa";

/**
 * PWA policy: precache the app shell only (hosted-origin JS/CSS/HTML/icons).
 * Live data and previews come from the daemon and are NEVER cached by the service worker:
 *  - no `runtimeCaching` routes, so every non-precached request goes straight to the network
 *    (offline => fetch fails => the app's "daemon unreachable" screen, never stale data);
 *  - navigations to daemon-served paths (`/api`, `/ws`, `/preview`) are excluded from the
 *    SPA navigation fallback so the iframe preview and API are always live;
 *  - `?t=` link navigations hit the network so the daemon can mint its `wb_session` cookie.
 */
/**
 * Shell JS = the scripts `index.html` loads (entry + `modulepreload` static deps). Every other
 * emitted chunk is a lazy `import()` (Shiki grammars/themes/wasm, ...): fetched from the network
 * on demand, never precached. A positive selection, so new lazy chunks stay out by default.
 */
export function shellScripts(html: string): Set<string> {
  const urls = new Set<string>();
  const tag = /<(?:script|link)\b[^>]*>/g;
  for (const [t] of html.matchAll(tag)) {
    const isScript = t.startsWith("<script") && /\btype="module"/.test(t);
    const isPreload = /\brel="modulepreload"/.test(t);
    if (!isScript && !isPreload) continue;
    const m = /\b(?:src|href)="([^"]+)"/.exec(t);
    if (m) urls.add(m[1]!.replace(/^\//, ""));
  }
  return urls;
}

/** Keep non-JS shell files; keep JS only when it is a shell script. */
export function isShellEntry(url: string, shell: Set<string>): boolean {
  return !url.endsWith(".js") || shell.has(url) || !url.startsWith("assets/");
}

const DIST = path.resolve(import.meta.dirname, "dist");

export const SHELL_GLOB_PATTERNS = ["**/*.{js,css,html,ico,png,svg,webmanifest,woff2}"];

export const DAEMON_NAVIGATION_DENYLIST = [/^\/api(\/|$)/, /^\/ws(\/|$)/, /^\/preview(\/|$)/, /[?&]t=/];

export const pwaOptions: Partial<VitePWAOptions> = {
  registerType: "autoUpdate",
  // The manifest is a static file in `public/` (linked from index.html).
  manifest: false,
  includeAssets: ["icons/*.png"],
  workbox: {
    globPatterns: SHELL_GLOB_PATTERNS,
    navigateFallback: "/index.html",
    navigateFallbackDenylist: DAEMON_NAVIGATION_DENYLIST,
    manifestTransforms: [
      (entries) => {
        const index = path.join(DIST, "index.html");
        // Without index.html we cannot tell shell from lazy chunks: precache nothing extra.
        const shell = shellScripts(existsSync(index) ? readFileSync(index, "utf8") : "");
        return { manifest: entries.filter((e) => isShellEntry(e.url, shell)), warnings: [] };
      },
    ],
    cleanupOutdatedCaches: true,
    clientsClaim: true,
    skipWaiting: true,
    // Deliberately no runtimeCaching.
    runtimeCaching: [],
  },
};
