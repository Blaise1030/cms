import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import mdx from "@astrojs/mdx";
import ui from "@nuxt/ui/vite";
import { defineConfig, fontProviders } from "astro/config";
import { orso } from "./integrations/orso/index.js";
import { collections } from "./src/content/collections.js";
import { pagefindIndex } from "./integrations/pagefind.js";
import { legal } from "./integrations/legal.js";
import { site } from "./src/site.ts";
import { collectionsWithTranslations } from "./src/lib/translatedFiles.ts";

import vue from "@astrojs/vue";

// Absolute URL of the live site, used for canonical links, the sitemap, social cards and
// llms.txt. Set PUBLIC_SITE_URL at build time (or change `url` in src/site.ts).
const siteUrl = process.env.PUBLIC_SITE_URL || site.url;

// Nuxt UI swaps in its Vue-compatible components (plain-anchor links, no Nuxt runtime) only for
// files under its real install path. Previews run with node_modules symlinked and keep symlinked
// paths (below), so component imports are pointed at the real path.
const nuxtUiComponents = realpathSync(fileURLToPath(new URL("./node_modules/@nuxt/ui/dist/runtime/components/", import.meta.url)));

export default defineConfig({
  site: siteUrl,
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
    // The site's collections (src/content/collections.js) plus the privacy policy and terms
    // (see integrations/legal.js).
    // Crowdin owns the translations (src/content.config.ts), so orso hides them on purpose.
    orso({
      collections: { ...collections, legal },
      hide: collectionsWithTranslations().map((name) => `${name}I18n`),
    }),
    // Nuxt UI components (Vue), rendered to static HTML in .astro files and hydrated as islands.
    vue({ appEntrypoint: "/src/vue-app.ts" }),
    // Search indexes for the blog and help search boxes (build only; see integrations/pagefind.js).
    pagefindIndex({
      sections: [
        // Translated pages (zh/blog/…) join the same index; Pagefind splits it by <html lang>, so
        // each language only searches its own pages.
        { name: "blog", glob: "{blog,*/blog}/**/*.html" },
        { name: "help", glob: "{help,*/help}/**/*.html" },
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
          // Outlined neutral buttons and badges draw their edge in the site's border gray.
          button: { compoundVariants: [{ color: "neutral", variant: "outline", class: "ring-line" }] },
          badge: { compoundVariants: [{ color: "neutral", variant: "outline", class: "ring-line" }] },
          kbd: { compoundVariants: [{ color: "neutral", variant: "outline", class: "ring-line" }] },
          // Teach Nuxt UI's class merging the site's type roles, so `text-body` on a component
          // replaces its `text-sm` instead of being mistaken for a colour.
          tv: {
            twMergeConfig: {
              extend: {
                classGroups: {
                  "font-size": [{ text: ["label", "small", "body", "heading", "title", "display", "watermark"] }],
                  "ring-color": [{ ring: ["line", "line-strong"] }],
                },
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
  },
});