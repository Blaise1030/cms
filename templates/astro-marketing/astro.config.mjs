import mdx from "@astrojs/mdx";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "astro/config";
import { orso } from "./integrations/orso/index.js";
import { marketingCollections } from "./integrations/orso/marketing.js";

// The orso dev server proxies the site under /preview/{id}/.
const base = process.env.PUBLIC_BASE ?? "/";
// The browser's HMR socket: the dev server's own port when the preview is framed directly
// (orso's Vite dev UI), otherwise the daemon that proxies the preview.
const hmrPort = Number(process.env.ORSO_DEV_PORT || process.env.ORSO_PROXY_PORT || 0);

export default defineConfig({
  base,
  integrations: [mdx(), orso({ collections: marketingCollections })],
  vite: {
    plugins: [tailwindcss()],
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
