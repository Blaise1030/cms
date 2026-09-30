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
    cleanupOutdatedCaches: true,
    clientsClaim: true,
    skipWaiting: true,
    // Deliberately no runtimeCaching.
    runtimeCaching: [],
  },
};
