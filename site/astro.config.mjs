import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import mdx from "@astrojs/mdx";
import ui from "@nuxt/ui/vite";
import { defineConfig, fontProviders } from "astro/config";
import { orso } from "./integrations/orso/index.js";
import { marketingCollections } from "./integrations/orso/marketing.js";
import { pagefindIndex } from "./integrations/pagefind.js";
import { site } from "./src/site.ts";

import vue from "@astrojs/vue";

// The orso dev server proxies the site under /preview/{id}/.
const base = process.env.PUBLIC_BASE ?? "/";
// The browser's HMR socket: the dev server's own port when the preview is framed directly
// (orso's Vite dev UI), otherwise the daemon that proxies the preview.
const hmrPort = Number(process.env.ORSO_DEV_PORT || process.env.ORSO_PROXY_PORT || 0);

// Absolute URL of the live site, used for canonical links, the sitemap, social cards and
// llms.txt. Set PUBLIC_SITE_URL at build time (or change `url` in src/site.ts).
const siteUrl = process.env.PUBLIC_SITE_URL || site.url;

// Nuxt UI swaps in its Vue-compatible components (plain-anchor links, no Nuxt runtime) only for
// files under its real install path. Previews run with node_modules symlinked and keep symlinked
// paths (below), so component imports are pointed at the real path.
const nuxtUiComponents = realpathSync(fileURLToPath(new URL("./node_modules/@nuxt/ui/dist/runtime/components/", import.meta.url)));

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
  integrations: [
    mdx(),
    orso({ collections: marketingCollections }),
    // Nuxt UI components (Vue), rendered to static HTML in .astro files and hydrated as islands.
    vue({ appEntrypoint: "/src/vue-app.ts" }),
    // Search indexes for the blog and help search boxes (build only; see integrations/pagefind.js).
    pagefindIndex({
      sections: [
        { name: "blog", glob: "blog/**/*.html" },
        { name: "help", glob: "help/**/*.html" },
      ],
    }),
  ],
  vite: {
    plugins: [
      // Nuxt UI also registers Tailwind's Vite plugin. Components are imported explicitly, links
      // are plain anchors (no vue-router), and light/dark follows the system like the tokens do.
      ui({
        router: false,
        colorMode: false,
        autoImport: false,
        components: false,
        ui: {
          colors: { primary: "blue", neutral: "zinc" },
          // Teach Nuxt UI's class merging the site's type roles, so `text-body` on a component
          // replaces its `text-sm` instead of being mistaken for a colour.
          tv: {
            twMergeConfig: {
              extend: {
                classGroups: { "font-size": [{ text: ["label", "small", "body", "heading", "title", "display", "watermark"] }] },
              },
            },
          },
        },
        // Bundle the Hugeicons the site names (i-hugeicons-…), so icons render at build time and
        // never fetch from the Iconify API. .astro files are scanned too.
        icon: { clientBundle: { scan: { globInclude: ["src/**/*.{vue,astro,ts}"] } } },
      }),
    ],
    // Orso's conflict preview and the template tests run the site with node_modules symlinked
    // in. Keeping symlinked paths lets Astro compile components that ship their own <style>
    // (such as <ClientRouter />) from there. Dedupe keeps every island on one copy of Vue.
    resolve: {
      preserveSymlinks: true,
      dedupe: ["vue"],
      alias: [{ find: /^@nuxt\/ui\/components\//, replacement: `${nuxtUiComponents}/` }],
    },
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