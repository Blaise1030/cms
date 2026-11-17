import mdx from "@astrojs/mdx";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig, fontProviders } from "astro/config";
import { orso } from "./integrations/orso/index.js";
import { marketingCollections } from "./integrations/orso/marketing.js";
import { site } from "./src/site.ts";

import react from "@astrojs/react";

// The orso dev server proxies the site under /preview/{id}/.
const base = process.env.PUBLIC_BASE ?? "/";
// The browser's HMR socket: the dev server's own port when the preview is framed directly
// (orso's Vite dev UI), otherwise the daemon that proxies the preview.
const hmrPort = Number(process.env.ORSO_DEV_PORT || process.env.ORSO_PROXY_PORT || 0);

// Absolute URL of the live site, used for canonical links, the sitemap, social cards and
// llms.txt. Set PUBLIC_SITE_URL at build time (or change `url` in src/site.ts).
const siteUrl = process.env.PUBLIC_SITE_URL || site.url;

export default defineConfig({
  site: siteUrl,
  base,
  // Geist ships with the site (no third-party font request). Astro writes the
  // @font-face rules, a preload and a metric-matched fallback so the swap causes no layout shift.
  fonts: [
    {
      provider: fontProviders.local(),
      name: "Geist",
      cssVariable: "--font-geist",
      fallbacks: ["ui-sans-serif", "system-ui", "sans-serif"],
      options: {
        variants: [
          { src: ["@fontsource-variable/geist/files/geist-latin-wght-normal.woff2"], weight: "100 900", style: "normal", display: "swap" },
          { src: ["@fontsource-variable/geist/files/geist-latin-wght-italic.woff2"], weight: "100 900", style: "italic", display: "swap" },
        ],
      },
    },
  ],
  integrations: [mdx(), orso({ collections: marketingCollections }), react()],
  vite: {
    plugins: [tailwindcss()],
    // Orso's conflict preview and the template tests run the site with node_modules symlinked
    // in. Keeping symlinked paths lets Astro compile components that ship their own <style>
    // (such as <ClientRouter />) from there.
    resolve: { preserveSymlinks: true },
    server: {
      // Astro keeps Vite's own base at "/", so without this the HMR socket would connect to
      // the proxy root instead of the /preview/{id}/ prefix. clientPort points it at this
      // server (direct preview) or at the orso daemon that proxies it.
      hmr: {
        path: `${base}vite-hmr`.replace(/\/{2,}/g, "/"),
        ...(hmrPort > 0 ? { clientPort: hmrPort } : {}),
      },
    },
  },
});