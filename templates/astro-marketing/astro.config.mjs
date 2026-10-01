import mdx from "@astrojs/mdx";
import { defineConfig } from "astro/config";

// The workbench dev server proxies the site under /preview/{id}/.
const base = process.env.PUBLIC_BASE ?? "/";
const proxyPort = Number(process.env.WORKBENCH_PROXY_PORT || 0);

export default defineConfig({
  base,
  integrations: [mdx()],
  vite: {
    server: {
      // Astro keeps Vite's own base at "/", so without this the HMR socket would connect to
      // the proxy root instead of the /preview/{id}/ prefix. clientPort makes the browser
      // talk to the workbench daemon (not the raw Astro port) when the iframe is proxied.
      hmr: {
        path: `${base}vite-hmr`.replace(/\/{2,}/g, "/"),
        ...(proxyPort > 0 ? { clientPort: proxyPort } : {}),
      },
    },
  },
});
